use super::*;
use axum::{routing::post, Json, Router};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn jwt(value: Value) -> String {
    format!("e30.{}.", URL_SAFE_NO_PAD.encode(value.to_string()))
}

fn login(exp: i64) -> Value {
    json!({"auth_mode":"chatgpt", "tokens":{
        "access_token":jwt(json!({"exp":exp})), "refresh_token":"refresh-original",
        "id_token":jwt(json!({"sub":"user-one", "https://api.openai.com/auth":{"chatgpt_account_id":"workspace-one"}})),
        "account_id":"workspace-one"}, "custom":{"preserved":true}})
}

fn seed(path: &Path, auth: &Value) {
    crate::config::write_json_file_private(path, auth).unwrap();
}

async fn mock_refresh(
    status: http::StatusCode,
    response: Value,
    mutation: Option<(std::path::PathBuf, Value)>,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let handler = move |axum::extract::Form(form): axum::extract::Form<
        std::collections::HashMap<String, String>,
    >| {
        let counter = counter.clone();
        let response = response.clone();
        let mutation = mutation.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(form["grant_type"], "refresh_token");
            assert_eq!(form["refresh_token"], "refresh-original");
            assert_eq!(form["client_id"], CLIENT_ID);
            if let Some((path, auth)) = mutation {
                seed(&path, &auth);
            }
            (status, Json(response))
        }
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/token", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/token", post(handler)))
            .await
            .unwrap();
    });
    (url, calls, task)
}

#[tokio::test]
async fn valid_login_is_read_without_network_or_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    seed(&path, &login(chrono::Utc::now().timestamp() + 3600));
    let before = std::fs::read(&path).unwrap();
    let result = NativeLogin::default()
        .credentials(&path, "http://127.0.0.1:1")
        .await
        .unwrap();
    assert_eq!(result.account_id, "workspace-one");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn refresh_rotates_tokens_preserves_unknown_fields_and_serializes_callers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    seed(&path, &login(1));
    let fresh_token = jwt(json!({"exp":chrono::Utc::now().timestamp()+3600}));
    let (url, calls, task) = mock_refresh(
        http::StatusCode::OK,
        json!({"access_token":fresh_token,"refresh_token":"refresh-new"}),
        None,
    )
    .await;
    let manager = NativeLogin::default();
    let (first, second) = tokio::join!(
        manager.credentials(&path, &url),
        manager.credentials(&path, &url)
    );
    assert_eq!(first.unwrap().access_token, fresh_token);
    assert_eq!(second.unwrap().access_token, fresh_token);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let saved = read_login(&path).unwrap();
    assert_eq!(saved["tokens"]["refresh_token"], "refresh-new");
    assert_eq!(saved["custom"]["preserved"], true);
    assert!(saved["last_refresh"].as_str().is_some());
    task.abort();
}

#[tokio::test]
async fn cli_rotation_during_refresh_wins_including_after_server_rejection() {
    for status in [http::StatusCode::OK, http::StatusCode::UNAUTHORIZED] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        seed(&path, &login(1));
        let cli_login = login(chrono::Utc::now().timestamp() + 7200);
        let (url, calls, task) = mock_refresh(
            status,
            json!({"access_token":"discarded"}),
            Some((path.clone(), cli_login.clone())),
        )
        .await;
        let result = NativeLogin::default()
            .credentials(&path, &url)
            .await
            .unwrap();
        assert_eq!(
            result.access_token,
            cli_login["tokens"]["access_token"].as_str().unwrap()
        );
        assert_eq!(read_login(&path).unwrap(), cli_login);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        task.abort();
    }
}

#[tokio::test]
async fn logout_during_refresh_is_not_resurrected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    seed(&path, &login(1));
    let (url, _, task) = mock_refresh(
        http::StatusCode::OK,
        json!({"access_token":"discarded"}),
        Some((path.clone(), json!({}))),
    )
    .await;
    assert!(NativeLogin::default()
        .credentials(&path, &url)
        .await
        .is_err());
    assert_eq!(read_login(&path).unwrap(), json!({}));
    task.abort();
}

#[tokio::test]
async fn rereads_account_switch_and_logout_between_requests() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    let manager = NativeLogin::default();
    let mut auth = login(chrono::Utc::now().timestamp() + 3600);
    seed(&path, &auth);
    assert_eq!(
        manager
            .credentials(&path, "unused")
            .await
            .unwrap()
            .account_id,
        "workspace-one"
    );
    auth["tokens"]["account_id"] = json!("workspace-two");
    seed(&path, &auth);
    assert_eq!(
        manager
            .credentials(&path, "unused")
            .await
            .unwrap()
            .account_id,
        "workspace-two"
    );
    std::fs::remove_file(&path).unwrap();
    assert!(manager.credentials(&path, "unused").await.is_err());
}

#[tokio::test]
async fn rejected_refresh_is_sanitized_and_does_not_change_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("auth.json");
    let original = login(1);
    seed(&path, &original);
    let (url, _, task) = mock_refresh(
        http::StatusCode::UNAUTHORIZED,
        json!({"error":"refresh-original secret"}),
        None,
    )
    .await;
    let error = match NativeLogin::default().credentials(&path, &url).await {
        Ok(_) => panic!("rejected refresh unexpectedly succeeded"),
        Err(e) => e.to_string(),
    };
    assert!(!error.contains("refresh-original"));
    assert_eq!(read_login(&path).unwrap(), original);
    task.abort();
}

#[tokio::test]
async fn malformed_or_changed_identity_refresh_is_not_persisted() {
    for response in [
        json!({}),
        json!({"access_token":"new", "id_token":jwt(json!({
        "sub":"other-user", "https://api.openai.com/auth":{"chatgpt_account_id":"workspace-one"}}))}),
        json!({"access_token":"new", "id_token":jwt(json!({
        "sub":"user-one", "https://api.openai.com/auth":{"chatgpt_account_id":"other-workspace"}}))}),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let original = login(1);
        seed(&path, &original);
        let (url, _, task) = mock_refresh(http::StatusCode::OK, response, None).await;
        assert!(NativeLogin::default()
            .credentials(&path, &url)
            .await
            .is_err());
        assert_eq!(read_login(&path).unwrap(), original);
        task.abort();
    }
}

#[test]
fn missing_token_api_key_and_missing_workspace_are_rejected() {
    for auth in [
        json!({}),
        json!({"auth_mode":"apikey","OPENAI_API_KEY":"key"}),
        json!({"tokens":{"access_token":"token"}}),
    ] {
        assert!(credential(&auth).is_err());
    }
}

#[test]
fn workspace_can_be_discovered_from_id_token() {
    let mut auth = login(1);
    auth["tokens"].as_object_mut().unwrap().remove("account_id");
    assert_eq!(credential(&auth).unwrap().account_id, "workspace-one");
}

#[test]
fn replaces_stale_bearer_and_workspace_together() {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        "authorization",
        http::HeaderValue::from_static("Bearer stale"),
    );
    headers.insert(
        "chatgpt-account-id",
        http::HeaderValue::from_static("old-workspace"),
    );
    headers.insert("session_id", http::HeaderValue::from_static("session-one"));
    let credential = NativeCredential {
        access_token: "current".into(),
        account_id: "current-workspace".into(),
    };
    apply_headers(&mut headers, &credential).unwrap();
    assert_eq!(headers["authorization"], "Bearer current");
    assert_eq!(headers["chatgpt-account-id"], "current-workspace");
    assert_eq!(headers["originator"], CODEX_OAUTH_ORIGINATOR);
    assert_eq!(headers["session_id"], "session-one");
}

#[test]
fn invalid_header_material_is_rejected_without_partial_update() {
    let mut headers = http::HeaderMap::new();
    let credential = NativeCredential {
        access_token: "token".into(),
        account_id: "bad\nworkspace".into(),
    };
    assert!(apply_headers(&mut headers, &credential).is_err());
    assert!(headers.is_empty());
}

#[test]
fn codex_version_and_user_agent_are_consistent_and_do_not_downgrade_client() {
    for (incoming, expected) in [
        ("0.162.0", "0.162.0"),
        ("0.1.0", CODEX_OAUTH_CLIENT_VERSION),
        ("invalid", CODEX_OAUTH_CLIENT_VERSION),
    ] {
        let mut headers = http::HeaderMap::new();
        headers.insert("version", http::HeaderValue::from_str(incoming).unwrap());
        let credential = NativeCredential {
            access_token: "token".into(),
            account_id: "workspace".into(),
        };
        apply_headers(&mut headers, &credential).unwrap();
        assert_eq!(headers["version"], expected);
        assert!(headers["user-agent"]
            .to_str()
            .unwrap()
            .starts_with(&format!("codex_cli_rs/{expected} ")));
        assert_eq!(headers["openai-beta"], "responses=experimental");
    }
}

#[test]
fn explicit_cache_key_wins_over_old_session_headers() {
    let mut headers = http::HeaderMap::new();
    headers.insert("session_id", http::HeaderValue::from_static("old"));
    headers.insert("conversation_id", http::HeaderValue::from_static("old"));
    apply_session_headers(&mut headers, &json!({"prompt_cache_key":"cache-key"})).unwrap();
    assert_eq!(headers["session-id"], "cache-key");
    assert_eq!(headers["thread-id"], "cache-key");
    assert!(!headers.contains_key("session_id"));
    assert!(!headers.contains_key("conversation_id"));
}

#[test]
fn legacy_session_is_translated_when_no_explicit_cache_key_exists() {
    let mut headers = http::HeaderMap::new();
    headers.insert("session_id", http::HeaderValue::from_static("session-one"));
    apply_session_headers(&mut headers, &json!({})).unwrap();
    assert_eq!(headers["session-id"], "session-one");
    assert_eq!(headers["thread-id"], "session-one");
}
