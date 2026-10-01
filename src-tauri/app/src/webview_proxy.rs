//! The configured proxy (`aws.proxyConfig`) for the app's webviews — Electron's
//! `setupSessionProxy` (`session.setProxy` + `app.on('login')`), so remote images and the
//! codesandbox.io / embed.diagrams.net iframes go through the same proxy as the Rust HTTP
//! clients.
//!
//! Like Electron's session rules (`http=host:port;https=host:port`), webviews always reach the
//! proxy over plain HTTP (CONNECT); the `protocol` setting only affects the Rust clients.
//! Loopback hosts are not proxied (Chromium's implicit bypass), which keeps the dev server
//! reachable.
//!
//! Per platform:
//! - **Windows** (WebView2): Tauri's `proxy_url` (`--proxy-server`). Credentials can't be
//!   supplied; a proxy that requires authentication is not supported.
//! - **Linux** (WebKitGTK): Tauri's `proxy_url`; proxy authentication challenges are answered
//!   with the stored username / password (once per challenge; a rejected retry falls back to
//!   WebKitGTK's own handling).
//! - **macOS** (WKWebView): needs macOS 14+ (`WKWebsiteDataStore.proxyConfigurations`). Instead
//!   of Tauri's `macos-proxy` feature — which links the macOS 14 Network.framework symbols
//!   directly and would stop the app from launching on older systems — the proxy is set on the
//!   default data store shared by every window, with the Network.framework functions looked up
//!   at runtime. Credentials go into the proxy configuration. On macOS 13 and older webviews
//!   connect directly (logged at startup).
//!
//! The proxy is read once at startup and every window uses that same setting: a webview's proxy
//! can't be changed after it is created, and WebView2 refuses to create webviews whose browser
//! arguments differ from the running ones. A change in Settings is logged and applies to
//! webviews after a restart; the Rust HTTP clients pick it up immediately.

use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Runtime, Url, WebviewWindow, WebviewWindowBuilder};

/// Hosts never sent through the proxy.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const EXCLUDED_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// The webview proxy endpoint.
#[derive(Clone, PartialEq, Eq)]
pub struct WebviewProxy {
    pub host: String,
    pub port: u16,
    /// `(username, password)` when both are set.
    pub credentials: Option<(String, String)>,
}

impl std::fmt::Debug for WebviewProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebviewProxy")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("has_credentials", &self.credentials.is_some())
            .finish()
    }
}

impl WebviewProxy {
    /// The proxy from the whole store (`store.all()`): `None` unless enabled with a host that
    /// forms a valid URL. The port defaults to 8080, as for the Rust clients.
    pub fn from_store(store: &Value) -> Option<WebviewProxy> {
        let config = tools::ToolSettings::from_store(store).proxy?;
        if !config.enabled {
            return None;
        }
        let host = config.host?.trim().to_string();
        let port = config.port.unwrap_or(8080);
        // Validate (and normalize) the host the way the URL handed to Tauri will parse it.
        let bracketed = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host
        };
        let url = Url::parse(&format!("http://{bracketed}:{port}")).ok()?;
        let host = url
            .host_str()?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        let credentials = match (config.username, config.password) {
            (Some(u), Some(p)) => Some((u, p)),
            _ => None,
        };
        Some(WebviewProxy {
            host,
            port,
            credentials,
        })
    }

    /// The proxy from the store's `aws` value alone (a `store-changed` payload).
    pub fn from_aws_value(aws: Option<&Value>) -> Option<WebviewProxy> {
        Self::from_store(&json!({ "aws": aws }))
    }

    /// `http://host:port`, without credentials (Tauri accepts only `http` / `socks5` URLs and
    /// ignores user info). A proxy on port 80 is a known gap on Windows / Linux: the URL omits
    /// the default port and Tauri then passes the proxy on without one.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn url(&self) -> Url {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        Url::parse(&format!("http://{host}:{}", self.port)).expect("validated in from_store")
    }
}

/// Managed state: the proxy every webview of this run uses, and the last stored setting seen
/// (so a change is logged once).
pub struct WebviewProxyState {
    startup: Option<WebviewProxy>,
    last_seen: Mutex<Option<WebviewProxy>>,
}

impl WebviewProxyState {
    pub fn new(startup: Option<WebviewProxy>) -> Self {
        Self {
            last_seen: Mutex::new(startup.clone()),
            startup,
        }
    }

    /// A `store-changed` for `key`: when the stored proxy no longer matches the last one seen,
    /// returns whether it now differs from the one the webviews use.
    fn note_change(&self, key: &str, value: Option<&Value>) -> Option<bool> {
        if key != "aws" {
            return None;
        }
        let proxy = WebviewProxy::from_aws_value(value);
        let mut last = self.last_seen.lock().ok()?;
        if *last == proxy {
            return None;
        }
        *last = proxy.clone();
        Some(proxy != self.startup)
    }
}

/// Startup: log the webview proxy and, on macOS, install it on the shared data store. Call in
/// `setup` before the first window is built.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    let Some(proxy) = app
        .try_state::<WebviewProxyState>()
        .and_then(|s| s.startup.clone())
    else {
        tracing::debug!("Webview proxy disabled - using direct connections");
        return;
    };
    #[cfg(target_os = "macos")]
    match macos::set_default_data_store_proxy(&proxy) {
        Ok(()) => tracing::info!(
            host = %proxy.host,
            port = proxy.port,
            has_auth = proxy.credentials.is_some(),
            "Webview proxy configured"
        ),
        Err(e) => tracing::warn!(
            host = %proxy.host,
            port = proxy.port,
            error = %e,
            "Webview proxy not applied; web content in the app connects directly"
        ),
    }
    #[cfg(not(target_os = "macos"))]
    {
        tracing::info!(
            host = %proxy.host,
            port = proxy.port,
            has_auth = proxy.credentials.is_some(),
            "Webview proxy configured"
        );
        #[cfg(windows)]
        if proxy.credentials.is_some() {
            tracing::warn!(
                "Proxy credentials can't be passed to WebView2; web content in the app fails \
                 to load through a proxy that requires authentication"
            );
        }
    }
}

/// Apply the startup proxy to a window builder. Every webview window the app creates must go
/// through this (on Windows a webview with a different proxy fails to be created).
pub fn configure<'a, R: Runtime, M: Manager<R>>(
    app: &AppHandle<R>,
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(not(target_os = "macos"))]
    if let Some(proxy) = app
        .try_state::<WebviewProxyState>()
        .and_then(|s| s.startup.clone())
    {
        return builder.proxy_url(proxy.url());
    }
    // macOS: set once on the shared data store by `install`.
    let _ = app;
    builder
}

/// After a window configured with [`configure`] is built: on Linux, answer proxy
/// authentication challenges with the stored credentials.
pub fn attach<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    if let Some((user, pass)) = window
        .try_state::<WebviewProxyState>()
        .and_then(|s| s.startup.as_ref().and_then(|p| p.credentials.clone()))
    {
        if let Err(e) =
            window.with_webview(move |webview| linux::answer_proxy_auth(webview, user, pass))
        {
            tracing::warn!(window = window.label(), error = %e, "Failed to install the proxy authentication handler");
        }
    }
    let _ = window;
}

/// `store-changed` hook: log a proxy change, which webviews pick up after a restart.
pub fn on_store_change<R: Runtime>(app: &AppHandle<R>, key: &str, value: Option<&Value>) {
    let Some(state) = app.try_state::<WebviewProxyState>() else {
        return;
    };
    match state.note_change(key, value) {
        Some(true) => tracing::info!(
            "Proxy settings changed; AWS and tool connections use them now, web content in the \
             app windows after a restart"
        ),
        Some(false) => tracing::info!("Proxy settings match the ones web content in the app uses"),
        None => {}
    }
}

#[cfg(target_os = "macos")]
mod macos {
    //! `WKWebsiteDataStore.defaultDataStore.proxyConfigurations = @[nw_proxy_config]` (what
    //! wry's `mac-proxy` does per webview), with every macOS 14 symbol resolved at runtime.

    use super::{WebviewProxy, EXCLUDED_HOSTS};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, sel};
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::sync::OnceLock;

    type NwObject = *mut AnyObject;

    extern "C" {
        fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_LAZY: c_int = 0x1;

    /// The Network.framework functions used (all macOS 14+ except `nw_endpoint_create_host`).
    struct Network {
        endpoint_create_host: unsafe extern "C" fn(*const c_char, *const c_char) -> NwObject,
        proxy_config_create_http_connect: unsafe extern "C" fn(NwObject, NwObject) -> NwObject,
        proxy_config_set_username_and_password:
            unsafe extern "C" fn(NwObject, *const c_char, *const c_char),
        proxy_config_add_excluded_domain: unsafe extern "C" fn(NwObject, *const c_char),
    }

    fn network() -> Option<&'static Network> {
        static NETWORK: OnceLock<Option<Network>> = OnceLock::new();
        NETWORK
            .get_or_init(|| {
                // SAFETY: dlopen/dlsym with NUL-terminated names; each symbol is transmuted to
                // its documented C signature (Network.framework <nw_proxy_config.h>,
                // <nw_endpoint.h>).
                unsafe {
                    let handle = dlopen(
                        c"/System/Library/Frameworks/Network.framework/Network".as_ptr(),
                        RTLD_LAZY,
                    );
                    if handle.is_null() {
                        return None;
                    }
                    let sym = |name: &CStr| {
                        let p = dlsym(handle, name.as_ptr());
                        (!p.is_null()).then_some(p)
                    };
                    Some(Network {
                        endpoint_create_host: std::mem::transmute::<
                            *mut c_void,
                            unsafe extern "C" fn(*const c_char, *const c_char) -> NwObject,
                        >(sym(
                            c"nw_endpoint_create_host",
                        )?),
                        proxy_config_create_http_connect: std::mem::transmute::<
                            *mut c_void,
                            unsafe extern "C" fn(NwObject, NwObject) -> NwObject,
                        >(sym(
                            c"nw_proxy_config_create_http_connect",
                        )?),
                        proxy_config_set_username_and_password: std::mem::transmute::<
                            *mut c_void,
                            unsafe extern "C" fn(NwObject, *const c_char, *const c_char),
                        >(sym(
                            c"nw_proxy_config_set_username_and_password",
                        )?),
                        proxy_config_add_excluded_domain: std::mem::transmute::<
                            *mut c_void,
                            unsafe extern "C" fn(NwObject, *const c_char),
                        >(sym(
                            c"nw_proxy_config_add_excluded_domain",
                        )?),
                    })
                }
            })
            .as_ref()
    }

    /// Must run on the main thread (AppKit / WebKit).
    pub(super) fn set_default_data_store_proxy(proxy: &WebviewProxy) -> Result<(), String> {
        const NEEDS_14: &str = "webview proxies need macOS 14 or later";
        let nw = network().ok_or(NEEDS_14)?;
        let c = |s: &str| CString::new(s).map_err(|_| format!("invalid proxy setting: {s:?}"));
        let host = c(&proxy.host)?;
        let port = c(&proxy.port.to_string())?;
        let credentials = match &proxy.credentials {
            Some((u, p)) => Some((c(u)?, c(p)?)),
            None => None,
        };
        // SAFETY: Objective-C calls on WebKit / Foundation classes, on the main thread; the
        // `nw_*` create functions return +1 objects, owned by the `Retained`s below.
        unsafe {
            let store: Option<Retained<AnyObject>> =
                msg_send![class!(WKWebsiteDataStore), defaultDataStore];
            let store = store.ok_or("no default website data store")?;
            let supported: bool =
                msg_send![&*store, respondsToSelector: sel!(setProxyConfigurations:)];
            if !supported {
                return Err(NEEDS_14.into());
            }
            let endpoint =
                Retained::from_raw((nw.endpoint_create_host)(host.as_ptr(), port.as_ptr()))
                    .ok_or("failed to create the proxy endpoint")?;
            let config = Retained::from_raw((nw.proxy_config_create_http_connect)(
                Retained::as_ptr(&endpoint) as NwObject,
                std::ptr::null_mut(),
            ))
            .ok_or("failed to create the proxy configuration")?;
            let config_ptr = Retained::as_ptr(&config) as NwObject;
            if let Some((user, pass)) = &credentials {
                (nw.proxy_config_set_username_and_password)(
                    config_ptr,
                    user.as_ptr(),
                    pass.as_ptr(),
                );
            }
            for host in EXCLUDED_HOSTS {
                let host = c(host)?;
                (nw.proxy_config_add_excluded_domain)(config_ptr, host.as_ptr());
            }
            let proxies: Retained<AnyObject> =
                msg_send![class!(NSArray), arrayWithObject: &*config];
            let _: () = msg_send![&*store, setProxyConfigurations: &*proxies];
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    //! Electron's `app.on('login')` for proxies: WebKitGTK's `authenticate` signal.

    use webkit2gtk::glib::translate::{ToGlibPtr, ToGlibPtrMut};
    use webkit2gtk::{AuthenticationRequestExt, Credential, CredentialPersistence, WebViewExt};

    pub(super) fn answer_proxy_auth(
        webview: tauri::webview::PlatformWebview,
        user: String,
        pass: String,
    ) {
        webview.inner().connect_authenticate(move |_, request| {
            // A rejected retry is left to WebKitGTK (its credential dialog) rather than
            // re-sending the same credentials forever.
            if !request.is_for_proxy() || request.is_retry() {
                return false;
            }
            let mut credential = Credential::new(&user, &pass, CredentialPersistence::ForSession);
            // SAFETY: both pointers are valid for the call; WebKit copies the credential.
            unsafe {
                webkit2gtk::ffi::webkit_authentication_request_authenticate(
                    request.to_glib_none().0,
                    credential.to_glib_none_mut().0,
                );
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(proxy: Value) -> Value {
        json!({ "aws": { "region": "us-west-2", "proxyConfig": proxy } })
    }

    #[test]
    fn disabled_or_hostless_proxy_is_none() {
        assert_eq!(WebviewProxy::from_store(&json!({})), None);
        assert_eq!(
            WebviewProxy::from_store(&store(json!({ "enabled": false, "host": "p", "port": 1 }))),
            None
        );
        assert_eq!(
            WebviewProxy::from_store(&store(json!({ "enabled": true, "port": 3128 }))),
            None
        );
        assert_eq!(
            WebviewProxy::from_store(&store(json!({ "enabled": true, "host": "bad host" }))),
            None
        );
    }

    #[test]
    fn url_is_plain_http_without_credentials() {
        let proxy = WebviewProxy::from_store(&store(json!({
            "enabled": true, "host": "proxy.corp", "port": 3128, "protocol": "https",
            "username": "u", "password": "p@ss"
        })))
        .unwrap();
        assert_eq!(proxy.url().as_str(), "http://proxy.corp:3128/");
        assert_eq!(proxy.credentials, Some(("u".into(), "p@ss".into())));
        assert!(!format!("{proxy:?}").contains("p@ss"));
    }

    #[test]
    fn port_defaults_to_8080_and_credentials_need_both_parts() {
        let proxy = WebviewProxy::from_store(&store(json!({
            "enabled": true, "host": "proxy", "username": "u"
        })))
        .unwrap();
        assert_eq!(proxy.url().as_str(), "http://proxy:8080/");
        assert_eq!(proxy.credentials, None);
    }

    #[test]
    fn ipv6_hosts_are_bracketed_in_the_url_only() {
        let proxy = WebviewProxy::from_store(&store(
            json!({ "enabled": true, "host": "::1", "port": 8888 }),
        ))
        .unwrap();
        assert_eq!(proxy.host, "::1");
        assert_eq!(proxy.url().as_str(), "http://[::1]:8888/");
        let bracketed = WebviewProxy::from_store(&store(
            json!({ "enabled": true, "host": "[fd00::2]", "port": 8888 }),
        ))
        .unwrap();
        assert_eq!(bracketed.host, "fd00::2");
    }

    #[test]
    fn store_changes_are_reported_once_against_the_startup_proxy() {
        let startup = WebviewProxy::from_store(&store(json!({ "enabled": true, "host": "a" })));
        let state = WebviewProxyState::new(startup);
        let aws = |host: &str| json!({ "proxyConfig": { "enabled": true, "host": host } });

        assert_eq!(state.note_change("language", Some(&json!("en"))), None);
        // Same proxy (e.g. only the region changed): nothing to report.
        assert_eq!(state.note_change("aws", Some(&aws("a"))), None);
        assert_eq!(state.note_change("aws", Some(&aws("b"))), Some(true));
        assert_eq!(state.note_change("aws", Some(&aws("b"))), None);
        // Back to the proxy the webviews use.
        assert_eq!(state.note_change("aws", Some(&aws("a"))), Some(false));
        // Disabling it (`proxyConfig` dropped) is a change too.
        assert_eq!(
            state.note_change("aws", Some(&json!({ "region": "x" }))),
            Some(true)
        );
        assert_eq!(state.note_change("aws", None), None);
    }
}
