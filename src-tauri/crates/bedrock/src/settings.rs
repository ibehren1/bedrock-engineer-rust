//! Settings the converse path reads from the electron-store config.
//!
//! These mirror the store keys read by `src/main/api/bedrock/services/converseService.ts`
//! (`aws`, `inferenceParams`, `thinkingMode`, `guardrailSettings`,
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

/// How the app signs in to AWS (`aws.authMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AwsAuthMode {
    /// The static access key / secret / session token.
    AccessKey,
    /// A named profile from the shared config/credentials files.
    Profile,
    /// An Amazon Bedrock API key (HTTP bearer token). Only the Bedrock runtime and control
    /// plane accept it; SigV4-only services (agent runtime, Translate, S3) are unavailable.
    ApiKey,
}

/// `authMode` as stored: a known value, or `None` for absent, `null`, or anything this build
/// doesn't know (a newer build's mode must not make the whole `aws` key unreadable).
fn lenient_auth_mode<'de, D>(d: D) -> std::result::Result<Option<AwsAuthMode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
}

/// The `aws` store key, mirroring `AWSCredentials` in `src/types/aws.ts`.
///
/// [`AwsSettings::auth_mode`] picks the sign-in method: `authMode` when set, else the older
/// `useProfile` flag (true -> the named `profile` from the shared config/credentials files,
/// otherwise the static key pair).
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
    #[serde(
        default,
        deserialize_with = "lenient_auth_mode",
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_mode: Option<AwsAuthMode>,
    /// Amazon Bedrock API key (store key `bedrockApiKey`), used in [`AwsAuthMode::ApiKey`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bedrock_api_key: Option<String>,
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
            .field("auth_mode", &self.auth_mode)
            .field("bedrock_api_key", &redact(&self.bedrock_api_key))
            .field("proxy_config", &self.proxy_config)
            .finish()
    }
}

impl AwsSettings {
    /// The sign-in method: `authMode` when set, else `useProfile: true` -> profile, else
    /// access key (configs written before `authMode` existed).
    pub fn auth_mode(&self) -> AwsAuthMode {
        match self.auth_mode {
            Some(mode) => mode,
            None if self.use_profile == Some(true) => AwsAuthMode::Profile,
            None => AwsAuthMode::AccessKey,
        }
    }

    /// The Bedrock API key, trimmed, when one is set.
    pub fn api_key(&self) -> Option<&str> {
        self.bedrock_api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
    }

    /// Whether the settings name credentials for the chosen method (region aside): a profile
    /// (an empty name means the default one), an access key id, or a non-empty API key.
    pub fn has_credentials(&self) -> bool {
        match self.auth_mode() {
            AwsAuthMode::Profile => true,
            AwsAuthMode::AccessKey => !self.access_key_id.is_empty(),
            AwsAuthMode::ApiKey => self.api_key().is_some(),
        }
    }

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
        let with_key = AwsSettings {
            bedrock_api_key: Some("bedrock-key-secret".into()),
            ..Default::default()
        };
        assert!(!format!("{with_key:?}").contains("bedrock-key-secret"));
        assert!(!out.contains("secret\""), "{out}");
        assert!(out.contains(REDACTED));
    }

    #[test]
    fn auth_mode_falls_back_to_use_profile() {
        let old: AwsSettings =
            serde_json::from_value(json!({ "useProfile": true, "profile": "dev" })).unwrap();
        assert_eq!(old.auth_mode, None);
        assert_eq!(old.auth_mode(), AwsAuthMode::Profile);
        let empty: AwsSettings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(empty.auth_mode(), AwsAuthMode::AccessKey);
        let no_profile: AwsSettings =
            serde_json::from_value(json!({ "useProfile": false })).unwrap();
        assert_eq!(no_profile.auth_mode(), AwsAuthMode::AccessKey);
        // The explicit value wins over `useProfile`.
        let key: AwsSettings = serde_json::from_value(
            json!({ "useProfile": true, "authMode": "apiKey", "bedrockApiKey": "k" }),
        )
        .unwrap();
        assert_eq!(key.auth_mode(), AwsAuthMode::ApiKey);
        assert_eq!(key.bedrock_api_key.as_deref(), Some("k"));
        for (raw, mode) in [
            ("accessKey", AwsAuthMode::AccessKey),
            ("profile", AwsAuthMode::Profile),
            ("apiKey", AwsAuthMode::ApiKey),
        ] {
            let s: AwsSettings = serde_json::from_value(json!({ "authMode": raw })).unwrap();
            assert_eq!(s.auth_mode(), mode);
            assert_eq!(serde_json::to_value(&s).unwrap()["authMode"], raw);
        }
    }

    #[test]
    fn unknown_auth_mode_does_not_break_the_aws_key() {
        for raw in [json!("sso"), json!(null), json!(3)] {
            let s: AwsSettings = serde_json::from_value(
                json!({ "region": "us-east-1", "useProfile": true, "authMode": raw }),
            )
            .unwrap();
            assert_eq!(s.region, "us-east-1");
            assert_eq!(s.auth_mode, None);
            assert_eq!(s.auth_mode(), AwsAuthMode::Profile);
        }
    }

    #[test]
    fn has_credentials_per_mode() {
        let mut s = AwsSettings::default();
        assert!(!s.has_credentials());
        s.access_key_id = "AKID".into();
        assert!(s.has_credentials());
        s.auth_mode = Some(AwsAuthMode::ApiKey);
        assert!(!s.has_credentials());
        s.bedrock_api_key = Some("  ".into());
        assert!(!s.has_credentials());
        s.bedrock_api_key = Some("key".into());
        assert!(s.has_credentials());
        s.auth_mode = Some(AwsAuthMode::Profile);
        assert!(s.has_credentials());
    }
}
