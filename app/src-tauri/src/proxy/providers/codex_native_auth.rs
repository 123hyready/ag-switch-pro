//! Native Codex login used by router-owned requests. Credentials stay in auth.json,
//! outside provider rows and cloud sync. Re-read on every call so logout/account
//! changes take effect, and serialize refreshes made by this process.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use std::{path::Path, sync::LazyLock, time::Duration};
use tokio::sync::Mutex;

use super::codex_oauth_auth::{CODEX_OAUTH_CLIENT_VERSION, CODEX_OAUTH_ORIGINATOR};
use crate::proxy::ProxyError;

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REFRESH_BUFFER_SECONDS: i64 = 300;
static NATIVE_LOGIN: LazyLock<NativeLogin> = LazyLock::new(NativeLogin::default);

#[derive(Default)]
struct NativeLogin {
    refresh_lock: Mutex<()>,
}

// Deliberately no Debug implementation: these values must not enter logs.
pub(crate) struct NativeCredential {
    pub access_token: String,
    pub account_id: String,
}

fn auth_error(message: &str) -> ProxyError {
    ProxyError::AuthError(message.to_string())
}

fn read_login(path: &Path) -> Result<Value, ProxyError> {
    let bytes =
        std::fs::read(path).map_err(|_| auth_error("无法读取 Codex 登录，请先运行 codex login"))?;
    serde_json::from_slice(&bytes).map_err(|_| auth_error("Codex auth.json 无效，请重新登录"))
}

fn token_field<'a>(auth: &'a Value, field: &str) -> Option<&'a str> {
    auth.get("tokens")?
        .get(field)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()
}

fn credential(auth: &Value) -> Result<NativeCredential, ProxyError> {
    if auth["auth_mode"] == "apikey" {
        return Err(auth_error(
            "Codex 当前使用 API Key 登录，请选择 API Key 供应商",
        ));
    }
    let access_token = token_field(auth, "access_token")
        .ok_or_else(|| auth_error("Codex 未登录 ChatGPT，请先运行 codex login"))?;
    let id_claims = token_field(auth, "id_token").and_then(claims);
    let account_id = token_field(auth, "account_id")
        .map(str::to_string)
        .or_else(|| {
            id_claims
                .as_ref()?
                .get("https://api.openai.com/auth")?
                .get("chatgpt_account_id")?
                .as_str()
                .map(str::to_string)
        })
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| auth_error("Codex 登录缺少 ChatGPT workspace ID，请重新登录"))?;
    Ok(NativeCredential {
        access_token: access_token.into(),
        account_id,
    })
}

fn expires_soon(token: &str) -> bool {
    // Match Magpie: unknown expiry is left for the upstream to validate.
    claims(token)
        .and_then(|c| c["exp"].as_i64())
        .is_some_and(|exp| exp <= chrono::Utc::now().timestamp() + REFRESH_BUFFER_SECONDS)
}

impl NativeLogin {
    async fn credentials(
        &self,
        path: &Path,
        token_url: &str,
    ) -> Result<NativeCredential, ProxyError> {
        let _guard = self.refresh_lock.lock().await;
        // A second attempt adopts a CLI rotation observed during our refresh.
        for _ in 0..2 {
            let original = read_login(path)?;
            let current = credential(&original)?;
            if !expires_soon(&current.access_token) {
                return Ok(current);
            }
            let refresh_token = token_field(&original, "refresh_token")
                .ok_or_else(|| auth_error("Codex 登录已到期且没有刷新令牌，请重新登录"))?;
            let response = crate::proxy::http_client::get()
                .post(token_url)
                .timeout(Duration::from_secs(30))
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh_token),
                    ("client_id", CLIENT_ID),
                    ("scope", "openid profile email"),
                ])
                .send()
                .await
                .map_err(|_| auth_error("Codex 登录刷新网络失败，请稍后重试"))?;
            let status = response.status();
            let refreshed: Value = if status.is_success() {
                response
                    .json()
                    .await
                    .map_err(|_| auth_error("Codex 登录刷新响应无效"))?
            } else {
                // Do not expose response bodies, which can contain credential material.
                if read_login(path)? != original {
                    continue;
                }
                return Err(auth_error(&format!(
                    "Codex 登录刷新失败（HTTP {status}），请重新登录"
                )));
            };
            let new_access = refreshed["access_token"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| auth_error("Codex 登录刷新响应缺少 access_token"))?;
            if let Some(new_claims) = refreshed["id_token"].as_str().and_then(claims) {
                let workspace =
                    new_claims["https://api.openai.com/auth"]["chatgpt_account_id"].as_str();
                let old_claims = token_field(&original, "id_token").and_then(claims);
                let old_user = old_claims.as_ref().and_then(|c| c["sub"].as_str());
                let new_user = new_claims["sub"].as_str();
                if workspace.is_some_and(|id| id != current.account_id)
                    || (old_user.is_some() && new_user.is_some() && old_user != new_user)
                {
                    return Err(auth_error("Codex 刷新后的账号身份不一致，请重新登录"));
                }
            }
            let mut updated = original.clone();
            updated["tokens"]["access_token"] = json!(new_access);
            for field in ["refresh_token", "id_token"] {
                if let Some(value) = refreshed[field].as_str().filter(|s| !s.trim().is_empty()) {
                    updated["tokens"][field] = json!(value);
                }
            }
            if credential(&updated)?.account_id != current.account_id {
                return Err(auth_error("Codex 刷新后的账号身份不一致，请重新登录"));
            }
            updated["last_refresh"] = json!(chrono::Utc::now().to_rfc3339());
            // Best effort across processes: Codex CLI does not share our mutex.
            // Compare the entire document immediately before atomic replacement.
            if read_login(path)? != original {
                continue;
            }
            crate::config::write_json_file_private(path, &updated)
                .map_err(|_| auth_error("无法保存刷新后的 Codex 登录"))?;
            return credential(&updated);
        }
        Err(auth_error("Codex 登录正在被其他进程更新，请重试"))
    }
}

pub(crate) async fn current_credential() -> Result<NativeCredential, ProxyError> {
    NATIVE_LOGIN
        .credentials(&crate::codex_config::get_codex_auth_path(), TOKEN_URL)
        .await
}

pub(crate) fn apply_headers(
    headers: &mut http::HeaderMap,
    credential: &NativeCredential,
) -> Result<(), ProxyError> {
    let bearer = http::HeaderValue::from_str(&format!("Bearer {}", credential.access_token))
        .map_err(|_| auth_error("Codex 登录令牌格式无效"))?;
    let account = http::HeaderValue::from_str(&credential.account_id)
        .map_err(|_| auth_error("Codex workspace ID 格式无效"))?;
    let version = request_version(headers).to_string();
    let user_agent = http::HeaderValue::from_str(&format!(
        "codex_cli_rs/{version} ({}; {}) AGSwitchPro",
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
    .map_err(|_| auth_error("Codex 客户端版本格式无效"))?;
    // Replace stale client identity together with the token; never mix accounts.
    headers.insert(http::header::AUTHORIZATION, bearer);
    headers.insert("chatgpt-account-id", account);
    headers.insert(
        "originator",
        http::HeaderValue::from_static(CODEX_OAUTH_ORIGINATOR),
    );
    headers.insert(
        "version",
        http::HeaderValue::from_str(&version)
            .map_err(|_| auth_error("Codex 客户端版本格式无效"))?,
    );
    headers.insert(http::header::USER_AGENT, user_agent);
    headers.insert(
        "openai-beta",
        http::HeaderValue::from_static("responses=experimental"),
    );
    headers.insert(
        http::header::ACCEPT,
        http::HeaderValue::from_static("text/event-stream"),
    );
    Ok(())
}

fn request_version(headers: &http::HeaderMap) -> &str {
    let parse = |version: &str| -> Option<Vec<u32>> {
        let parts: Vec<u32> = version
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        (parts.len() == 3).then_some(parts)
    };
    let Some(version) = headers.get("version").and_then(|value| value.to_str().ok()) else {
        return CODEX_OAUTH_CLIENT_VERSION;
    };
    match (parse(version), parse(CODEX_OAUTH_CLIENT_VERSION)) {
        (Some(incoming), Some(minimum)) if incoming >= minimum => version,
        _ => CODEX_OAUTH_CLIENT_VERSION,
    }
}

/// Align the cache identity with Codex's modern headers; an explicit body key wins.
pub(crate) fn apply_session_headers(
    headers: &mut http::HeaderMap,
    body: &Value,
) -> Result<(), ProxyError> {
    let session = body
        .get("prompt_cache_key")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            headers
                .get("session-id")
                .and_then(|value| value.to_str().ok())
        })
        .or_else(|| {
            headers
                .get("session_id")
                .and_then(|value| value.to_str().ok())
        })
        .filter(|value| !value.trim().is_empty());
    let Some(session) = session else {
        return Ok(());
    };
    let value = http::HeaderValue::from_str(session)
        .map_err(|_| ProxyError::InvalidRequest("Codex 会话标识格式无效".into()))?;
    headers.remove("session_id");
    headers.remove("conversation_id");
    headers.insert("session-id", value.clone());
    headers.insert("thread-id", value);
    Ok(())
}

#[cfg(test)]
mod tests;
