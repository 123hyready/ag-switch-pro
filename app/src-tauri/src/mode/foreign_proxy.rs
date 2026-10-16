//! Refuse takeover while another known gateway still owns the client's active route.
use crate::app_config::AppType;
use serde_json::Value;

fn known_gateway(base: &str, token: Option<&str>) -> bool {
    if token == Some("AG_SWITCH_PRO_PROXY_MANAGED") {
        return false;
    }
    if token.is_some_and(|token| token == "PROXY_MANAGED" || token.starts_with("magpie-")) {
        return true;
    }
    let Ok(url) = url::Url::parse(base) else {
        return false;
    };
    let loopback = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
    );
    loopback && matches!(url.port(), Some(15721 | 3425))
}

pub fn refuse_takeover(app: &AppType) -> Result<(), String> {
    let foreign = match app {
        AppType::Claude => {
            let path = crate::config::get_claude_settings_path();
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let settings: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let base = settings
                .pointer("/env/ANTHROPIC_BASE_URL")
                .and_then(Value::as_str)
                .unwrap_or("");
            let token = settings
                .pointer("/env/ANTHROPIC_AUTH_TOKEN")
                .or_else(|| settings.pointer("/env/ANTHROPIC_API_KEY"))
                .and_then(Value::as_str);
            known_gateway(base, token)
        }
        AppType::Codex => {
            let text = crate::codex_config::read_codex_config_text().unwrap_or_default();
            let config: toml::Value = text
                .parse()
                .unwrap_or(toml::Value::Table(Default::default()));
            let active = config
                .get("model_provider")
                .and_then(toml::Value::as_str)
                .unwrap_or("openai");
            let table = config
                .get("model_providers")
                .and_then(|providers| providers.get(active));
            let base = table
                .and_then(|table| table.get("base_url"))
                .or_else(|| config.get("openai_base_url"))
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            let token = table
                .and_then(|table| table.get("experimental_bearer_token"))
                .and_then(toml::Value::as_str);
            let old_catalog = config
                .get("model_catalog_json")
                .and_then(toml::Value::as_str)
                .is_some_and(|path| {
                    path.replace('\\', "/").rsplit('/').next()
                        == Some("cc-switch-model-catalog.json")
                });
            known_gateway(base, token) || old_catalog
        }
        _ => false,
    };
    if foreign {
        Err(
            "客户端仍由 CC Switch 或 magpie 接管，请先在对应应用恢复直连，再启用 AG Switch Pro。"
                .into(),
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_known_foreign_ports_but_not_this_apps_marker() {
        assert!(known_gateway(
            "http://127.0.0.1:15721/v1",
            Some("PROXY_MANAGED")
        ));
        assert!(known_gateway("http://localhost:3425", None));
        assert!(!known_gateway(
            "http://127.0.0.1:15722/v1",
            Some("AG_SWITCH_PRO_PROXY_MANAGED")
        ));
        assert!(!known_gateway("https://api.example.invalid:15721", None));
    }
}
