//! The app-command ACL stays in step with `handler()`: `build.rs` derives the command list from
//! the `generate_handler!` call in `mod.rs`, and the capabilities grant each window only what it
//! uses.

#[path = "../command_list.rs"]
mod command_list;

use command_list::{all_commands_set, allow_permission, handler_commands};
use serde_json::Value;
use std::collections::BTreeSet;

const HANDLER_SOURCE: &str = include_str!("mod.rs");
const ALL_COMMANDS_SET: &str = include_str!("../../permissions/app-commands.toml");
const DEFAULT_CAPABILITY: &str = include_str!("../../capabilities/default.json");
const CAMERA_CAPABILITY: &str = include_str!("../../capabilities/camera-preview.json");
const CAMERA_PREVIEW_HTML: &str = include_str!("../../../../src/renderer/camera-preview.html");

fn commands() -> Vec<String> {
    handler_commands(HANDLER_SOURCE).expect("parse generate_handler!")
}

fn capabilities() -> Vec<Value> {
    [DEFAULT_CAPABILITY, CAMERA_CAPABILITY]
        .iter()
        .map(|s| serde_json::from_str(s).expect("capability JSON"))
        .collect()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} array"))
        .iter()
        .map(|p| p.as_str().expect("string").to_string())
        .collect()
}

/// Capability window patterns are labels or `prefix*` globs.
fn matches_window(pattern: &str, label: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => label.starts_with(prefix),
        None => pattern == label,
    }
}

/// App permissions (no `plugin:` prefix) granted to the window `label`.
fn app_permissions_for(label: &str) -> BTreeSet<String> {
    capabilities()
        .iter()
        .filter(|c| {
            strings(c, "windows")
                .iter()
                .any(|w| matches_window(w, label))
        })
        .flat_map(|c| strings(c, "permissions"))
        .filter(|p| !p.contains(':'))
        .collect()
}

#[test]
fn handler_list_is_parsed() {
    let commands = commands();
    assert!(commands.len() > 100, "{}", commands.len());
    for c in [
        "store_get",
        "logger_log",
        "camera_capture_response",
        "tools_execute",
    ] {
        assert!(commands.iter().any(|x| x == c), "{c}");
    }
    // Every `module::command,` line in the list is counted.
    let body = HANDLER_SOURCE.split("generate_handler![").nth(1).unwrap();
    let body = body.split(']').next().unwrap();
    let items = body
        .lines()
        .map(|l| l.split("//").next().unwrap().trim())
        .filter(|l| l.contains("::"))
        .count();
    assert_eq!(commands.len(), items);
}

#[test]
fn parser_handles_comments_and_rejects_bad_lists() {
    let ok = "x generate_handler![\n  // a [comment], with ] brackets\n  a::one,\n  two, // trailing\n  b::c::three\n] y";
    assert_eq!(handler_commands(ok).unwrap(), ["one", "two", "three"]);
    assert!(handler_commands("nothing here").is_err());
    assert!(handler_commands("generate_handler![]").is_err());
    assert!(handler_commands("generate_handler![a::x, b::x]").is_err());
    assert!(handler_commands("generate_handler![a::x(1)]").is_err());
    assert_eq!(allow_permission("store_get"), "allow-store-get");
}

#[test]
fn generated_permission_set_is_current() {
    assert_eq!(ALL_COMMANDS_SET, all_commands_set(&commands()));
}

#[test]
fn every_granted_app_permission_is_a_registered_command() {
    let allowed: BTreeSet<String> = commands().iter().map(|c| allow_permission(c)).collect();
    for cap in capabilities() {
        for p in strings(&cap, "permissions") {
            if p.contains(':') || p == "app-commands" {
                continue;
            }
            assert!(
                allowed.contains(&p),
                "{} grants unknown {p}",
                cap["identifier"]
            );
        }
    }
}

#[test]
fn windows_get_only_what_they_use() {
    // The full UI windows get every command.
    for label in ["main", "task-history"] {
        assert_eq!(
            app_permissions_for(label),
            BTreeSet::from(["app-commands".to_string()])
        );
    }

    // Camera previews: exactly the commands camera-preview.html invokes
    // (`invokeMain('camera:close-preview-window', …)` → `camera_close_preview_window`).
    let invoked: BTreeSet<String> = CAMERA_PREVIEW_HTML
        .split("invokeMain('")
        .skip(1)
        .map(|rest| {
            let channel = rest.split('\'').next().unwrap();
            allow_permission(&channel.replace([':', '-'], "_"))
        })
        .collect();
    assert_eq!(invoked.len(), 2, "{invoked:?}");
    assert_eq!(app_permissions_for("camera-preview-1"), invoked);

    // PDF export windows (hidden, printing model-derived HTML) and anything unknown: nothing.
    for label in ["pdf-export-1", "some-other-window"] {
        assert!(
            capabilities().iter().all(|c| !strings(c, "windows")
                .iter()
                .any(|w| matches_window(w, label))),
            "{label} matches a capability"
        );
    }
}
