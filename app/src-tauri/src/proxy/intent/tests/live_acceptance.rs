//! Explicitly enabled acceptance checks against the user's native Codex login.
//! Only the route-group fixture and sanitized results touch disk; execution uses
//! an isolated database. No edits to client configuration or tool installations.
use super::*;

struct LiveHarness {
    state: ProxyState,
    db: Arc<Database>,
    group: RouteGroup,
    results: Vec<Value>,
    report: std::path::PathBuf,
}

impl LiveHarness {
    async fn send(
        &mut self,
        case: &str,
        session: &str,
        mut body: Value,
    ) -> (Value, DecisionRecord) {
        body["model"] = json!(self.group.virtual_model());
        body["stream"] = json!(true);
        body["store"] = json!(false);
        body["instructions"] = json!("Follow the user's request. Keep the answer concise.");
        let request = http::Request::builder()
            .method("POST")
            .uri("/v1/responses")
            .header("content-type", "application/json")
            .header("session_id", session)
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let started = std::time::Instant::now();
        let response = tokio::time::timeout(
            Duration::from_secs(90),
            handlers::handle_responses(axum::extract::State(self.state.clone()), request),
        )
        .await
        .expect("proxy timed out")
        .expect("proxy rejected request");
        assert_eq!(response.status(), http::StatusCode::OK, "{case}");
        let bytes = tokio::time::timeout(Duration::from_secs(90), response.into_body().collect())
            .await
            .expect("stream timed out")
            .unwrap()
            .to_bytes();
        let completed = String::from_utf8_lossy(&bytes)
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|event| event["type"] == "response.completed")
            .expect("missing response.completed")["response"]
            .clone();
        let record = self.db.list_intent_decisions(1).unwrap().remove(0);
        self.results.push(
            json!({"case":case,"elapsedMs":started.elapsed().as_millis(),
            "source":record.source,"model":record.model,"reason":record.reason,
            "judgeMillis":record.judge_millis,"judgeInputTokens":record.judge_input_tokens,
            "judgeOutputTokens":record.judge_output_tokens,"workerUsage":completed["usage"],
            "upstreamModel":completed["model"],"completed":true}),
        );
        std::fs::write(
            &self.report,
            serde_json::to_vec_pretty(&self.results).unwrap(),
        )
        .unwrap();
        println!(
            "ACCEPTANCE {case}: source={} worker={} completed=true",
            record.source, record.model
        );
        (completed, record)
    }
}

fn output_text(response: &Value) -> String {
    response["output"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

#[tokio::test]
#[ignore = "requires AG_INTENT_ACCEPTANCE_GROUP, AG_INTENT_ACCEPTANCE_REPORT and real account quota"]
async fn saved_coding_group_live_acceptance() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let config: RouteGroup = serde_json::from_slice(
        &std::fs::read(
            std::env::var("AG_INTENT_ACCEPTANCE_GROUP").expect("group fixture required"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(config.app_type, "codex");
    assert_eq!(config.judge.provider_id, "codex-official");
    assert!(config
        .workers
        .iter()
        .all(|worker| worker.provider_id == "codex-official"));
    let db = Arc::new(Database::memory().unwrap());
    let mut official = Provider::with_id(
        "codex-official".into(),
        "OpenAI Official".into(),
        json!({}),
        None,
    );
    official.category = Some("official".into());
    db.save_provider("codex", &official).unwrap();
    let config = db.save_intent_group(config).unwrap();
    let mut harness = LiveHarness {
        state: ProxyState::for_test(db.clone()),
        db,
        group: config,
        results: Vec::new(),
        report: std::env::var("AG_INTENT_ACCEPTANCE_REPORT").unwrap().into(),
    };
    let session = uuid::Uuid::new_v4().to_string();
    let simple = json!({"input":[{"role":"user","content":"将 red, blue, green 按字母排序，只输出排序后的三个单词。"},
        {"type":"reasoning","encrypted_content":"old-session-test-state"}]});
    let (answer, decision) = harness
        .send("simple_old_history", &session, simple.clone())
        .await;
    assert_eq!(
        decision.source, "judge",
        "simple classification failed: {}",
        decision.reason
    );
    assert_eq!(decision.model, "gpt-6-luna");
    let text = output_text(&answer);
    assert!(
        text.find("blue") < text.find("green") && text.find("green") < text.find("red"),
        "wrong sorted answer"
    );
    let (_, duplicate) = harness.send("duplicate", &session, simple).await;
    assert_eq!(duplicate.source, "duplicate");
    assert_eq!(duplicate.judge_millis, 0);

    let mut input =
        vec![json!({"role":"user","content":"调用 read_fixture 读取文本并原样返回，不做分析。"})];
    let tools = json!([{"type":"function","name":"read_fixture","description":"Read a fixed text fixture",
        "parameters":{"type":"object","properties":{},"additionalProperties":false,"required":[]}}]);
    let tool_session = uuid::Uuid::new_v4().to_string();
    let (tool_response, _) = harness.send("tool_call", &tool_session,
        json!({"input":input,"tools":tools,"tool_choice":{"type":"function","name":"read_fixture"},"include":["reasoning.encrypted_content"]})).await;
    let call = tool_response["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call")
        .expect("tool call missing");
    input.extend(tool_response["output"].as_array().unwrap().clone());
    input.push(json!({"type":"function_call_output","call_id":call["call_id"],"output":"TOOL_ROUNDTRIP_OK"}));
    let (followup, _) = harness
        .send(
            "tool_result_continuation",
            &tool_session,
            json!({"input":input,"tools":tools,"tool_choice":"none"}),
        )
        .await;
    assert!(output_text(&followup).contains("TOOL_ROUNDTRIP_OK"));

    let complex_session = uuid::Uuid::new_v4().to_string();
    let (_, complex) = harness.send("complex_design", &complex_session, json!({"input":[{"role":"user","content":
        "设计跨地域支付账本：消息至少一次投递、跨节点时钟偏差、分区重试与并发扣款。分析如何保证余额不负与幂等，给出线性化点、故障时序反例和不变量。必须做并发一致性推理，以不超过400字给出设计。"}]})).await;
    assert_eq!(
        complex.source, "judge",
        "complex classification failed: {}",
        complex.reason
    );
    assert_eq!(complex.model, "gpt-6.1-sol");
}
