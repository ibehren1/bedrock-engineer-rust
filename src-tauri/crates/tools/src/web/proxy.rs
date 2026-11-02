//! The configured proxy for the app's own HTTP clients (AGENT-B1 / B5): when a proxy is turned
//! on, every connection goes through it or fails. Nothing falls back to connecting directly: a
//! proxy that is on but unusable (no host, an unsupported protocol, a malformed host) makes every
//! request fail with the reason instead.

use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::context::ProxyConfig;

/// What the proxy setting means for a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxySetting {
    /// No proxy turned on: connect directly.
    Off,
    /// Connect only through this proxy URL (credentials included).
    On(String),
    /// A proxy is turned on but can't be used: connect nowhere, with this reason.
    Broken(String),
}

/// Characters that can't appear in a proxy host name (they would change the URL's meaning).
const BAD_HOST_CHARS: &[char] = &['/', '@', '#', '?', '\\', ' ', '\t', '\n', '\r'];

impl ProxyConfig {
    /// [`ProxySetting`] for this configuration, validated (AGENT-B1).
    pub fn setting(&self) -> ProxySetting {
        if !self.enabled {
            return ProxySetting::Off;
        }
        let broken = |why: &str| {
            ProxySetting::Broken(format!(
                "A proxy is turned on but {why}, so the app won't connect (it never connects directly while a proxy is on). Fix it in Settings → AWS → Proxy Settings, or turn the proxy off."
            ))
        };
        let Some(host) = self
            .host
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
        else {
            return broken("has no host");
        };
        let protocol = self.protocol.as_deref().unwrap_or("http");
        if !matches!(protocol, "http" | "https") {
            return broken(&format!("its protocol \"{protocol}\" isn't http or https"));
        }
        let bare = host.trim_start_matches('[').trim_end_matches(']');
        let ipv6 = bare.parse::<std::net::Ipv6Addr>().is_ok();
        if host.contains(BAD_HOST_CHARS) || (!ipv6 && host.contains(':')) {
            return broken(&format!("its host \"{host}\" isn't a valid host name"));
        }
        match self.proxy_url() {
            Some(url) => ProxySetting::On(url),
            None => broken(&format!("\"{host}\" doesn't make a valid proxy address")),
        }
    }
}

/// A resolver that fails every lookup with `reason`.
struct Refuse(String);

impl Resolve for Refuse {
    fn resolve(&self, _name: Name) -> Resolving {
        let reason = self.0.clone();
        Box::pin(async move { Err::<Addrs, _>(reason.into()) })
    }
}

/// Host used as the "proxy" of a client that must not connect; it never resolves (`.invalid`) and
/// lookups go through [`Refuse`] anyway, so every request fails with the reason.
const NOWHERE: &str = "http://proxy-is-misconfigured.invalid:1";

/// `builder` set up for `proxy`: through it when on; unchanged when off; and when it is on but
/// unusable, a client whose every request fails with the reason (literal IP URLs included, since
/// they would have to reach the unresolvable proxy first).
pub fn proxied(
    builder: reqwest::ClientBuilder,
    proxy: Option<&ProxyConfig>,
) -> reqwest::ClientBuilder {
    match proxy.map(ProxyConfig::setting).unwrap_or(ProxySetting::Off) {
        ProxySetting::Off => builder,
        ProxySetting::On(url) => match reqwest::Proxy::all(&url) {
            Ok(p) => builder.proxy(p),
            Err(e) => refuse(
                builder,
                format!("The proxy address isn't usable ({e}); not connecting directly."),
            ),
        },
        ProxySetting::Broken(reason) => refuse(builder, reason),
    }
}

/// `builder` made into a client whose every request fails with `reason`.
pub fn refuse(builder: reqwest::ClientBuilder, reason: String) -> reqwest::ClientBuilder {
    let nowhere = reqwest::Proxy::all(NOWHERE).expect("static proxy URL");
    builder
        .proxy(nowhere)
        .dns_resolver(Arc::new(Refuse(reason)))
}

/// The error for `proxy` when it is on but unusable, if so (for callers that can refuse before
/// building anything).
pub fn broken_reason(proxy: Option<&ProxyConfig>) -> Option<String> {
    match proxy.map(ProxyConfig::setting) {
        Some(ProxySetting::Broken(reason)) => Some(reason),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, host: Option<&str>, protocol: Option<&str>) -> ProxyConfig {
        ProxyConfig {
            enabled,
            host: host.map(str::to_string),
            port: Some(3128),
            protocol: protocol.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn settings_are_validated() {
        assert_eq!(cfg(false, None, None).setting(), ProxySetting::Off);
        assert_eq!(
            cfg(true, Some("proxy.corp"), None).setting(),
            ProxySetting::On("http://proxy.corp:3128/".into())
        );
        assert!(matches!(
            cfg(true, Some("::1"), None).setting(),
            ProxySetting::On(_)
        ));
        for bad in [
            cfg(true, None, None),
            cfg(true, Some("  "), None),
            cfg(true, Some("proxy.corp"), Some("socks5")),
            cfg(true, Some("evil.com/@x"), None),
            cfg(true, Some("a b"), None),
            cfg(true, Some("host:99"), None),
        ] {
            assert!(
                matches!(bad.setting(), ProxySetting::Broken(_)),
                "{:?}",
                bad.host
            );
        }
    }

    // AGENT-B5: a proxy that is on but unusable never means "connect directly".
    #[tokio::test]
    async fn a_broken_proxy_fails_every_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let broken = cfg(true, None, None);
        let client = proxied(reqwest::Client::builder(), Some(&broken))
            .build()
            .unwrap();
        for url in [format!("http://{addr}/"), "http://example.com/".to_string()] {
            let err = client.get(&url).send().await.unwrap_err();
            let mut msg = err.to_string();
            let mut source = std::error::Error::source(&err);
            while let Some(s) = source {
                msg = s.to_string();
                source = s.source();
            }
            assert!(msg.contains("proxy is turned on"), "{url}: {msg}");
        }
        // Nothing reached the local server directly.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    }
}
