//! Store → service settings. Every command reads the store fresh (`store.all()`) and maps it
//! with these functions, so a settings change applies to the next call, like the TS services
//! that called `store.get(...)` per request.

use attachments::AttachmentPaths;
use bedrock::{AwsSettings, ConverseSettings};
use serde_json::Value;

/// `ConverseService`'s store reads (`aws`, `inferenceParams`, `thinkingMode`,
/// `guardrailSettings`, `bedrockSettings`). Malformed keys fall back to
/// their defaults one by one instead of failing the call.
pub fn converse_settings(store: &Value) -> ConverseSettings {
    tools::converse_settings_from_store(store)
}

/// The `aws` key alone (credentials, region, proxy) for the non-Converse services.
pub fn aws_settings(store: &Value) -> AwsSettings {
    converse_settings(store).aws
}

fn non_empty_str<'a>(store: &'a Value, key: &str) -> Option<&'a str> {
    store
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// `projectPath` / `userDataPath` for the attachments crate.
pub fn attachment_paths(store: &Value) -> AttachmentPaths {
    AttachmentPaths::new(
        non_empty_str(store, "projectPath"),
        non_empty_str(store, "userDataPath"),
    )
}

/// `aws.proxyConfig` as a proxy URL (`createProxyAgents`): `None` unless enabled with a host.
#[cfg(test)]
fn proxy_url(store: &Value) -> Option<String> {
    tools::ToolSettings::from_store(store)
        .proxy
        .and_then(|p| p.proxy_url())
}

fn proxy_config(store: &Value) -> Option<tools::ProxyConfig> {
    tools::ToolSettings::from_store(store).proxy
}

/// `builder` with the stored proxy (see `tools::web::proxy`), built. When a proxy is on, every
/// request goes through it or fails; nothing ever connects directly while a proxy is on, not
/// even if building the client fails (AGENT-B5).
fn build_with_proxy(builder: reqwest::ClientBuilder, store: &Value) -> reqwest::Client {
    let proxy = proxy_config(store);
    if let Some(reason) = tools::web::proxy::broken_reason(proxy.as_ref()) {
        tracing::warn!(reason = %reason, "Proxy is on but unusable; requests will fail");
    }
    tools::web::proxy::proxied(builder, proxy.as_ref())
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "Failed to build HTTP client; requests will fail");
            refusing_client(format!("The app couldn't set up its HTTP client: {e}"))
        })
}

/// A client whose every request fails with `reason`.
fn refusing_client(reason: String) -> reqwest::Client {
    tools::web::proxy::refuse(reqwest::Client::builder(), reason)
        .build()
        .expect("a client with only a proxy and a resolver builds")
}

/// Whether the stored proxy is on and usable (so name resolution happens at the proxy).
fn proxy_on(store: &Value) -> bool {
    matches!(
        proxy_config(store).map(|p| p.setting()),
        Some(tools::web::proxy::ProxySetting::On(_))
    )
}

/// An HTTP client honoring the stored proxy (the MCP registry search, the embedding model
/// download).
pub fn http_client(store: &Value) -> reqwest::Client {
    build_with_proxy(reqwest::Client::builder(), store)
}

/// [`http_client`] for MCP URL servers: redirects stay on the server's origin, so the headers
/// configured for it (API keys, tokens) are never re-sent to another host (AGENT-M5).
pub fn mcp_http_client(store: &Value) -> reqwest::Client {
    build_with_proxy(
        reqwest::Client::builder().redirect(mcp::client::redirect_policy()),
        store,
    )
}

/// [`http_client`] for fetches the model can aim (the knowledge base website crawler): the
/// destination checks of `tools::web::net_guard` on every request and redirect, with the store's
/// `blockLocalNetworkAccess` switch (DATA-1).
pub fn guarded_http_client(store: &Value) -> reqwest::Client {
    let block_local = store
        .get("blockLocalNetworkAccess")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let builder =
        tools::web::net_guard::guard(reqwest::Client::builder(), block_local, proxy_on(store));
    build_with_proxy(builder, store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn converse_settings_come_from_store_keys() {
        let store = json!({
            "aws": {
                "region": "eu-west-1",
                "accessKeyId": "AKIA",
                "secretAccessKey": "s",
                "useProfile": true,
                "profile": "dev",
                "proxyConfig": { "enabled": true, "host": "proxy.local", "port": 3128 }
            },
            "inferenceParams": { "maxTokens": 4096.0, "temperature": 0.5, "topP": 0.9 },
            "thinkingMode": { "type": "enabled", "budget_tokens": 2048 },
            "guardrailSettings": {
                "enabled": true, "guardrailIdentifier": "gr", "guardrailVersion": "1", "trace": "enabled"
            },
            "bedrockSettings": { "enableRegionFailover": true, "availableFailoverRegions": ["us-east-1"] }
        });
        let s = converse_settings(&store);
        assert_eq!(s.aws.region, "eu-west-1");
        assert_eq!(s.aws.profile.as_deref(), Some("dev"));
        assert_eq!(s.aws.use_profile, Some(true));
        assert_eq!(
            s.aws.proxy_config.as_ref().and_then(|p| p.host.as_deref()),
            Some("proxy.local")
        );
        // electron-store keeps JS numbers: 4096.0 still maps to the integer field.
        assert_eq!(s.inference_params.max_tokens, Some(4096));
        assert_eq!(s.inference_params.top_p, Some(0.9));
        assert_eq!(s.thinking_mode.unwrap().budget_tokens, Some(2048));
        assert_eq!(s.guardrail_settings.unwrap().guardrail_identifier, "gr");
        assert_eq!(
            s.bedrock_settings.unwrap().available_failover_regions,
            vec!["us-east-1".to_string()]
        );
        assert_eq!(aws_settings(&store).region, "eu-west-1");
    }

    #[test]
    fn malformed_keys_fall_back_individually() {
        let store = json!({
            "aws": { "region": "us-west-2", "accessKeyId": "", "secretAccessKey": "" },
            "inferenceParams": "not an object",
            "thinkingMode": 42
        });
        let s = converse_settings(&store);
        assert_eq!(s.aws.region, "us-west-2");
        assert_eq!(s.inference_params, Default::default());
        assert_eq!(s.thinking_mode, None);
        assert_eq!(converse_settings(&json!({})), ConverseSettings::default());
    }

    #[test]
    fn attachment_paths_treat_empty_as_unset() {
        let p = attachment_paths(&json!({ "projectPath": "/p", "userDataPath": "" }));
        assert_eq!(p.project_path, Some(PathBuf::from("/p")));
        assert_eq!(p.user_data_path, None);
        assert_eq!(attachment_paths(&json!({})), AttachmentPaths::default());
    }

    #[test]
    fn proxy_url_only_when_enabled_with_host() {
        let enabled = json!({ "aws": { "proxyConfig": {
            "enabled": true, "host": "proxy.local", "port": 3128,
            "username": "u", "password": "p"
        } } });
        assert_eq!(
            proxy_url(&enabled).as_deref(),
            Some("http://u:p@proxy.local:3128/")
        );
        let default_port = json!({ "aws": { "proxyConfig": {
            "enabled": true, "host": "proxy.local", "protocol": "https"
        } } });
        assert_eq!(
            proxy_url(&default_port).as_deref(),
            Some("https://proxy.local:8080/")
        );
        let disabled = json!({ "aws": { "proxyConfig": { "enabled": false, "host": "h" } } });
        assert_eq!(proxy_url(&disabled), None);
        assert_eq!(proxy_url(&json!({})), None);
        // Building the client never fails the call.
        let _ = http_client(&enabled);
    }
}
