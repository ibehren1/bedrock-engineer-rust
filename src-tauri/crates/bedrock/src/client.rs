//! AWS client factory — port of `src/main/api/bedrock/client.ts` and the Bedrock parts of
//! `src/main/lib/proxy-utils.ts`.
//!
//! The sign-in method is [`AwsSettings::auth_mode`] (`authMode`, falling back to `useProfile`):
//!
//! * `profile` -> credentials from the named profile in the shared config/credentials
//!   files (JS `fromIni({ profile })`; no profile name -> `AWS_PROFILE` / `default`). Only the
//!   profile file provider is used, not the full default chain, so environment credentials do not
//!   override an explicitly chosen profile — same as `fromIni`.
//! * `accessKey` -> the static access key / secret / session token.
//! * `apiKey` -> the Amazon Bedrock API key as an HTTP bearer token. The SigV4 credentials
//!   provider is one that always refuses, so a SigV4-only service (agent runtime, Translate, S3)
//!   fails before sending anything instead of picking up environment or default-chain
//!   credentials (or going out unsigned); [`require_sigv4`] gives those features a clear error
//!   first.
//!
//! The auth scheme preference is always set explicitly (bearer for `apiKey`, SigV4 otherwise).
//! Set on the shared config it counts as client config, so an `AWS_BEARER_TOKEN_BEDROCK`
//! environment variable can't switch a key- or profile-based setup to bearer auth.
//! * `proxyConfig.enabled && host` -> all traffic through `protocol://host:port` (default
//!   `http`, port 8080) with basic auth when both username and password are set.
//! * timeouts: 30 s connect, 300 s read (`NodeHttpHandler({ connectionTimeout, requestTimeout })`).
//!
//! [`load_sdk_config`] returns a plain `SdkConfig`, so other AWS service crates added later
//! (bedrock control plane, agent runtime, translate, S3) can be built from the same settings.

use crate::error::{Error, Result};
use crate::settings::{AwsAuthMode, AwsSettings, ProxyConfiguration};
use aws_config::profile::ProfileFileCredentialsProvider;
use aws_config::timeout::TimeoutConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::error::CredentialsError;
use aws_credential_types::provider::{ProvideCredentials, SharedCredentialsProvider};
use aws_credential_types::{Credentials, Token};
use aws_smithy_http_client::proxy::ProxyConfig;
use aws_smithy_http_client::tls::{rustls_provider::CryptoMode, Provider};
use aws_smithy_http_client::{Builder as HttpClientBuilder, Connector};
use aws_smithy_runtime_api::client::auth::http::HTTP_BEARER_AUTH_SCHEME_ID;
use aws_smithy_runtime_api::client::auth::AuthSchemeId;
use std::time::Duration;

/// `connectionTimeout: 30000`.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// `requestTimeout: 300000` ("5 minutes for long-running operations").
pub const READ_TIMEOUT: Duration = Duration::from_secs(300);

/// Provider name attached to static credentials.
const STATIC_PROVIDER: &str = "BedrockEngineerStaticCredentials";

/// The SigV4 auth scheme (`aws_runtime::auth::sigv4::SCHEME_ID`).
const SIGV4_AUTH_SCHEME_ID: AuthSchemeId = AuthSchemeId::new("sigv4");

/// Fails with a clear message when `feature` needs SigV4 (AWS access keys or a profile) but the
/// settings sign in with a Bedrock API key, which only the Bedrock runtime and control plane
/// accept.
pub fn require_sigv4(aws: &AwsSettings, feature: &str) -> Result<()> {
    if aws.auth_mode() == AwsAuthMode::ApiKey {
        return Err(Error::plain(format!(
            "{feature} isn't available with a Bedrock API key. Choose AWS access keys or an AWS profile in Settings → AWS Authentication."
        )));
    }
    Ok(())
}

/// Resolved proxy endpoint: URL without credentials plus optional basic auth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyTarget {
    pub url: String,
    pub basic_auth: Option<(String, String)>,
}

/// `resolveProxyConfig` + URL construction from `createProxyAgents`. `Ok(None)` when the proxy
/// is off. A proxy that is on but unusable (no host, a protocol other than http/https, a host
/// that isn't a host name) is an error: the SDK must not quietly connect directly while a proxy
/// is turned on (AGENT-B1 / B5).
pub fn resolve_proxy(
    cfg: Option<&ProxyConfiguration>,
) -> std::result::Result<Option<ProxyTarget>, String> {
    let Some(cfg) = cfg.filter(|c| c.enabled) else {
        return Ok(None);
    };
    let broken = |why: String| {
        Err(format!(
            "A proxy is turned on but {why}, so the app won't connect to AWS (it never connects directly while a proxy is on). Fix it in Settings → AWS Authentication → Proxy Settings, or turn the proxy off."
        ))
    };
    let Some(host) = cfg.host.as_deref().map(str::trim).filter(|h| !h.is_empty()) else {
        return broken("has no host".into());
    };
    let protocol = cfg
        .protocol
        .as_deref()
        .filter(|p| !p.is_empty())
        .unwrap_or("http");
    if !matches!(protocol, "http" | "https") {
        return broken(format!("its protocol \"{protocol}\" isn't http or https"));
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let ipv6 = bare.parse::<std::net::Ipv6Addr>().is_ok();
    if host.contains(['/', '@', '#', '?', '\\', ' ', '\t', '\n', '\r'])
        || (!ipv6 && host.contains(':'))
    {
        return broken(format!("its host \"{host}\" isn't a valid host name"));
    }
    let host = if ipv6 {
        format!("[{bare}]")
    } else {
        host.to_string()
    };
    let port = cfg.port.filter(|p| *p != 0).unwrap_or(8080);
    let basic_auth = match (cfg.username.as_deref(), cfg.password.as_deref()) {
        (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => {
            Some((u.to_string(), p.to_string()))
        }
        _ => None,
    };
    Ok(Some(ProxyTarget {
        url: format!("{protocol}://{host}:{port}"),
        basic_auth,
    }))
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

/// SigV4 credentials provider for the given settings. In API key mode it is [`NoSigV4`], which
/// refuses every request.
pub fn credentials_provider(aws: &AwsSettings) -> SharedCredentialsProvider {
    match aws.auth_mode() {
        AwsAuthMode::ApiKey => SharedCredentialsProvider::new(NoSigV4),
        AwsAuthMode::Profile => profile_credentials(aws),
        AwsAuthMode::AccessKey => SharedCredentialsProvider::new(Credentials::new(
            aws.access_key_id.clone(),
            aws.secret_access_key.clone(),
            aws.session_token.clone().filter(|t| !t.is_empty()),
            None,
            STATIC_PROVIDER,
        )),
    }
}

/// The SigV4 credentials provider in API key mode: always an error, so a service that can't
/// take the bearer token fails locally rather than sending an unsigned request.
#[derive(Debug)]
pub struct NoSigV4;

impl ProvideCredentials for NoSigV4 {
    fn provide_credentials<'a>(
        &'a self,
    ) -> aws_credential_types::provider::future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        aws_credential_types::provider::future::ProvideCredentials::ready(Err(
            CredentialsError::invalid_configuration(
                "this AWS service needs AWS access keys or an AWS profile; the Bedrock API key only works for Amazon Bedrock",
            ),
        ))
    }
}

fn profile_credentials(aws: &AwsSettings) -> SharedCredentialsProvider {
    let mut builder = ProfileFileCredentialsProvider::builder();
    if let Some(profile) = aws.profile.as_deref().filter(|p| !p.is_empty()) {
        builder = builder.profile_name(profile);
    }
    SharedCredentialsProvider::new(builder.build())
}

/// Build an `SdkConfig` from the `aws` store settings.
pub async fn load_sdk_config(aws: &AwsSettings) -> Result<SdkConfig> {
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(aws.region.clone()))
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(READ_TIMEOUT)
                .build(),
        );
    loader = loader.credentials_provider(credentials_provider(aws));
    loader = if aws.auth_mode() == AwsAuthMode::ApiKey {
        let key = aws.api_key().ok_or_else(|| {
            Error::Config(
                "no Bedrock API key is set. Enter one in Settings → AWS Authentication, or choose AWS access keys or an AWS profile.".into(),
            )
        })?;
        loader
            .token_provider(Token::new(key.to_string(), None))
            .auth_scheme_preference([HTTP_BEARER_AUTH_SCHEME_ID])
    } else {
        loader.auth_scheme_preference([SIGV4_AUTH_SCHEME_ID])
    };
    if let Some(target) = resolve_proxy(aws.proxy_config.as_ref()).map_err(Error::Config)? {
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

    fn proxy(enabled: bool, host: Option<&str>) -> ProxyConfiguration {
        ProxyConfiguration {
            enabled,
            host: host.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn proxy_off_is_none_and_unusable_is_an_error() {
        assert_eq!(resolve_proxy(None), Ok(None));
        assert_eq!(resolve_proxy(Some(&proxy(false, Some("p")))), Ok(None));
        // AGENT-B5: on but unusable never means "connect directly".
        assert!(resolve_proxy(Some(&proxy(true, None))).is_err());
        assert!(resolve_proxy(Some(&proxy(true, Some("")))).is_err());
        // AGENT-B1: only http/https and real host names.
        let mut socks = proxy(true, Some("p"));
        socks.protocol = Some("socks5".into());
        assert!(resolve_proxy(Some(&socks)).is_err());
        for bad in ["evil.com/@x", "a b", "host:99"] {
            assert!(
                resolve_proxy(Some(&proxy(true, Some(bad)))).is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            resolve_proxy(Some(&proxy(true, Some("::1"))))
                .unwrap()
                .unwrap()
                .url,
            "http://[::1]:8080"
        );
    }

    #[test]
    fn proxy_defaults_protocol_and_port() {
        assert_eq!(
            resolve_proxy(Some(&proxy(true, Some("proxy.corp")))).unwrap(),
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
        assert_eq!(resolve_proxy(Some(&cfg)).unwrap().unwrap().basic_auth, None);
        cfg.password = Some("pw".into());
        assert_eq!(
            resolve_proxy(Some(&cfg)).unwrap(),
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

    /// Sends one Bedrock Runtime and one S3 request with the settings' config over a fake HTTP
    /// client; returns the `Authorization` headers seen (`None` when the call failed before
    /// sending).
    async fn authorization_headers(aws: &AwsSettings) -> (Option<String>, Option<String>) {
        use aws_config::retry::RetryConfig;
        use aws_smithy_http_client::test_util::infallible_client_fn;
        use aws_smithy_types::body::SdkBody;
        use std::sync::{Arc, Mutex};

        let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
        let record = seen.clone();
        let http_client = infallible_client_fn(move |req: http::Request<SdkBody>| {
            let auth = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string();
            record
                .lock()
                .unwrap()
                .push((req.uri().host().unwrap_or_default().to_string(), auth));
            http::Response::builder()
                .status(200)
                .body(SdkBody::from(r#"{"output":{"message":{"role":"assistant","content":[]}},"stopReason":"end_turn","usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2},"metrics":{"latencyMs":1}}"#))
                .unwrap()
        });
        let conf = load_sdk_config(aws)
            .await
            .unwrap()
            .to_builder()
            .retry_config(RetryConfig::disabled())
            .http_client(http_client)
            .build();
        let _ = aws_sdk_bedrockruntime::Client::new(&conf)
            .converse()
            .model_id("m")
            .send()
            .await;
        let _ = aws_sdk_s3::Client::new(&conf)
            .get_object()
            .bucket("b")
            .key("k")
            .send()
            .await;
        let seen = seen.lock().unwrap().clone();
        let find = |svc: &str| {
            seen.iter()
                .find(|(host, _)| host.contains(svc))
                .map(|(_, a)| a.clone())
        };
        (find("bedrock-runtime"), find("s3"))
    }

    #[tokio::test]
    async fn api_key_mode_sends_bearer_and_no_sigv4() {
        let aws = AwsSettings {
            region: "us-east-1".into(),
            auth_mode: Some(AwsAuthMode::ApiKey),
            bedrock_api_key: Some(" bedrock-api-key ".into()),
            // Keys left over from another mode must not be used.
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "secret".into(),
            ..Default::default()
        };
        assert!(credentials_provider(&aws)
            .provide_credentials()
            .await
            .is_err());
        let cfg = load_sdk_config(&aws).await.unwrap();
        assert!(cfg.token_provider().is_some());
        let (runtime, s3) = authorization_headers(&aws).await;
        assert_eq!(runtime.as_deref(), Some("Bearer bedrock-api-key"));
        // S3 is SigV4-only: the refusing credentials stop the call before it goes out.
        assert_eq!(s3, None);
    }

    #[tokio::test]
    async fn access_key_mode_signs_with_sigv4() {
        let aws = AwsSettings {
            region: "us-east-1".into(),
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "secret".into(),
            // A saved API key is ignored outside API key mode.
            bedrock_api_key: Some("bedrock-api-key".into()),
            ..Default::default()
        };
        let (runtime, s3) = authorization_headers(&aws).await;
        assert!(
            runtime.as_deref().unwrap().starts_with("AWS4-HMAC-SHA256"),
            "{runtime:?}"
        );
        assert!(s3.unwrap().starts_with("AWS4-HMAC-SHA256"));
        let cfg = load_sdk_config(&aws).await.unwrap();
        assert_eq!(
            cfg.auth_scheme_preference()
                .unwrap()
                .clone()
                .into_iter()
                .collect::<Vec<_>>(),
            vec![SIGV4_AUTH_SCHEME_ID]
        );
    }

    #[tokio::test]
    async fn api_key_mode_without_a_key_is_a_config_error() {
        for key in [None, Some(String::new()), Some("   ".into())] {
            let aws = AwsSettings {
                region: "us-east-1".into(),
                auth_mode: Some(AwsAuthMode::ApiKey),
                bedrock_api_key: key,
                ..Default::default()
            };
            let err = load_sdk_config(&aws).await.unwrap_err();
            assert!(matches!(err, Error::Config(_)), "{err:?}");
            assert!(err.to_string().contains("Bedrock API key"), "{err}");
        }
    }

    #[test]
    fn require_sigv4_rejects_only_api_key_mode() {
        let mut aws = AwsSettings::default();
        assert!(require_sigv4(&aws, "Amazon Translate").is_ok());
        aws.use_profile = Some(true);
        assert!(require_sigv4(&aws, "Amazon Translate").is_ok());
        aws.auth_mode = Some(AwsAuthMode::ApiKey);
        let err = require_sigv4(&aws, "Amazon Translate").unwrap_err();
        assert_eq!(
            err.message(),
            "Amazon Translate isn't available with a Bedrock API key. Choose AWS access keys or an AWS profile in Settings → AWS Authentication."
        );
    }
}
