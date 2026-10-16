//! Exercise the real forwarder against a loopback upstream and isolated login.
use super::super::providers::CodexAdapter;
use super::*;
use serde_json::json;
use std::time::Duration;

struct LoopbackOfficialAdapter(String);

impl ProviderAdapter for LoopbackOfficialAdapter {
    fn name(&self) -> &'static str {
        "LoopbackOfficial"
    }
    fn extract_base_url(&self, _: &Provider) -> Result<String, ProxyError> {
        Ok(self.0.clone())
    }
    fn extract_auth(&self, provider: &Provider) -> Option<AuthInfo> {
        CodexAdapter.extract_auth(provider)
    }
    fn build_url(&self, base: &str, endpoint: &str) -> String {
        format!("{base}{endpoint}")
    }
    fn get_auth_headers(
        &self,
        auth: &AuthInfo,
    ) -> Result<Vec<(http::HeaderName, http::HeaderValue)>, ProxyError> {
        CodexAdapter.get_auth_headers(auth)
    }
}

struct IsolatedHome {
    _dir: tempfile::TempDir,
    previous: Option<std::ffi::OsString>,
}

impl IsolatedHome {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("CC_SWITCH_TEST_HOME");
        std::env::set_var("CC_SWITCH_TEST_HOME", dir.path());
        crate::settings::reload_settings().unwrap();
        let auth_path = crate::codex_config::get_codex_auth_path();
        assert!(
            auth_path.starts_with(dir.path()),
            "Test must never access the real login"
        );
        crate::config::write_json_file_private(
            &auth_path,
            &json!({"auth_mode":"chatgpt",
            "tokens":{"access_token":"native-test-token","account_id":"native-workspace"}}),
        )
        .unwrap();
        Self {
            _dir: dir,
            previous,
        }
    }
}

impl Drop for IsolatedHome {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("CC_SWITCH_TEST_HOME", value),
            None => std::env::remove_var("CC_SWITCH_TEST_HOME"),
        }
        crate::settings::reload_settings().unwrap();
    }
}

#[tokio::test]
#[serial_test::serial]
async fn native_codex_login_authenticates_judge_and_worker_at_actual_forwarder_boundary() {
    let _home = IsolatedHome::new();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let app = axum::Router::new().route(
        "/responses",
        axum::routing::post(
            move |headers: http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                let tx = tx.clone();
                async move {
                    tx.send((headers, body.clone())).await.unwrap();
                    axum::Json(json!({"object":"response","model":body["model"],"output":[]}))
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adapter = LoopbackOfficialAdapter(format!("http://{}", listener.local_addr().unwrap()));
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut provider = Provider::with_id(
        "official".into(),
        "OpenAI Official".into(),
        json!({"auth":{},"config":""}),
        None,
    );
    provider.category = Some("official".into());
    assert!(super::super::providers::is_codex_official_provider(
        &provider
    ));
    let forwarder = super::tests::test_forwarder(Duration::from_secs(5), Duration::from_secs(5))
        .stack_request(true);
    for model in ["judge-model", "worker-model"] {
        let mut incoming = http::HeaderMap::new();
        if model == "worker-model" {
            incoming.insert(
                "authorization",
                http::HeaderValue::from_static("Bearer stale-client-token"),
            );
            incoming.insert(
                "chatgpt-account-id",
                http::HeaderValue::from_static("stale-workspace"),
            );
        }
        let body = json!({"model":model,"input":"test","stream":false});
        let mut format = None;
        forwarder
            .forward(
                &AppType::Codex,
                &http::Method::POST,
                &provider,
                "/responses",
                &body,
                &incoming,
                &Extensions::new(),
                &adapter,
                &mut format,
            )
            .await
            .unwrap();
        let (sent_headers, sent_body) = rx.recv().await.unwrap();
        assert_eq!(sent_headers["authorization"], "Bearer native-test-token");
        assert_eq!(sent_headers["chatgpt-account-id"], "native-workspace");
        assert_eq!(sent_headers["originator"], "codex_cli_rs");
        assert_eq!(sent_body["model"], model);
    }
    task.abort();
}
