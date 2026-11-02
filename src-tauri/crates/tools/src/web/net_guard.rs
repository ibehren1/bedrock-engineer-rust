//! Which addresses the app's web requests may reach (TOOLS-4, DATA-1).
//!
//! A web fetch made for the model runs from the user's machine, so a prompt-injected model can aim
//! it at addresses only that machine reaches. Two levels:
//!
//! * Always refused: link-local addresses (169.254.0.0/16, fe80::/10) and the cloud instance
//!   metadata endpoints there (169.254.169.254, fd00:ec2::254), which hand out temporary cloud
//!   credentials to whoever asks from inside the instance; and the unspecified address.
//! * Refused when the user turns on **Block local network access** (`blockLocalNetworkAccess`):
//!   this computer (loopback, `localhost`) and private networks (10/8, 172.16/12, 192.168/16,
//!   100.64/10, fc00::/7).
//!
//! Checked on the URL itself, on every redirect hop, and on the addresses a host name resolves to
//! (a custom resolver), so a public name pointing at 127.0.0.1 doesn't get around it. Through a
//! proxy the proxy resolves names, so only literal addresses and `localhost` names are checked.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// Most redirects followed, as reqwest's default.
const MAX_REDIRECTS: usize = 10;

fn v4_problem(ip: Ipv4Addr, block_local: bool) -> Option<&'static str> {
    if ip.is_link_local() {
        return Some("a link-local or cloud metadata address");
    }
    if ip.is_unspecified() {
        return Some("an unspecified address");
    }
    if block_local {
        let [a, b, ..] = ip.octets();
        if ip.is_loopback() {
            return Some("this computer");
        }
        if ip.is_private() || (a == 100 && (64..128).contains(&b)) {
            return Some("a local network address");
        }
    }
    None
}

fn v6_problem(ip: Ipv6Addr, block_local: bool) -> Option<&'static str> {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_problem(v4, block_local);
    }
    let first = ip.segments()[0];
    // fd00:ec2::254 is the EC2 metadata service over IPv6.
    if (first & 0xffc0) == 0xfe80 || ip == Ipv6Addr::new(0xfd00, 0xec2, 0, 0, 0, 0, 0, 0x254) {
        return Some("a link-local or cloud metadata address");
    }
    if ip.is_unspecified() {
        return Some("an unspecified address");
    }
    if block_local {
        if ip.is_loopback() {
            return Some("this computer");
        }
        if (first & 0xfe00) == 0xfc00 {
            return Some("a local network address");
        }
    }
    None
}

/// Why `ip` may not be contacted, if it may not.
pub fn address_problem(ip: IpAddr, block_local: bool) -> Option<&'static str> {
    match ip {
        IpAddr::V4(v4) => v4_problem(v4, block_local),
        IpAddr::V6(v6) => v6_problem(v6, block_local),
    }
}

/// Why `url` may not be fetched, judged on the URL alone (scheme, literal address, `localhost`).
pub fn url_problem(url: &url::Url, block_local: bool) -> Option<String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Some(format!("{url}: only http and https URLs can be fetched"));
    }
    let refused = |what: &str| {
        Some(format!(
            "{url} points at {what}, which web tools may not reach"
        ))
    };
    match url.host() {
        Some(url::Host::Ipv4(ip)) => address_problem(IpAddr::V4(ip), block_local).and_then(refused),
        Some(url::Host::Ipv6(ip)) => address_problem(IpAddr::V6(ip), block_local).and_then(refused),
        Some(url::Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if block_local && (name == "localhost" || name.ends_with(".localhost")) {
                refused("this computer")
            } else {
                None
            }
        }
        None => Some(format!("{url} has no host")),
    }
}

/// Resolves names with the system resolver and drops the addresses [`address_problem`] refuses;
/// a name left with none fails to resolve with the reason.
struct GuardedResolver {
    block_local: bool,
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let block_local = self.block_local;
        Box::pin(async move {
            let host = name.as_str().to_string();
            let resolved: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let mut reason = None;
            let allowed: Vec<SocketAddr> = resolved
                .into_iter()
                .filter(|addr| match address_problem(addr.ip(), block_local) {
                    Some(why) => {
                        reason = Some(why);
                        false
                    }
                    None => true,
                })
                .collect();
            if allowed.is_empty() {
                let why = reason.unwrap_or("no address");
                return Err(
                    format!("{host} resolves to {why}, which web tools may not reach").into(),
                );
            }
            Ok(Box::new(allowed.into_iter()) as Addrs)
        })
    }
}

/// `builder` with the destination checks: a redirect policy that re-checks every hop and,
/// without a proxy, a resolver that checks every resolved address.
pub fn guard(
    builder: reqwest::ClientBuilder,
    block_local: bool,
    proxied: bool,
) -> reqwest::ClientBuilder {
    let builder = builder.redirect(reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        match url_problem(attempt.url(), block_local) {
            Some(problem) => attempt.error(problem),
            None => attempt.follow(),
        }
    }));
    if proxied {
        builder
    } else {
        builder.dns_resolver(Arc::new(GuardedResolver { block_local }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn metadata_and_link_local_are_always_refused() {
        for a in [
            "169.254.169.254",
            "169.254.1.1",
            "fe80::1",
            "fd00:ec2::254",
            "0.0.0.0",
            "::ffff:169.254.169.254",
        ] {
            assert!(address_problem(ip(a), false).is_some(), "{a}");
        }
        for a in [
            "127.0.0.1",
            "10.0.0.1",
            "192.168.1.1",
            "::1",
            "fd12::1",
            "8.8.8.8",
        ] {
            assert!(address_problem(ip(a), false).is_none(), "{a}");
        }
    }

    #[test]
    fn the_switch_refuses_this_computer_and_local_networks() {
        for a in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.0.10",
            "100.64.0.1",
            "::1",
            "fd12::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(address_problem(ip(a), true).is_some(), "{a}");
        }
        for a in ["8.8.8.8", "172.32.0.1", "100.128.0.1", "2606:4700::1111"] {
            assert!(address_problem(ip(a), true).is_none(), "{a}");
        }
    }

    #[test]
    fn urls_are_checked_by_scheme_and_host() {
        let u = |s: &str| url::Url::parse(s).unwrap();
        assert!(url_problem(&u("http://169.254.169.254/latest/meta-data/"), false).is_some());
        assert!(url_problem(&u("file:///etc/passwd"), false).is_some());
        assert!(url_problem(&u("http://localhost:3000/"), false).is_none());
        assert!(url_problem(&u("http://localhost:3000/"), true).is_some());
        assert!(url_problem(&u("http://app.localhost/"), true).is_some());
        assert!(url_problem(&u("https://example.com/"), true).is_none());
        assert!(url_problem(&u("http://[::1]:8080/"), true).is_some());
    }

    #[tokio::test]
    async fn the_resolver_drops_refused_addresses() {
        let r = GuardedResolver { block_local: true };
        let err = r.resolve("localhost".parse().unwrap()).await.err().unwrap();
        assert!(err.to_string().contains("may not reach"), "{err}");
        let r = GuardedResolver { block_local: false };
        assert!(r.resolve("localhost".parse().unwrap()).await.is_ok());
    }
}
