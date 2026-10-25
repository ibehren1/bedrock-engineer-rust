//! `{{awsRegion}}` / `{{awsProfile}}` in a stdio server's `env`, filled in from the app's AWS
//! settings when the server starts.
//!
//! Opt-in per server: a server without these placeholders gets none of the app's credentials.
//! With a named profile, `{{awsProfile}}` becomes the profile name. With static keys, the entry
//! whose value is `{{awsProfile}}` is dropped and the key pair (and session token, if any) is
//! added in its place. Every entry a placeholder produced is *forced*: it wins over the app's own
//! process environment, which otherwise overrides the configured `env`.

use common::agent::McpServerConfig;
use indexmap::IndexMap;

const REGION: &str = "{{awsRegion}}";
const PROFILE: &str = "{{awsProfile}}";

/// The app's AWS settings, as far as MCP servers need them.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct AppAwsEnv {
    pub region: String,
    /// The named profile, when the app uses one.
    pub profile: Option<String>,
    /// Static credentials, when the app uses an access key pair instead of a profile.
    pub keys: Option<AwsKeys>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct AwsKeys {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

/// Source of [`AppAwsEnv`], read each time servers are (re)initialized.
pub trait AwsEnvSource: Send + Sync {
    fn aws_env(&self) -> AppAwsEnv;
}

/// A server config with its placeholders filled in, plus the `env` keys they produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedServer {
    pub config: McpServerConfig,
    pub forced: Vec<String>,
}

fn has_placeholder(server: &McpServerConfig) -> bool {
    server
        .env
        .iter()
        .flatten()
        .any(|(_, v)| v.contains(REGION) || v.contains(PROFILE))
}

/// Fill in `server`'s AWS placeholders from `aws`. Servers without any are returned unchanged.
pub fn expand_server_env(server: &McpServerConfig, aws: &AppAwsEnv) -> ExpandedServer {
    let Some(env) = server.env.as_ref().filter(|_| has_placeholder(server)) else {
        return ExpandedServer {
            config: server.clone(),
            forced: Vec::new(),
        };
    };
    let mut out = IndexMap::new();
    let mut forced = Vec::new();
    let mut add_keys = false;
    for (k, v) in env {
        if !(v.contains(REGION) || v.contains(PROFILE)) {
            out.insert(k.clone(), v.clone());
            continue;
        }
        if v.trim() == PROFILE && aws.profile.is_none() {
            add_keys = aws.keys.is_some();
            continue;
        }
        let value = v
            .replace(REGION, &aws.region)
            .replace(PROFILE, aws.profile.as_deref().unwrap_or_default());
        out.insert(k.clone(), value);
        forced.push(k.clone());
    }
    if let (true, Some(keys)) = (add_keys, &aws.keys) {
        let mut put = |k: &str, v: &str| {
            out.insert(k.into(), v.into());
            forced.push(k.into());
        };
        put("AWS_ACCESS_KEY_ID", &keys.access_key_id);
        put("AWS_SECRET_ACCESS_KEY", &keys.secret_access_key);
        if let Some(token) = keys.session_token.as_deref().filter(|t| !t.is_empty()) {
            put("AWS_SESSION_TOKEN", token);
        }
    }
    let mut config = server.clone();
    config.env = Some(out);
    ExpandedServer { config, forced }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(env: &[(&str, &str)]) -> McpServerConfig {
        serde_json::from_value(serde_json::json!({
            "name": "pricing",
            "description": "",
            "command": "uvx",
            "args": ["awslabs.aws-pricing-mcp-server@latest"],
            "env": env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<IndexMap<_, _>>(),
        }))
        .unwrap()
    }

    fn env_of(e: &ExpandedServer) -> Vec<(String, String)> {
        e.config.env.clone().unwrap_or_default().into_iter().collect()
    }

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    const AWS_ENV: &[(&str, &str)] = &[
        ("FASTMCP_LOG_LEVEL", "ERROR"),
        ("AWS_PROFILE", "{{awsProfile}}"),
        ("AWS_REGION", "{{awsRegion}}"),
    ];

    #[test]
    fn profile_mode_fills_profile_and_region() {
        let aws = AppAwsEnv {
            region: "eu-west-1".into(),
            profile: Some("work".into()),
            keys: None,
        };
        let e = expand_server_env(&server(AWS_ENV), &aws);
        assert_eq!(
            env_of(&e),
            pairs(&[
                ("FASTMCP_LOG_LEVEL", "ERROR"),
                ("AWS_PROFILE", "work"),
                ("AWS_REGION", "eu-west-1"),
            ])
        );
        assert_eq!(e.forced, vec!["AWS_PROFILE", "AWS_REGION"]);
    }

    #[test]
    fn key_mode_swaps_profile_for_the_key_pair() {
        let aws = AppAwsEnv {
            region: "us-east-1".into(),
            profile: None,
            keys: Some(AwsKeys {
                access_key_id: "AKIA".into(),
                secret_access_key: "secret".into(),
                session_token: Some("token".into()),
            }),
        };
        let e = expand_server_env(&server(AWS_ENV), &aws);
        assert_eq!(
            env_of(&e),
            pairs(&[
                ("FASTMCP_LOG_LEVEL", "ERROR"),
                ("AWS_REGION", "us-east-1"),
                ("AWS_ACCESS_KEY_ID", "AKIA"),
                ("AWS_SECRET_ACCESS_KEY", "secret"),
                ("AWS_SESSION_TOKEN", "token"),
            ])
        );
        assert_eq!(
            e.forced,
            vec![
                "AWS_REGION",
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
                "AWS_SESSION_TOKEN"
            ]
        );
    }

    #[test]
    fn key_mode_without_token_omits_it() {
        let aws = AppAwsEnv {
            region: "us-east-1".into(),
            profile: None,
            keys: Some(AwsKeys {
                access_key_id: "AKIA".into(),
                secret_access_key: "secret".into(),
                session_token: None,
            }),
        };
        let e = expand_server_env(&server(AWS_ENV), &aws);
        assert!(!e.forced.iter().any(|k| k == "AWS_SESSION_TOKEN"));
        assert!(e.forced.iter().any(|k| k == "AWS_ACCESS_KEY_ID"));
    }

    #[test]
    fn region_only_does_not_pass_keys() {
        let aws = AppAwsEnv {
            region: "us-west-2".into(),
            profile: None,
            keys: Some(AwsKeys {
                access_key_id: "AKIA".into(),
                secret_access_key: "secret".into(),
                session_token: None,
            }),
        };
        let e = expand_server_env(&server(&[("AWS_REGION", "{{awsRegion}}")]), &aws);
        assert_eq!(env_of(&e), pairs(&[("AWS_REGION", "us-west-2")]));
    }

    #[test]
    fn servers_without_placeholders_are_unchanged() {
        let aws = AppAwsEnv {
            region: "us-east-1".into(),
            profile: Some("work".into()),
            keys: None,
        };
        let s = server(&[("FASTMCP_LOG_LEVEL", "ERROR")]);
        let e = expand_server_env(&s, &aws);
        assert_eq!(e.config, s);
        assert!(e.forced.is_empty());
    }
}
