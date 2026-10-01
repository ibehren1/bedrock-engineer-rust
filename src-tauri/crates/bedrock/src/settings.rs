//! Settings the converse path reads from the electron-store config.
//!
//! These mirror the store keys read by `src/main/api/bedrock/services/converseService.ts`
//! (`aws`, `inferenceParams`, `thinkingMode`, `interleaveThinking`, `guardrailSettings`,
//! `bedrockSettings`). They are plain serde structs so the app layer can deserialize them straight
//! out of the store JSON without this crate depending on the `store` crate.

use serde::{Deserialize, Serialize};

/// Manual proxy settings (`aws.proxyConfig`), mirroring `ProxyConfiguration` in
/// `src/main/api/bedrock/types.ts`.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfiguration {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// `'http' | 'https'`; defaults to `http` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

/// The `aws` store key, mirroring `AWSCredentials` in `src/main/api/bedrock/types.ts`.
///
/// When `use_profile` is true the named `profile` is read from the shared config/credentials
/// files (the JS `fromIni({ profile })`); otherwise the static key pair is used.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsSettings {
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    #[serde(default)]
    pub region: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_profile: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_config: Option<ProxyConfiguration>,
}

/// Shown in place of secrets in `Debug` output, so settings can be logged safely.
const REDACTED: &str = "<redacted>";

fn redact<T>(v: &Option<T>) -> Option<&'static str> {
    v.as_ref().map(|_| REDACTED)
}

impl std::fmt::Debug for ProxyConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConfiguration")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &redact(&self.password))
            .field("protocol", &self.protocol)
            .finish()
    }
}

impl std::fmt::Debug for AwsSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secret = (!self.secret_access_key.is_empty()).then_some(REDACTED);
        f.debug_struct("AwsSettings")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &secret)
            .field("session_token", &redact(&self.session_token))
            .field("region", &self.region)
            .field("profile", &self.profile)
            .field("use_profile", &self.use_profile)
            .field("proxy_config", &self.proxy_config)
            .finish()
    }
}

impl AwsSettings {
    /// Copy of these settings targeting another region (used by region failover).
    pub fn with_region(&self, region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            ..self.clone()
        }
    }
}

/// `InferenceConfiguration` as the renderer sends it and as `inferenceParams` stores it.
///
/// Every field is optional so "delete inferenceConfig.topP" in the TS maps to `None`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// The `thinkingMode` store key (`{ type: 'enabled' | 'adaptive' | 'disabled', budget_tokens? }`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingMode {
    /// `enabled`, `adaptive` or `disabled`. Kept as a string so unknown values round-trip.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<i64>,
}

/// The `guardrailSettings` store key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub guardrail_identifier: String,
    #[serde(default)]
    pub guardrail_version: String,
    /// `enabled` | `disabled`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
}

/// The `bedrockSettings` store key (only the fields the converse path reads).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockSettings {
    #[serde(default)]
    pub enable_region_failover: bool,
    #[serde(default)]
    pub available_failover_regions: Vec<String>,
}

/// Everything `ConverseService` reads from the store for one request.
///
/// The TS service calls `store.get(...)` at request time; the app layer should build one of these
/// from the store per command invocation so settings changes apply to the next request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverseSettings {
    #[serde(default)]
    pub aws: AwsSettings,
    #[serde(default)]
    pub inference_params: InferenceConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_mode: Option<ThinkingMode>,
    #[serde(default)]
    pub interleave_thinking: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrail_settings: Option<GuardrailSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bedrock_settings: Option<BedrockSettings>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_store_shaped_json() {
        let settings: ConverseSettings = serde_json::from_value(json!({
            "aws": {
                "region": "us-west-2",
                "accessKeyId": "AKIA",
                "secretAccessKey": "secret",
                "useProfile": true,
                "profile": "dev",
                "proxyConfig": { "enabled": true, "host": "proxy", "port": 3128 }
            },
            "inferenceParams": { "maxTokens": 4096, "temperature": 0.5, "topP": 0.9 },
            "thinkingMode": { "type": "enabled", "budget_tokens": 4096 },
            "interleaveThinking": true,
            "guardrailSettings": {
                "enabled": false, "guardrailIdentifier": "", "guardrailVersion": "DRAFT", "trace": "enabled"
            },
            "bedrockSettings": {
                "enableRegionFailover": true,
                "availableFailoverRegions": ["us-east-1"],
                "enableInferenceProfiles": false,
                "visibleModelIds": []
            }
        }))
        .unwrap();
        assert_eq!(settings.aws.profile.as_deref(), Some("dev"));
        assert_eq!(settings.aws.use_profile, Some(true));
        assert_eq!(settings.aws.proxy_config.as_ref().unwrap().port, Some(3128));
        assert_eq!(settings.inference_params.max_tokens, Some(4096));
        assert_eq!(settings.inference_params.top_p, Some(0.9));
        assert_eq!(
            settings.thinking_mode.as_ref().unwrap().kind.as_deref(),
            Some("enabled")
        );
        assert_eq!(settings.thinking_mode.unwrap().budget_tokens, Some(4096));
        assert!(settings.interleave_thinking);
        assert_eq!(
            settings
                .bedrock_settings
                .unwrap()
                .available_failover_regions,
            vec!["us-east-1".to_string()]
        );
    }

    #[test]
    fn store_default_aws_value_deserializes() {
        // `init()` seeds `{ region: 'us-west-2', accessKeyId: '', secretAccessKey: '' }`.
        let aws: AwsSettings = serde_json::from_value(
            json!({ "region": "us-west-2", "accessKeyId": "", "secretAccessKey": "" }),
        )
        .unwrap();
        assert_eq!(aws.region, "us-west-2");
        assert_eq!(aws.use_profile, None);
        assert_eq!(aws.with_region("eu-west-1").region, "eu-west-1");
    }

    #[test]
    fn empty_settings_object_deserializes() {
        let s: ConverseSettings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(s, ConverseSettings::default());
    }

    #[test]
    fn debug_redacts_secrets() {
        let aws = AwsSettings {
            access_key_id: "AKIA".into(),
            secret_access_key: "top-secret".into(),
            session_token: Some("tok-secret".into()),
            proxy_config: Some(ProxyConfiguration {
                password: Some("pw-secret".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = format!("{aws:?}");
        assert!(out.contains("AKIA"));
        assert!(!out.contains("secret\""), "{out}");
        assert!(out.contains(REDACTED));
    }
}
