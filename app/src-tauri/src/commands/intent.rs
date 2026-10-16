use crate::{app_config::AppType, database::DecisionRecord, proxy::intent, store::AppState};
use ag_intent_router::{RequestFacts, RouteGroup};
use std::sync::Arc;
use tauri::State;

/// Fetch selectable IDs using the same provider and account as route execution.
#[tauri::command]
pub async fn get_intent_provider_models(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    app_type: String,
    provider_id: String,
) -> Result<Vec<crate::services::model_fetch::FetchedModel>, String> {
    use tauri::Manager;
    let provider = intent::load_provider(&state.db, &app_type, &provider_id)
        .map_err(|error| error.to_string())?;
    if app_type == "codex" && crate::proxy::providers::is_codex_official_provider(&provider) {
        let credential = match provider
            .meta
            .as_ref()
            .and_then(|meta| meta.managed_account_id_for("codex_oauth"))
        {
            Some(id) => {
                let oauth = app.state::<crate::commands::CodexOAuthState>();
                crate::proxy::providers::codex_native_auth::NativeCredential {
                    access_token: oauth
                        .0
                        .get_valid_token_for_account(&id)
                        .await
                        .map_err(|e| e.to_string())?,
                    account_id: oauth
                        .0
                        .chatgpt_account_id_for_account(&id)
                        .await
                        .map_err(|e| e.to_string())?,
                }
            }
            None => crate::proxy::providers::codex_native_auth::current_credential()
                .await
                .map_err(|e| e.to_string())?,
        };
        return crate::services::codex_oauth_models::fetch_models_with_token(
            &credential.access_token,
            &credential.account_id,
        )
        .await;
    }
    let client = if app_type == "claude" {
        AppType::Claude
    } else {
        AppType::Codex
    };
    let adapter = crate::proxy::providers::get_adapter(&client).ok_or("此客户端不支持模型发现")?;
    let base = adapter
        .extract_base_url(&provider)
        .map_err(|e| e.to_string())?;
    let auth = adapter
        .extract_auth(&provider)
        .ok_or("供应商缺少认证信息")?;
    if provider.uses_managed_account_auth() {
        return Err(
            "请先在供应商配置中获取托管账号模型列表；此账号类型尚未接入路由模型发现".into(),
        );
    }
    crate::services::model_fetch::fetch_models(
        &base,
        &auth.api_key,
        false,
        None,
        None,
        provider
            .meta
            .as_ref()
            .and_then(|meta| meta.api_format.as_deref()),
        None,
    )
    .await
}

#[tauri::command]
pub fn list_intent_groups(state: State<'_, AppState>) -> Result<Vec<RouteGroup>, String> {
    state
        .db
        .list_intent_groups()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_intent_group(
    state: State<'_, AppState>,
    group: RouteGroup,
) -> Result<RouteGroup, String> {
    group.validate()?;
    intent::load_provider(&state.db, &group.app_type, &group.judge.provider_id)
        .map_err(|error| error.to_string())?;
    for worker in &group.workers {
        intent::load_provider(&state.db, &group.app_type, &worker.provider_id)
            .map_err(|error| error.to_string())?;
    }
    let saved = state
        .db
        .save_intent_group(group)
        .map_err(|error| error.to_string())?;
    let app = if saved.app_type == "claude" {
        AppType::Claude
    } else {
        AppType::Codex
    };
    // Resync only already-attached clients; saving a group does not start takeover.
    crate::mode::controller::resync_route(state.inner(), &app)
        .await
        .map_err(|error| format!("路由组已保存，但客户端目录同步失败：{error}"))?;
    Ok(saved)
}

#[tauri::command]
pub async fn delete_intent_group(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let existing = state
        .db
        .get_intent_group(&id)
        .map_err(|error| error.to_string())?;
    state
        .db
        .delete_intent_group(&id)
        .map_err(|error| error.to_string())?;
    if let Some(group) = existing {
        let app = if group.app_type == "claude" {
            AppType::Claude
        } else {
            AppType::Codex
        };
        crate::mode::controller::resync_route(state.inner(), &app).await?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_intent_decisions(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<DecisionRecord>, String> {
    state
        .db
        .list_intent_decisions(limit.unwrap_or(100))
        .map_err(|error| error.to_string())
}

/// A real, billed classifier request in isolated state; no work request or client writes.
#[tauri::command]
pub async fn simulate_intent_route(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    group: RouteGroup,
    text: String,
) -> Result<DecisionRecord, String> {
    group.validate()?;
    if text.trim().is_empty() || text.len() > 32_000 {
        return Err("请输入不超过 32000 字节的任务描述".into());
    }
    let server = crate::proxy::server::ProxyServer::new(
        crate::proxy::ProxyConfig::default(),
        state.db.clone(),
        Some(app),
    );
    let proxy = server.state();
    let facts = RequestFacts::from_body(
        &group.app_type,
        None,
        "simulation",
        &serde_json::json!({"input":text}),
    );
    let router = Arc::new(ag_intent_router::Router::default());
    let selection = router
        .route(&group, &facts, |input| {
            intent::ask_judge(proxy.clone(), group.clone(), None, input)
        })
        .await?;
    intent::decision_record(&proxy, &group, &facts, &selection).map_err(|error| error.to_string())
}
