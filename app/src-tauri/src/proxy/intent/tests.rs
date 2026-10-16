use super::*;
mod live_acceptance;

/// Opt-in: uses the current native Codex login and consumes real model quota.
#[tokio::test]
#[ignore = "real Codex account and billed model calls required"]
async fn live_native_codex_full_history_route() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let db = Arc::new(Database::memory().unwrap());
    let mut official = provider("codex-official", "codex", "https://chatgpt.com");
    official.category = Some("official".into());
    db.save_provider("codex", &official).unwrap();
    let mut config = group("codex");
    config.judge.provider_id = "codex-official".into();
    config.judge.model = "gpt-6-luna".into();
    config.judge_timeout_ms = 30000;
    for worker in &mut config.workers {
        worker.provider_id = "codex-official".into();
        worker.model = "gpt-6-luna".into();
        worker.images = true;
        worker.context_tokens = 272000;
    }
    db.save_intent_group(config).unwrap();
    let state = ProxyState::for_test(db.clone());
    let body = json!({"model":"ags-group/coding","stream":true,"store":false,
    "instructions":"Answer the user briefly.","input":[
        {"role":"user","content":"Reply with exactly ROUTE_OK"},
        {"type":"reasoning","encrypted_content":"old-model-opaque-test-state"}
    ]});
    let request = http::Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header("content-type", "application/json")
        .header("session_id", "00000000-0000-4000-8000-000000000099")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(90),
        handlers::handle_responses(axum::extract::State(state), request),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);
    let bytes = tokio::time::timeout(Duration::from_secs(90), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("response.completed"),
        "upstream stream did not complete"
    );
    assert!(text.contains("ROUTE_OK"), "expected worker output missing");
    let records = db.list_intent_decisions(10).unwrap();
    assert_eq!(records.len(), 1);
    println!(
        "LIVE_ROUTE_OK source={} model=gpt-6-luna completed=true",
        records[0].source
    );
}

#[test]
fn full_history_replay_removes_only_opaque_reasoning() {
    let original = json!({"input":[
        {"role":"user","content":"Fix cache"},
        {"type":"reasoning","encrypted_content":"opaque"},
        {"type":"function_call","call_id":"c1","name":"read","arguments":"{}"},
        {"type":"function_call_output","call_id":"c1","output":"file content"}
    ]});
    let mut replay = original.clone();
    prepare_codex_replay(&mut replay);
    assert_eq!(replay["input"].as_array().unwrap().len(), 3);
    assert_eq!(replay["input"][2], original["input"][3]);
    assert!(!RequestFacts::from_body("codex", Some("s"), "main", &replay).sealed_state);
    let mut incremental = original.clone();
    incremental["previous_response_id"] = json!("resp1");
    let before = incremental.clone();
    prepare_codex_replay(&mut incremental);
    assert_eq!(incremental, before);
    let mut compacted = original;
    compacted["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"compaction","encrypted_content":"state"}));
    let before = compacted.clone();
    prepare_codex_replay(&mut compacted);
    assert_eq!(compacted, before);
}
use crate::proxy::{handlers, server::ProxyState};
use ag_intent_router::RouteMode;
use axum::{routing::post, Json, Router};
use http_body_util::BodyExt;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn group(app: &str) -> RouteGroup {
    RouteGroup {
        id: "coding".into(),
        name: "Coding".into(),
        app_type: app.into(),
        enabled: true,
        mode: RouteMode::Stage,
        judge: ag_intent_router::ModelTarget {
            provider_id: "judge".into(),
            model: "judge-model".into(),
        },
        workers: vec![
            Worker {
                id: "cheap".into(),
                provider_id: "workers".into(),
                model: "cheap-model".into(),
                description: "Cheap mechanical tasks".into(),
                context_tokens: 128000,
                tools: true,
                images: false,
            },
            Worker {
                id: "strong".into(),
                provider_id: "workers".into(),
                model: "strong-model".into(),
                description: "Complex reasoning".into(),
                context_tokens: 128000,
                tools: true,
                images: true,
            },
        ],
        default_worker: "strong".into(),
        fallback_worker: "strong".into(),
        pinned_worker: None,
        max_hold_requests: 3,
        max_judge_calls: 20,
        judge_timeout_ms: 1000,
        revision: 1,
    }
}

fn provider(id: &str, app: &str, base: &str) -> Provider {
    let settings = if app == "claude" {
        json!({"env":{"ANTHROPIC_BASE_URL":base,"ANTHROPIC_AUTH_TOKEN":"test-key"}})
    } else {
        json!({"base_url":base,"auth":{"OPENAI_API_KEY":"test-key"},"config":format!("model = \"strong-model\"\nmodel_provider = \"custom\"\n[model_providers.custom]\nbase_url = \"{base}\"\nwire_api = \"responses\"\n")})
    };
    Provider::with_id(id.into(), id.into(), settings, None)
}

async fn mock_upstream(
    fail_cheap: bool,
    judge_calls: Arc<AtomicUsize>,
) -> (String, tokio::task::JoinHandle<()>) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let handler = move |Json(body): Json<Value>| {
        let calls = judge_calls.clone();
        async move {
            let model = body["model"].as_str().unwrap_or("");
            if fail_cheap && model == "cheap-model" {
                return (
                    http::StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error":{"message":"temporarily unavailable"}})),
                );
            }
            let output = if model == "judge-model" {
                calls.fetch_add(1, Ordering::SeqCst);
                json!({"workerId":"cheap","phase":"collect","holdRequests":3,"reason":"Mechanical task"}).to_string()
            } else {
                format!("Executed {model}")
            };
            let response = if body.get("messages").is_some() {
                json!({"id":format!("msg-{model}"),"type":"message","role":"assistant","model":model,"content":[{"type":"text","text":output}],"stop_reason":"end_turn","usage":{"input_tokens":20,"output_tokens":10}})
            } else {
                json!({"id":format!("resp-{model}"),"object":"response","status":"completed","model":model,"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":output}]}],"usage":{"input_tokens":20,"output_tokens":10}})
            };
            (http::StatusCode::OK, Json(response))
        }
    };
    let app = Router::new()
        .route("/v1/responses", post(handler.clone()))
        .route("/v1/messages", post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), task)
}

async fn request(state: ProxyState, app: &str, input: Value) -> Value {
    let body = if app == "claude" {
        json!({"model":"ags-claude-group-coding","messages":[{"role":"user","content":input}],"max_tokens":100,"stream":false,"metadata":{"session_id":"test-session"}})
    } else {
        json!({"model":"ags-group/coding","input":input,"stream":false})
    };
    let request = http::Request::builder()
        .method("POST")
        .uri(if app == "claude" {
            "/v1/messages"
        } else {
            "/v1/responses"
        })
        .header("content-type", "application/json")
        .header("session_id", "00000000-0000-4000-8000-000000000001")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = if app == "claude" {
        handlers::handle_messages(axum::extract::State(state), request)
            .await
            .unwrap()
    } else {
        handlers::handle_responses(axum::extract::State(state), request)
            .await
            .unwrap()
    };
    assert_eq!(response.status(), http::StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn codex_request_uses_judge_and_reuses_decision_without_changing_global_route() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (base, upstream) = mock_upstream(false, calls.clone()).await;
    let db = Arc::new(Database::memory().unwrap());
    db.save_provider("codex", &provider("judge", "codex", &base))
        .unwrap();
    db.save_provider("codex", &provider("workers", "codex", &base))
        .unwrap();
    db.save_intent_group(group("codex")).unwrap();
    let state = ProxyState::for_test(db.clone());
    let first = request(state.clone(), "codex", json!("Read this file")).await;
    let duplicate = request(state.clone(), "codex", json!("Read this file")).await;
    assert_eq!(first["model"], "cheap-model");
    assert_eq!(duplicate["model"], "cheap-model");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(state.current_providers.read().await.is_empty());
    let records = db.list_intent_decisions(10).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().any(|record| record.source == "duplicate"));
    upstream.abort();
}

#[tokio::test]
async fn claude_request_fails_over_before_output_to_a_different_model_on_the_same_provider() {
    let (base, upstream) = mock_upstream(true, Arc::new(AtomicUsize::new(0))).await;
    let db = Arc::new(Database::memory().unwrap());
    db.save_provider("claude", &provider("judge", "claude", &base))
        .unwrap();
    db.save_provider("claude", &provider("workers", "claude", &base))
        .unwrap();
    db.save_intent_group(group("claude")).unwrap();
    let state = ProxyState::for_test(db.clone());
    let response = request(state.clone(), "claude", json!("Read this file")).await;
    assert_eq!(response["model"], "strong-model");
    assert_eq!(
        db.list_intent_decisions(1).unwrap()[0].source,
        "execution_fallback"
    );
    let trace = db.list_intent_decisions(1).unwrap().remove(0);
    assert_eq!(trace.decision_source.as_deref(), Some("judge"));
    assert_eq!(trace.judge_model.as_deref(), Some(group("claude").judge.model.as_str()));
    assert_eq!(trace.selected_model.as_deref(), Some("cheap-model"));
    assert_eq!(trace.execution_attempts.len(), 2);
    assert_eq!(trace.execution_attempts[0].model, "cheap-model");
    assert_eq!(trace.execution_attempts[0].status, "failed");
    assert_eq!(trace.execution_attempts[1].model, "strong-model");
    assert_eq!(trace.execution_attempts[1].status, "accepted");
    let mut legacy = serde_json::to_value(&trace).unwrap();
    for key in ["judgeModel", "decisionSource", "selectedModel", "recommendedModel", "executionAttempts"] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    let legacy: crate::database::DecisionRecord = serde_json::from_value(legacy).unwrap();
    assert!(legacy.judge_model.is_none());
    assert!(legacy.execution_attempts.is_empty());
    assert!(state.current_providers.read().await.is_empty());
    upstream.abort();
}

#[test]
fn native_responses_usage_preserves_cache_read_tokens() {
    let response = json!({"object":"response","output":[{"content":[{"text":"{}"}]}],"usage":{"input_tokens":100,"output_tokens":10,"input_tokens_details":{"cached_tokens":80}}});
    let (_, usage, inclusive) = parse_judge_body(response.to_string().as_bytes()).unwrap();
    assert!(inclusive);
    assert_eq!(usage.unwrap().cache_read_tokens, 80);
}

#[tokio::test]
async fn execution_fallback_does_not_revisit_a_cooling_worker() {
    let (base, upstream) = mock_upstream(false, Arc::new(AtomicUsize::new(0))).await;
    let db = Arc::new(Database::memory().unwrap());
    db.save_provider("codex", &provider("judge", "codex", &base))
        .unwrap();
    db.save_provider("codex", &provider("workers", "codex", &base))
        .unwrap();
    let config = db.save_intent_group(group("codex")).unwrap();
    let state = ProxyState::for_test(db);
    state
        .intent_router
        .failed(&config, config.worker("strong").unwrap())
        .await;
    let mut body = json!({"model":"ags-group/coding","input":"Read the file"});
    let (_, execution) = resolve(&state, &AppType::Codex, &HeaderMap::new(), &mut body)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        execution.targets.len(),
        1,
        "A cooling fallback must not be sent another execution request"
    );
    assert_eq!(execution.targets[0].2.id, "cheap");
    upstream.abort();
}

#[test]
fn groups_have_independent_catalog_ids_and_conservative_context() {
    let db = Database::memory().unwrap();
    db.save_intent_group(group("codex")).unwrap();
    db.save_intent_group(RouteGroup {
        id: "claude-coding".into(),
        ..group("claude")
    })
    .unwrap();
    let codex = append_codex_models(&db, Some(json!({"models":[]})))
        .unwrap()
        .unwrap();
    assert_eq!(codex["models"][0]["slug"], "ags-group/coding");
    assert_eq!(codex["models"][0]["context_window"], 128000);
    assert_eq!(
        claude_models(&db).unwrap()[0].id,
        "ags-claude-group-claude-coding"
    );
}

#[test]
fn saving_group_advances_revision_and_delete_refuses_old_alias() {
    let db = Database::memory().unwrap();
    let first = db.save_intent_group(group("codex")).unwrap();
    let second = db.save_intent_group(group("codex")).unwrap();
    assert_eq!(second.revision, first.revision + 1);
    db.delete_intent_group(&first.id).unwrap();
    assert!(db.get_intent_group(&first.id).unwrap().is_none());
}

#[test]
fn virtual_catalog_has_complete_native_fields_with_empty_or_legacy_base() {
    let db = Database::memory().unwrap();
    db.save_intent_group(group("codex")).unwrap();
    for base in [
        None,
        Some(json!({"models":[]})),
        Some(json!({"models":[{
        "slug":"legacy","base_instructions":"Keep custom instructions"}]})),
    ] {
        let catalog = append_codex_models(&db, base).unwrap().unwrap();
        let entry = catalog["models"].as_array().unwrap().last().unwrap();
        assert_eq!(entry["support_verbosity"], false);
        assert!(entry["truncation_policy"]["limit"].is_number());
        assert!(entry["experimental_supported_tools"].is_array());
        assert!(entry["supports_parallel_tool_calls"].is_boolean());
        assert!(entry["supports_reasoning_summaries"].is_boolean());
        assert_eq!(entry["max_context_window"], 128000);
        assert_eq!(entry["auto_compact_token_limit"], 115200);
        if catalog["models"].as_array().unwrap().len() == 2 {
            assert_eq!(entry["base_instructions"], "Keep custom instructions");
        }
    }
}

#[test]
fn codex_native_official_is_eligible_without_copying_credentials_into_provider() {
    let db = Database::memory().unwrap();
    let mut official = Provider::with_id(
        "official".into(),
        "OpenAI Official".into(),
        json!({"auth":{},"config":""}),
        None,
    );
    official.category = Some("official".into());
    db.save_provider("codex", &official).unwrap();
    let loaded = load_provider(&db, "codex", "official").unwrap();
    assert_eq!(loaded.settings_config["auth"], json!({}));
    db.save_provider("claude", &official).unwrap();
    assert!(load_provider(&db, "claude", "official").is_err());
}

#[test]
fn official_judge_uses_subscription_wire_contract_but_custom_judge_keeps_api_contract() {
    let mut official = Provider::with_id(
        "official".into(),
        "OpenAI Official".into(),
        json!({"auth":{},"config":""}),
        None,
    );
    official.category = Some("official".into());
    let body = judge_body(&AppType::Codex, &official, "judge-model", "task summary");
    assert_eq!(body["model"], "judge-model");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["reasoning"]["effort"], "low");
    assert!(body.get("max_output_tokens").is_none());
    let custom = provider("judge", "codex", "https://gateway.example/v1");
    let body = judge_body(&AppType::Codex, &custom, "judge-model", "task summary");
    assert_eq!(body["stream"], false);
    assert_eq!(body["max_output_tokens"], 1536);
}

#[test]
fn official_judge_sse_returns_decision_and_real_usage() {
    let answer = json!({"workerId":"cheap","phase":"collect","holdRequests":3,"reason":"Simple"})
        .to_string();
    let delta = json!({"type":"response.output_text.delta","delta":answer});
    let completed = json!({"type":"response.completed","response":{"usage":{
        "input_tokens":100,"output_tokens":20,"input_tokens_details":{"cached_tokens":80}}}});
    let sse = format!("data: {delta}\n\ndata: {completed}\n\ndata: [DONE]\n\n");
    let (text, usage, inclusive) = parse_judge_body(sse.as_bytes()).unwrap();
    let decision: JudgeAnswer = serde_json::from_str(&text).unwrap();
    assert_eq!(decision.worker_id, "cheap");
    assert_eq!(usage.unwrap().cache_read_tokens, 80);
    assert!(inclusive);
}
