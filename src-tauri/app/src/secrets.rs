//! Credentials the renderer never sees (UI-3 / APP-5): the AWS secret access key and session
//! token and the proxy password (store key `aws`), and the Tavily API key (`tavilySearch`). Rust
//! reads them from the store as before; what goes to a webview (`store_get`, `store_all`,
//! `store-changed`) has them blanked, with `has…` flags so Settings can show that one is saved. A
//! write of one of these keys from a webview with a blank secret keeps the saved one;
//! `clear…: true` removes it.

use serde_json::{Map, Value};

/// `(field path under the store key, has-flag, clear-flag)`.
type Secret = (&'static [&'static str], &'static str, &'static str);

/// Under `tavilySearch`.
const TAVILY_SECRETS: &[Secret] = &[(&["apikey"], "hasApikey", "clearApikey")];

/// The secrets under store key `key`, if it holds any.
fn secrets_for(key: &str) -> Option<&'static [Secret]> {
    match key {
        "aws" => Some(SECRETS),
        "tavilySearch" => Some(TAVILY_SECRETS),
        _ => None,
    }
}

/// Under `aws`.
const SECRETS: &[Secret] = &[
    (
        &["secretAccessKey"],
        "hasSecretAccessKey",
        "clearSecretAccessKey",
    ),
    (&["sessionToken"], "hasSessionToken", "clearSessionToken"),
    (
        &["proxyConfig", "password"],
        "hasProxyPassword",
        "clearProxyPassword",
    ),
];

fn get_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, k| v.get(*k))
}

fn object_at<'a>(value: &'a mut Value, path: &[&str]) -> Option<&'a mut Map<String, Value>> {
    path.iter()
        .try_fold(value, |v, k| v.get_mut(*k))
        .and_then(Value::as_object_mut)
}

fn is_set(v: Option<&Value>) -> bool {
    v.and_then(Value::as_str).is_some_and(|s| !s.is_empty())
}

/// `aws` with its secrets blanked and `has…` flags added.
#[cfg(test)]
fn redact_aws(aws: &Value) -> Value {
    redact_with(aws, SECRETS)
}

fn redact_with(aws: &Value, secrets: &[Secret]) -> Value {
    let mut out = aws.clone();
    for (path, has, _) in secrets {
        let (field, parent) = path.split_last().expect("non-empty path");
        let saved = is_set(get_path(aws, path));
        if let Some(obj) = object_at(&mut out, parent) {
            if obj.contains_key(*field) {
                obj.insert((*field).to_string(), Value::String(String::new()));
            }
        }
        if let Some(obj) = out.as_object_mut() {
            obj.insert((*has).to_string(), Value::Bool(saved));
        }
    }
    out
}

/// What a webview may see of store key `key` (dotted paths into `aws` included).
pub fn redact(key: &str, value: Option<Value>) -> Option<Value> {
    let value = value?;
    if let Some(secrets) = secrets_for(key) {
        return Some(redact_with(&value, secrets));
    }
    let Some((top, rest)) = key.split_once('.') else {
        return Some(value);
    };
    if let Some(secrets) = secrets_for(top) {
        let path: Vec<&str> = rest.split('.').collect();
        for (secret, _, _) in secrets {
            if path.as_slice() == *secret {
                return Some(Value::String(String::new()));
            }
            if secret.starts_with(&path) {
                // A parent object of a secret (e.g. `aws.proxyConfig`): blank inside it.
                let mut wrapped = Value::Object(Map::new());
                let mut cursor = &mut wrapped;
                for k in &path {
                    cursor = cursor
                        .as_object_mut()
                        .expect("object")
                        .entry((*k).to_string())
                        .or_insert(Value::Object(Map::new()));
                }
                *cursor = value.clone();
                let redacted = redact_with(&wrapped, secrets);
                return get_path(&redacted, &path).cloned();
            }
        }
    }
    Some(value)
}

/// The whole store as a webview may see it.
pub fn redact_all(mut all: Value) -> Value {
    for key in ["aws", "tavilySearch"] {
        if let (Some(value), Some(secrets)) = (all.get(key).cloned(), secrets_for(key)) {
            all[key] = redact_with(&value, secrets);
        }
    }
    all
}

/// The value to save when a webview writes `incoming` to `key`: blank secrets keep the saved
/// ones (`existing`), `clear…: true` removes one, and the helper flags are dropped. Keys without
/// secrets are saved as given.
pub fn merge(key: &str, incoming: Value, existing: Option<&Value>) -> Value {
    match secrets_for(key) {
        Some(secrets) => merge_with(incoming, existing, secrets),
        None => incoming,
    }
}

/// The `aws` value to save when a webview writes `incoming`: blank secrets keep the saved ones
/// (`existing`), `clear…: true` removes one, and the helper flags are dropped.
#[cfg(test)]
fn merge_aws(incoming: Value, existing: Option<&Value>) -> Value {
    merge_with(incoming, existing, SECRETS)
}

fn merge_with(incoming: Value, existing: Option<&Value>, secrets: &[Secret]) -> Value {
    let mut out = incoming;
    for (path, has, clear) in secrets {
        let (field, parent) = path.split_last().expect("non-empty path");
        let clear_requested = out.get(*clear).and_then(Value::as_bool) == Some(true);
        let incoming_set = is_set(get_path(&out, path));
        let saved = existing.and_then(|e| get_path(e, path)).cloned();
        if let Some(obj) = out.as_object_mut() {
            obj.remove(*has);
            obj.remove(*clear);
        }
        if clear_requested {
            if let Some(obj) = object_at(&mut out, parent) {
                obj.remove(*field);
            }
            continue;
        }
        if !incoming_set {
            if let (Some(saved), Some(obj)) = (saved, object_at(&mut out, parent)) {
                if is_set(Some(&saved)) {
                    obj.insert((*field).to_string(), saved);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn saved() -> Value {
        json!({
            "region": "us-east-1", "accessKeyId": "AKIA", "secretAccessKey": "SECRET",
            "sessionToken": "TOKEN", "proxyConfig": { "enabled": true, "host": "p", "password": "PW" }
        })
    }

    #[test]
    fn webviews_never_see_the_secrets() {
        let r = redact_aws(&saved());
        assert_eq!(r["secretAccessKey"], "");
        assert_eq!(r["sessionToken"], "");
        assert_eq!(r["proxyConfig"]["password"], "");
        assert_eq!(r["accessKeyId"], "AKIA");
        assert_eq!(r["hasSecretAccessKey"], true);
        assert_eq!(r["hasSessionToken"], true);
        assert_eq!(r["hasProxyPassword"], true);
        assert!(!r.to_string().contains("SECRET"));

        assert_eq!(
            redact("aws.secretAccessKey", Some(json!("SECRET"))),
            Some(json!(""))
        );
        let proxy = redact("aws.proxyConfig", Some(saved()["proxyConfig"].clone())).unwrap();
        assert_eq!(proxy["password"], "");
        assert_eq!(proxy["host"], "p");
        assert_eq!(redact("language", Some(json!("en"))), Some(json!("en")));
        let all = redact_all(json!({ "aws": saved(), "language": "en" }));
        assert!(!all.to_string().contains("SECRET") && !all.to_string().contains("PW"));
    }

    #[test]
    fn the_tavily_key_is_handled_the_same_way() {
        let saved = json!({ "apikey": "tvly-KEY" });
        let shown = redact("tavilySearch", Some(saved.clone())).unwrap();
        assert_eq!(shown, json!({ "apikey": "", "hasApikey": true }));
        assert_eq!(
            redact("tavilySearch.apikey", Some(json!("tvly-KEY"))),
            Some(json!(""))
        );
        assert!(!redact_all(json!({ "tavilySearch": saved.clone() }))
            .to_string()
            .contains("KEY"));
        assert_eq!(merge("tavilySearch", shown, Some(&saved)), saved);
        let cleared = merge(
            "tavilySearch",
            json!({ "apikey": "", "clearApikey": true }),
            Some(&saved),
        );
        assert_eq!(cleared, json!({}));
        assert_eq!(merge("language", json!("en"), None), json!("en"));
    }

    #[test]
    fn blank_writes_keep_saved_secrets_and_clear_flags_remove_them() {
        let incoming = redact_aws(&saved());
        let merged = merge_aws(incoming, Some(&saved()));
        assert_eq!(merged, saved());

        let mut replace = redact_aws(&saved());
        replace["secretAccessKey"] = json!("NEW");
        assert_eq!(merge_aws(replace, Some(&saved()))["secretAccessKey"], "NEW");

        let mut clear = redact_aws(&saved());
        clear["clearSessionToken"] = json!(true);
        let merged = merge_aws(clear, Some(&saved()));
        assert!(merged.get("sessionToken").is_none());
        assert!(merged.get("clearSessionToken").is_none());
        assert_eq!(merged["secretAccessKey"], "SECRET");
    }
}
