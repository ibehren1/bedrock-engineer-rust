//! AWS client factory — port of `src/main/api/bedrock/client.ts` and the Bedrock parts of
//! `src/main/lib/proxy-utils.ts`.
//!
//! * `useProfile: true` -> credentials from the named profile in the shared config/credentials
//!   files (JS `fromIni({ profile })`; no profile name -> `AWS_PROFILE` / `default`). Only the
//!   profile file provider is used, not the full default chain, so environment credentials do not
//!   override an explicitly chosen profile — same as `fromIni`.
//! * otherwise -> the static access key / secret / session token.
//! * `proxyConfig.enabled && host` -> all traffic through `protocol://host:port` (default
//!   `http`, port 8080) with basic auth when both username and password are set.
//! * timeouts: 30 s connect, 300 s read (`NodeHttpHandler({ connectionTimeout, requestTimeout })`).
//!
//! The TS app has no Bedrock API key / bearer-token auth path, so none is implemented here.
//!
//! [`load_sdk_config`] returns a plain `SdkConfig`, so other AWS service crates added later
//! (bedrock control plane, agent runtime, translate, S3) can be built from the same settings.

use crate::error::{Error, Result};
use crate::settings::{AwsSettings, ProxyConfiguration};
use aws_config::profile::ProfileFileCredentialsProvider;
use aws_config::timeout::TimeoutConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials;
use aws_smithy_http_client::proxy::ProxyConfig;
use aws_smithy_http_client::tls::{rustls_provider::CryptoMode, Provider};
use aws_smithy_http_client::{Builder as HttpClientBuilder, Connector};
use std::time::Duration;

/// `connectionTimeout: 30000`.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// `requestTimeout: 300000` ("5 minutes for long-running operations").
pub const READ_TIMEOUT: Duration = Duration::from_secs(300);

/// Provider name attached to static credentials.
const STATIC_PROVIDER: &str = "BedrockEngineerStaticCredentials";

/// Resolved proxy endpoint: URL without credentials plus optional basic auth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyTarget {
    pub url: String,
    pub basic_auth: Option<(String, String)>,
}

/// `resolveProxyConfig` + URL construction from `createProxyAgents`. `None` when the proxy is
/// disabled or has no host.
pub fn resolve_proxy(cfg: Option<&ProxyConfiguration>) -> Option<ProxyTarget> {
    let cfg = cfg?;
    let host = cfg.host.as_deref().filter(|h| !h.is_empty())?;
    if !cfg.enabled {
        return None;
    }
    let protocol = cfg
        .protocol
        .as_deref()
        .filter(|p| !p.is_empty())
        .unwrap_or("http");
    let port = cfg.port.filter(|p| *p != 0).unwrap_or(8080);
    let basic_auth = match (cfg.username.as_deref(), cfg.password.as_deref()) {
        (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => {
            Some((u.to_string(), p.to_string()))
        }
        _ => None,
    };
    Some(ProxyTarget {
        url: format!("{protocol}://{host}:{port}"),
        basic_auth,
    })
}

fn proxied_http_client(
    target: &ProxyTarget,
) -> Result<aws_sdk_bedrockruntime::config::SharedHttpClient> {
    let mut proxy = ProxyConfig::all(target.url.as_str())
        .map_err(|e| Error::Config(format!("invalid proxy {}: {e}", target.url)))?;
    if let Some((user, pass)) = &target.basic_auth {
        proxy = proxy.with_basic_auth(user.clone(), pass.clone());
    }
    // `Builder` has no proxy setter, so build the connector ourselves (same wiring as the
    // builder's own `build_https`, plus the proxy).
    Ok(
        HttpClientBuilder::new().build_with_connector_fn(move |settings, components| {
            let mut builder = Connector::builder().proxy_config(proxy.clone());
            builder.set_connector_settings(settings.cloned());
            if let Some(components) = components {
                builder.set_sleep_impl(components.sleep_impl());
            }
            builder
                .tls_provider(Provider::Rustls(CryptoMode::AwsLc))
                .build()
        }),
    )
}

/// Credentials provider for the given settings.
pub fn credentials_provider(aws: &AwsSettings) -> SharedCredentialsProvider {
    if aws.use_profile.unwrap_or(false) {
        let mut builder = ProfileFileCredentialsProvider::builder();
        if let Some(profile) = aws.profile.as_deref().filter(|p| !p.is_empty()) {
            builder = builder.profile_name(profile);
        }
        SharedCredentialsProvider::new(builder.build())
    } else {
        SharedCredentialsProvider::new(Credentials::new(
            aws.access_key_id.clone(),
            aws.secret_access_key.clone(),
            aws.session_token.clone().filter(|t| !t.is_empty()),
            None,
            STATIC_PROVIDER,
        ))
    }
}

/// Build an `SdkConfig` from the `aws` store settings.
pub async fn load_sdk_config(aws: &AwsSettings) -> Result<SdkConfig> {
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(aws.region.clone()))
        .credentials_provider(credentials_provider(aws))
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(READ_TIMEOUT)
                .build(),
        );
    if let Some(target) = resolve_proxy(aws.proxy_config.as_ref()) {
        tracing::debug!(
            has_auth = target.basic_auth.is_some(),
            "Using manual proxy configuration"
        );
        loader = loader.http_client(proxied_http_client(&target)?);
    }
    Ok(loader.load().await)
}

/// `createRuntimeClient`: a Bedrock Runtime client for the settings.
pub async fn runtime_client(aws: &AwsSettings) -> Result<aws_sdk_bedrockruntime::Client> {
    Ok(aws_sdk_bedrockruntime::Client::new(
        &load_sdk_config(aws).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::provider::ProvideCredentials;

    fn proxy(enabled: bool, host: Option<&str>) -> ProxyConfiguration {
        ProxyConfiguration {
            enabled,
            host: host.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn proxy_disabled_or_hostless_is_none() {
        assert_eq!(resolve_proxy(None), None);
        assert_eq!(resolve_proxy(Some(&proxy(false, Some("p")))), None);
        assert_eq!(resolve_proxy(Some(&proxy(true, None))), None);
        assert_eq!(resolve_proxy(Some(&proxy(true, Some("")))), None);
    }

    #[test]
    fn proxy_defaults_protocol_and_port() {
        assert_eq!(
            resolve_proxy(Some(&proxy(true, Some("proxy.corp")))),
            Some(ProxyTarget {
                url: "http://proxy.corp:8080".into(),
                basic_auth: None
            })
        );
    }

    #[test]
    fn proxy_with_auth_requires_both_parts() {
        let mut cfg = proxy(true, Some("p"));
        cfg.protocol = Some("https".into());
        cfg.port = Some(3128);
        cfg.username = Some("u".into());
        assert_eq!(resolve_proxy(Some(&cfg)).unwrap().basic_auth, None);
        cfg.password = Some("pw".into());
        assert_eq!(
            resolve_proxy(Some(&cfg)),
            Some(ProxyTarget {
                url: "https://p:3128".into(),
                basic_auth: Some(("u".into(), "pw".into()))
            })
        );
    }

    #[tokio::test]
    async fn static_credentials_are_used_verbatim() {
        let aws = AwsSettings {
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "secret".into(),
            session_token: Some("token".into()),
            region: "us-west-2".into(),
            ..Default::default()
        };
        let creds = credentials_provider(&aws)
            .provide_credentials()
            .await
            .unwrap();
        assert_eq!(creds.access_key_id(), "AKIDEXAMPLE");
        assert_eq!(creds.secret_access_key(), "secret");
        assert_eq!(creds.session_token(), Some("token"));
    }

    #[tokio::test]
    async fn empty_session_token_is_dropped() {
        let aws = AwsSettings {
            access_key_id: "a".into(),
            secret_access_key: "b".into(),
            session_token: Some(String::new()),
            ..Default::default()
        };
        let creds = credentials_provider(&aws)
            .provide_credentials()
            .await
            .unwrap();
        assert_eq!(creds.session_token(), None);
    }

    #[tokio::test]
    async fn sdk_config_carries_region_and_timeouts() {
        let aws = AwsSettings {
            access_key_id: "a".into(),
            secret_access_key: "b".into(),
            region: "eu-central-1".into(),
            proxy_config: Some(proxy(true, Some("127.0.0.1"))),
            ..Default::default()
        };
        let cfg = load_sdk_config(&aws).await.unwrap();
        assert_eq!(cfg.region().map(|r| r.as_ref()), Some("eu-central-1"));
        let t = cfg.timeout_config().unwrap();
        assert_eq!(t.connect_timeout(), Some(CONNECT_TIMEOUT));
        assert_eq!(t.read_timeout(), Some(READ_TIMEOUT));
        assert!(cfg.http_client().is_some());
    }
}
