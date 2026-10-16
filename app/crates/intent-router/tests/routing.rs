use ag_intent_router::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn worker(id: &str) -> Worker {
    Worker {
        id: id.into(),
        provider_id: "provider".into(),
        model: id.into(),
        description: id.into(),
        context_tokens: 200_000,
        tools: true,
        images: false,
    }
}

#[tokio::test]
async fn image_payload_bytes_do_not_exhaust_a_vision_workers_context() {
    let mut config = group();
    config.workers[0].images = true;
    let request = facts(
        json!({"input":[{"role":"user","content":[
            {"type":"input_text","text":"Describe this screenshot"},
            {"type":"input_image","image_url":format!("data:image/png;base64,{}", "A".repeat(600_000))}
        ]}]}),
        "image",
    );
    assert!(request.needs_images);
    assert!(request.estimated_tokens < 20_000);
    let selected = Router::default()
        .route(&config, &request, cheap)
        .await
        .unwrap();
    assert_eq!(selected.worker.id, "cheap");
}

#[test]
fn tool_schemas_are_not_image_inputs_but_nested_tool_result_images_are() {
    let request = facts(
        json!({"input":"Use this tool", "tools":[{
            "type":"function","name":"render","parameters":{"type":"object",
            "properties":{"example":{"type":"input_image"}}}
        }]}),
        "schema",
    );
    assert!(!request.needs_images);
    assert!(request.needs_tools);
    for body in [
        json!({"input":[{"type":"function_call_output","output":[{"type":"input_image","image_url":"https://example.test/image.png"}]}]}),
        json!({"messages":[{"role":"user","content":[{"type":"tool_result","content":[{"type":"image","source":{"type":"base64","data":"AAAA"}}]}]}]}),
    ] {
        assert!(facts(body, "tool-image").needs_images);
    }
}

#[tokio::test]
async fn rejected_workers_report_capacity_and_capability_reasons_without_request_text() {
    let mut config = group();
    config.workers[0].tools = false;
    let mut request = facts(json!({"input":"private request text"}), "rejected");
    request.needs_images = true;
    request.needs_tools = true;
    request.estimated_tokens = 300_000;
    request.unavailable_workers.push("strong".into());
    let error = Router::default()
        .route(&config, &request, cheap)
        .await
        .unwrap_err();
    for reason in [
        "图片输入未启用",
        "工具调用未启用",
        "300000",
        "200000",
        "供应商不可用",
        "cheap",
        "strong",
    ] {
        assert!(error.contains(reason), "{error}");
    }
    assert!(!error.contains("private request text"));
}

#[tokio::test]
async fn all_workers_cooling_down_reports_retry_instead_of_incompatible_context() {
    let config = group();
    let router = Router::default();
    for worker in &config.workers {
        router.failed(&config, worker).await;
    }
    let error = router
        .route(&config, &facts(json!({"input":"Task"}), "cooldown"), cheap)
        .await
        .unwrap_err();
    assert!(error.contains("故障冷却"), "{error}");
    assert!(!error.contains("上下文不足"), "{error}");
}

fn group() -> RouteGroup {
    RouteGroup {
        id: "coding".into(),
        name: "Coding".into(),
        app_type: "codex".into(),
        enabled: true,
        mode: RouteMode::Stage,
        judge: ModelTarget {
            provider_id: "judge".into(),
            model: "small".into(),
        },
        workers: vec![worker("cheap"), worker("strong")],
        default_worker: "strong".into(),
        fallback_worker: "strong".into(),
        pinned_worker: None,
        max_hold_requests: 3,
        max_judge_calls: 20,
        judge_timeout_ms: 1000,
        revision: 1,
    }
}

fn facts(body: serde_json::Value, session: &str) -> RequestFacts {
    RequestFacts::from_body("codex", Some(session), "main", &body)
}

async fn cheap(_: DecisionInput) -> Result<JudgeAnswer, String> {
    Ok(JudgeAnswer {
        worker_id: "cheap".into(),
        phase: "collect".into(),
        hold_requests: 3,
        reason: "Collect known information".into(),
        input_tokens: Some(20),
        output_tokens: Some(10),
        cost_usd: None,
    })
}

#[test]
fn rejects_unknown_default_and_duplicate_worker_ids() {
    let mut config = group();
    config.default_worker = "missing".into();
    assert!(config.validate().is_err());
    config.default_worker = "strong".into();
    config.workers.push(worker("cheap"));
    assert!(config.validate().is_err());
}

#[test]
fn extracts_goal_from_user_message_not_tool_output() {
    let request = facts(
        json!({"input":[{"role":"user","content":"Fix the cache"},
        {"type":"function_call_output","call_id":"call-1","output":"Read file"}]}),
        "s1",
    );
    assert_eq!(request.goal, "Fix the cache");
    assert!(request.observation.contains("Read file"));
}

#[tokio::test]
async fn reuses_phase_and_does_not_charge_duplicate_request() {
    let router = Router::default();
    let config = group();
    let first = facts(
        json!({"input":[{"role":"user","content":"Fix the cache"}]}),
        "s1",
    );
    let decided = router.route(&config, &first, cheap).await.unwrap();
    assert_eq!(decided.worker.id, "cheap");
    assert_eq!(decided.source, DecisionSource::Judge);
    let duplicate = router
        .route(&config, &first, |_| async {
            panic!("Duplicate asked judge")
        })
        .await
        .unwrap();
    assert_eq!(duplicate.source, DecisionSource::Duplicate);
    let continuation = facts(
        json!({"input":[{"role":"user","content":"Fix the cache"},
        {"type":"function_call_output","call_id":"c1","output":"File contents"}]}),
        "s1",
    );
    let held = router
        .route(&config, &continuation, |_| async {
            panic!("Held phase asked judge")
        })
        .await
        .unwrap();
    assert_eq!(held.source, DecisionSource::Hold);
}

#[tokio::test]
async fn new_goal_and_tool_failure_reconsider_phase() {
    let router = Router::default();
    let config = group();
    router
        .route(
            &config,
            &facts(json!({"input":"Read the file"}), "s1"),
            cheap,
        )
        .await
        .unwrap();
    let failed = facts(
        json!({"input":[{"role":"user","content":"Read the file"},
        {"type":"function_call_output","call_id":"c1","output":"error: test failed"}]}),
        "s1",
    );
    let selected = router
        .route(&config, &failed, |_| async {
            Ok(JudgeAnswer {
                worker_id: "strong".into(),
                phase: "diagnose".into(),
                hold_requests: 2,
                reason: "Investigate failure".into(),
                input_tokens: None,
                output_tokens: None,
                cost_usd: None,
            })
        })
        .await
        .unwrap();
    assert_eq!(selected.worker.id, "strong");
    assert_eq!(selected.source, DecisionSource::Judge);
    let next = router
        .route(
            &config,
            &facts(json!({"input":"Design concurrency control"}), "s1"),
            cheap,
        )
        .await
        .unwrap();
    assert_eq!(next.source, DecisionSource::Judge);
}

#[tokio::test]
async fn failed_judge_uses_configured_fallback() {
    let router = Router::default();
    let result = router
        .route(&group(), &facts(json!({"input":"Task"}), "s1"), |_| async {
            Err("Unavailable".into())
        })
        .await
        .unwrap();
    assert_eq!(result.worker.id, "strong");
    assert_eq!(result.source, DecisionSource::Fallback);
    assert!(result.reason.contains("Unavailable"));
}

#[tokio::test]
async fn rejects_incompatible_judge_selection_without_dropping_images() {
    let router = Router::default();
    let mut config = group();
    config.workers[1].images = true;
    let request = facts(
        json!({"input":[{"role":"user","content":[{"type":"input_image","image_url":"https://example.invalid/image"}]}]}),
        "s1",
    );
    let selected = router.route(&config, &request, cheap).await.unwrap();
    assert_eq!(selected.worker.id, "strong");
    assert_eq!(selected.source, DecisionSource::Fallback);
}

#[tokio::test]
async fn sealed_state_pins_previous_target_and_refuses_unknown_origin() {
    let router = Router::default();
    let config = group();
    router
        .route(
            &config,
            &facts(json!({"input":"Read the file"}), "s1"),
            cheap,
        )
        .await
        .unwrap();
    let sealed = facts(
        json!({"previous_response_id":"resp1","input":[{"type":"function_call_output","call_id":"c1","output":"Done"}]}),
        "s1",
    );
    router
        .accepted(
            &config,
            &facts(json!({"input":"Read the file"}), "s1"),
            worker("cheap"),
        )
        .await;
    let kept = router
        .route(&config, &sealed, |_| async {
            panic!("Sealed state asked judge")
        })
        .await
        .unwrap();
    assert_eq!(kept.worker.id, "cheap");
    assert_eq!(kept.source, DecisionSource::Continuity);
    assert!(router
        .route(
            &config,
            &facts(json!({"previous_response_id":"resp1"}), "new"),
            cheap
        )
        .await
        .is_err());
}

#[tokio::test]
async fn judged_but_unaccepted_work_does_not_own_incremental_state() {
    let router = Router::default();
    let config = group();
    let initial = facts(json!({"input":"Read the file"}), "unaccepted");
    router.route(&config, &initial, cheap).await.unwrap();
    let sealed = facts(
        json!({"previous_response_id":"resp1","input":"Continue"}),
        "unaccepted",
    );
    let error = router
        .route(&config, &sealed, |_| async {
            panic!("Sealed state must not be judged")
        })
        .await
        .unwrap_err();
    assert!(error.contains("No upstream has accepted"));
}

#[tokio::test]
async fn incremental_state_stays_with_the_worker_that_accepted_execution_fallback() {
    let router = Router::default();
    let config = group();
    let initial = facts(json!({"input":"Read the file"}), "fallback-owner");
    router.route(&config, &initial, cheap).await.unwrap();
    router.accepted(&config, &initial, worker("strong")).await;
    let sealed = facts(
        json!({"previous_response_id":"resp1","input":"Continue"}),
        "fallback-owner",
    );
    let selection = router
        .route(&config, &sealed, |_| async {
            panic!("Sealed state must not be judged")
        })
        .await
        .unwrap();
    assert_eq!(selection.worker.id, "strong");
    assert_eq!(selection.source, DecisionSource::Continuity);
}

#[tokio::test]
async fn concurrent_duplicate_requests_share_one_judgment() {
    let router = Arc::new(Router::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let mut jobs = Vec::new();
    for _ in 0..8 {
        let router = router.clone();
        let calls = calls.clone();
        jobs.push(tokio::spawn(async move {
            router
                .route(
                    &group(),
                    &facts(json!({"input":"Read the file"}), "s1"),
                    |input| async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        cheap(input).await
                    },
                )
                .await
                .unwrap()
        }));
    }
    for job in jobs {
        assert_eq!(job.await.unwrap().worker.id, "cheap");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn different_branches_and_revisions_do_not_share_decisions() {
    let router = Router::default();
    let mut config = group();
    let request = facts(json!({"input":"Read file"}), "s1");
    router.route(&config, &request, cheap).await.unwrap();
    let mut branch = request.clone();
    branch.branch = "child".into();
    assert_eq!(
        router.route(&config, &branch, cheap).await.unwrap().source,
        DecisionSource::Judge
    );
    config.revision += 1;
    assert_eq!(
        router.route(&config, &request, cheap).await.unwrap().source,
        DecisionSource::Judge
    );
}

#[tokio::test]
async fn exhausted_judge_budget_uses_fallback_and_pinned_model_skips_judge() {
    let router = Router::default();
    let mut config = group();
    config.max_judge_calls = 0;
    let request = facts(json!({"input":"Task"}), "s1");
    let selected = router
        .route(&config, &request, |_| async { panic!("Budget exhausted") })
        .await
        .unwrap();
    assert_eq!(selected.source, DecisionSource::Budget);
    config.pinned_worker = Some("cheap".into());
    config.revision += 1;
    assert_eq!(
        router
            .route(&config, &request, |_| async { panic!("Pinned model") })
            .await
            .unwrap()
            .source,
        DecisionSource::Pinned
    );
}

#[tokio::test]
async fn phase_transition_reconsiders_before_hold_expires() {
    let router = Router::default();
    let config = group();
    router
        .route(&config, &facts(json!({"input":"Fix cache"}), "s"), cheap)
        .await
        .unwrap();
    let edit = facts(
        json!({"input":[{"role":"user","content":"Fix cache"},
        {"type":"function_call","call_id":"c","name":"apply_patch","arguments":"patch"},
        {"type":"function_call_output","call_id":"c","output":"Done"}]}),
        "s",
    );
    assert_eq!(edit.phase_signal.as_deref(), Some("implement"));
    assert_eq!(
        router.route(&config, &edit, cheap).await.unwrap().source,
        DecisionSource::Judge
    );
}

#[tokio::test]
async fn turn_mode_keeps_model_after_tool_failure() {
    let router = Router::default();
    let mut config = group();
    config.mode = RouteMode::Turn;
    router
        .route(&config, &facts(json!({"input":"Fix cache"}), "s"), cheap)
        .await
        .unwrap();
    let failed = facts(
        json!({"input":[{"role":"user","content":"Fix cache"},
        {"type":"function_call_output","output":"error: failed"}]}),
        "s",
    );
    assert_eq!(
        router
            .route(&config, &failed, |_| async { panic!("Turn should hold") })
            .await
            .unwrap()
            .source,
        DecisionSource::Hold
    );
}

#[tokio::test]
async fn observe_executes_default_and_paused_skips_judge() {
    let router = Router::default();
    let mut config = group();
    config.mode = RouteMode::Observe;
    let request = facts(json!({"input":"Task"}), "s");
    let observed = router.route(&config, &request, cheap).await.unwrap();
    assert_eq!(observed.worker.id, "strong");
    assert_eq!(observed.recommended_worker, "cheap");
    assert_eq!(observed.source, DecisionSource::Observe);
    config.enabled = false;
    config.revision += 1;
    assert_eq!(
        router
            .route(&config, &request, |_| async { panic!("Paused") })
            .await
            .unwrap()
            .source,
        DecisionSource::Paused
    );
}

#[tokio::test]
async fn timeout_cools_judge_without_delaying_the_next_goal() {
    let router = Router::default();
    let mut config = group();
    config.judge_timeout_ms = 100;
    let result = router
        .route(
            &config,
            &facts(json!({"input":"First task"}), "s"),
            |_| async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                Err("Too slow".into())
            },
        )
        .await
        .unwrap();
    assert!(result.reason.contains("timed out"));
    let next = router
        .route(
            &config,
            &facts(json!({"input":"Second task"}), "s"),
            |_| async { panic!("Cooling judge called") },
        )
        .await
        .unwrap();
    assert!(next.reason.contains("cooling"));
}

#[tokio::test]
async fn continue_carries_goal_into_the_next_phase_judgment() {
    let router = Router::default();
    let mut config = group();
    config.max_hold_requests = 1;
    router
        .route(
            &config,
            &facts(json!({"input":"Design a concurrency-safe cache"}), "s"),
            cheap,
        )
        .await
        .unwrap();
    router
        .route(
            &config,
            &facts(json!({"input":"继续"}), "s"),
            |input| async {
                assert_eq!(input.goal, "Design a concurrency-safe cache");
                cheap(input).await
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn unknown_worker_falls_back_and_no_capable_worker_refuses_request() {
    let router = Router::default();
    let result = router
        .route(&group(), &facts(json!({"input":"Task"}), "s"), |_| async {
            Ok(JudgeAnswer {
                worker_id: "unknown".into(),
                phase: "collect".into(),
                hold_requests: 100,
                reason: "Invalid target".into(),
                input_tokens: None,
                output_tokens: None,
                cost_usd: None,
            })
        })
        .await
        .unwrap();
    assert_eq!(result.source, DecisionSource::Fallback);
    let request = facts(
        json!({"input":[{"role":"user","content":[{"type":"input_image"}]}]}),
        "s",
    );
    assert!(router.route(&group(), &request, cheap).await.is_err());
}

#[tokio::test]
async fn requests_without_session_never_share_phase_state() {
    let router = Router::default();
    let mut request = facts(json!({"input":"Task"}), "s");
    request.session = None;
    assert_eq!(
        router
            .route(&group(), &request, cheap)
            .await
            .unwrap()
            .source,
        DecisionSource::Judge
    );
    assert_eq!(
        router
            .route(&group(), &request, cheap)
            .await
            .unwrap()
            .source,
        DecisionSource::Judge
    );
}

#[tokio::test]
async fn late_acceptance_does_not_override_a_more_recent_goal() {
    let router = Router::default();
    let config = group();
    let older = facts(json!({"input":"Task one"}), "s");
    let newer = facts(json!({"input":"Task two"}), "s");
    router.route(&config, &older, cheap).await.unwrap();
    router.route(&config, &newer, cheap).await.unwrap();
    router.accepted(&config, &older, worker("strong")).await;
    let next = facts(
        json!({"input":[{"role":"user","content":"Task two"},{"type":"function_call_output","output":"Done"}]}),
        "s",
    );
    let held = router
        .route(&config, &next, |_| async { panic!("Hold expected") })
        .await
        .unwrap();
    assert_eq!(held.worker.id, "cheap");
}

#[test]
fn messages_format_and_long_context_are_bounded_for_judge_only() {
    let long = "x".repeat(20_000);
    let request = facts(
        json!({"messages":[{"role":"user","content":long},
        {"role":"assistant","content":[{"type":"tool_use","name":"exec_command","input":{"cmd":"cargo test"}}]},
        {"role":"user","content":[{"type":"tool_result","is_error":true,"content":"FAILED"}]}]}),
        "s",
    );
    assert!(request.goal.contains("[truncated]"));
    assert!(request.tool_failed);
    assert_eq!(request.phase_signal.as_deref(), Some("diagnose"));
    assert!(request.estimated_tokens > request.goal.len() as u64);
    let encrypted = facts(
        json!({"input":[{"type":"reasoning","encrypted_content":"opaque"}]}),
        "s",
    );
    assert!(encrypted.sealed_state);
}

#[test]
fn validation_rejects_invalid_limits_and_virtual_ids_follow_client() {
    for field in [0, 21] {
        let mut config = group();
        config.max_hold_requests = field;
        assert!(config.validate().is_err());
    }
    let mut config = group();
    config.judge.model.clear();
    assert!(config.validate().is_err());
    config = group();
    config.app_type = "gemini".into();
    assert!(config.validate().is_err());
    config = group();
    config.id = "bad/id".into();
    assert!(config.validate().is_err());
    config = group();
    config.pinned_worker = Some("unknown".into());
    assert!(config.validate().is_err());
    assert_eq!(group().virtual_model(), "ags-group/coding");
    config = group();
    config.app_type = "claude".into();
    assert_eq!(config.virtual_model(), "ags-claude-group-coding");
}

#[tokio::test]
async fn signatures_in_tool_schemas_metadata_and_results_do_not_lock_new_conversations() {
    let request = facts(
        json!({
            "tools": [{"type":"function", "name":"verify", "parameters":{
                "type":"object", "properties":{"signature":{"type":"string"},
                    "encrypted_content":{"type":"string"}, "thoughtSignature":{"type":"string"}}
            }}],
            "metadata":{"signature":"application data"},
            "input":[
                {"role":"user", "content":"Inspect the signature"},
                {"type":"function_call", "name":"verify", "arguments":{"signature":"argument"}},
                {"type":"function_call_output", "output":{"type":"reasoning", "encrypted_content":"business data"}}
            ]
        }),
        "schema-signature",
    );
    assert!(!request.sealed_state);
    let selected = Router::default()
        .route(&group(), &request, cheap)
        .await
        .unwrap();
    assert_eq!(selected.source, DecisionSource::Judge);

    let claude = facts(
        json!({"messages":[{"role":"user", "content":[
            {"type":"tool_result", "content":[{"type":"thinking", "signature":"tool output"}]}
        ]}]}),
        "tool-signature",
    );
    assert!(!claude.sealed_state);
}

#[test]
fn actual_provider_state_remains_locked_to_its_owner() {
    for body in [
        json!({"previous_response_id":"resp_123", "input":"Continue"}),
        json!({"input":[{"type":"reasoning", "encrypted_content":"opaque"}]}),
        json!({"input":[{"type":"compaction", "encrypted_content":"opaque"}]}),
        json!({"input":[{"type":"agent_message", "content":[]}]}),
        json!({"messages":[{"role":"assistant", "content":[{"type":"thinking", "signature":"opaque"}]}]}),
        json!({"messages":[{"role":"assistant", "content":[{"type":"redacted_thinking", "data":"opaque"}]}]}),
        json!({"messages":[{"role":"assistant", "parts":[{"text":"thought", "thoughtSignature":"opaque"}]}]}),
    ] {
        assert!(facts(body.clone(), "actual-state").sealed_state, "{body}");
    }
}

#[tokio::test]
async fn failed_worker_is_not_offered_to_the_judge_on_the_next_goal() {
    let router = Router::default();
    let config = group();
    router.failed(&config, &config.workers[0]).await;
    let chosen = router
        .route(
            &config,
            &facts(json!({"input":"Task"}), "s"),
            |input| async move {
                assert!(input.candidates.iter().all(|worker| worker.id != "cheap"));
                Ok(JudgeAnswer {
                    worker_id: "strong".into(),
                    phase: "diagnose".into(),
                    hold_requests: 1,
                    reason: "Available model".into(),
                    input_tokens: None,
                    output_tokens: None,
                    cost_usd: None,
                })
            },
        )
        .await
        .unwrap();
    assert_eq!(chosen.worker.id, "strong");
}

#[tokio::test]
async fn deleted_provider_and_client_mismatch_are_handled_before_execution() {
    let router = Router::default();
    let mut request = facts(json!({"input":"Task"}), "s");
    request.unavailable_workers.push("cheap".into());
    let selected = router.route(&group(), &request, cheap).await.unwrap();
    assert_eq!(selected.worker.id, "strong");
    request.app_type = "claude".into();
    assert!(router.route(&group(), &request, cheap).await.is_err());
}
