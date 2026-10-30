//! The app-command ACL stays in step with `handler()`: `build.rs` derives the command list from
//! the `generate_handler!` call in `mod.rs`, and the capabilities grant each window only what it
//! uses.

#[path = "../command_list.rs"]
mod command_list;

use command_list::{
    all_commands_set, allow_permission, handler_commands, is_main_window_only,
    main_window_commands_set,
};
use serde_json::Value;
use std::collections::BTreeSet;

const HANDLER_SOURCE: &str = include_str!("mod.rs");
const ALL_COMMANDS_SET: &str = include_str!("../../permissions/app-commands.toml");
const MAIN_WINDOW_COMMANDS_SET: &str = include_str!("../../permissions/main-window-commands.toml");
const DEFAULT_CAPABILITY: &str = include_str!("../../capabilities/default.json");
const CAMERA_CAPABILITY: &str = include_str!("../../capabilities/camera-preview.json");
const FILE_VIEWER_CAPABILITY: &str = include_str!("../../capabilities/file-viewer.json");
const MAIN_WINDOW_CAPABILITY: &str = include_str!("../../capabilities/main-window.json");
const CAMERA_PREVIEW_HTML: &str = include_str!("../../../../src/renderer/camera-preview.html");

fn commands() -> Vec<String> {
    handler_commands(HANDLER_SOURCE).expect("parse generate_handler!")
}

fn capabilities() -> Vec<Value> {
    [
        DEFAULT_CAPABILITY,
        CAMERA_CAPABILITY,
        FILE_VIEWER_CAPABILITY,
        MAIN_WINDOW_CAPABILITY,
    ]
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
    assert_eq!(
        MAIN_WINDOW_COMMANDS_SET,
        main_window_commands_set(&commands())
    );
}

/// The commands of `set` in a generated permission file.
fn set_permissions(toml: &str) -> BTreeSet<String> {
    toml.lines()
        .filter_map(|l| l.trim().strip_prefix('"')?.strip_suffix("\","))
        .map(str::to_string)
        .collect()
}

#[test]
fn local_terminals_updates_and_knowledge_are_main_window_only() {
    let local: Vec<String> = commands()
        .into_iter()
        .filter(|c| c.starts_with("local_terminal_"))
        .collect();
    assert_eq!(
        local,
        [
            "local_terminal_open",
            "local_terminal_attach",
            "local_terminal_input",
            "local_terminal_resize",
            "local_terminal_close",
            "local_terminal_list",
            "local_terminal_backlog",
        ]
    );
    let all = set_permissions(ALL_COMMANDS_SET);
    let main_only = set_permissions(MAIN_WINDOW_COMMANDS_SET);
    for c in &local {
        assert!(is_main_window_only(c));
        assert!(
            !all.contains(&allow_permission(c)),
            "{c} is in app-commands"
        );
        assert!(main_only.contains(&allow_permission(c)), "{c}");
    }
    // App updates, the knowledgebase and the usage commands are the only other main-window-only
    // commands; nothing else is by accident, and the two sets cover every command.
    let updates: Vec<String> = commands()
        .into_iter()
        .filter(|c| c.starts_with("app_update_"))
        .collect();
    assert_eq!(
        updates,
        [
            "app_update_current_version",
            "app_update_check",
            "app_update_download",
            "app_update_restart",
        ]
    );
    for c in &updates {
        assert!(is_main_window_only(c));
        assert!(
            !all.contains(&allow_permission(c)),
            "{c} is in app-commands"
        );
        assert!(main_only.contains(&allow_permission(c)), "{c}");
    }
    let knowledge: Vec<String> = commands()
        .into_iter()
        .filter(|c| c.starts_with("knowledge_"))
        .collect();
    assert!(knowledge.contains(&"knowledge_pick_files".to_string()));
    assert!(knowledge.contains(&"knowledge_pick_folder".to_string()));
    for c in &knowledge {
        assert!(is_main_window_only(c));
        assert!(
            !all.contains(&allow_permission(c)),
            "{c} is in app-commands"
        );
        assert!(main_only.contains(&allow_permission(c)), "{c}");
    }
    let usage: Vec<String> = commands()
        .into_iter()
        .filter(|c| c.starts_with("usage_"))
        .collect();
    assert_eq!(
        usage,
        [
            "usage_summary",
            "usage_breakdown",
            "usage_months",
            "usage_events",
            "usage_info",
            "usage_export_csv",
            "usage_budget_status",
        ]
    );
    for c in &usage {
        assert!(is_main_window_only(c));
        assert!(
            !all.contains(&allow_permission(c)),
            "{c} is in app-commands"
        );
        assert!(main_only.contains(&allow_permission(c)), "{c}");
    }
    assert_eq!(
        main_only.len(),
        local.len() + updates.len() + knowledge.len() + usage.len()
    );
    assert_eq!(all.len() + main_only.len(), commands().len());
    // The sandbox terminal stays available to every full-UI window.
    assert!(all.contains("allow-docker-sandbox-terminal-open"));
}

#[test]
fn every_granted_app_permission_is_a_registered_command() {
    let allowed: BTreeSet<String> = commands().iter().map(|c| allow_permission(c)).collect();
    for cap in capabilities() {
        for p in strings(&cap, "permissions") {
            if p.contains(':') || p == "app-commands" || p == "main-window-commands" {
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
    // The main window gets every command, local terminals included.
    assert_eq!(
        app_permissions_for("main"),
        BTreeSet::from([
            "app-commands".to_string(),
            "main-window-commands".to_string()
        ])
    );
    // The other full UI windows get every command but those; file viewers match by prefix,
    // whatever the hash.
    for label in [
        "task-history",
        "file-viewer-0123456789ab",
        &crate::commands::window::file_viewer_label("docs/a b.md"),
    ] {
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

    // File viewers may also destroy themselves (the JS close-requested handler does); nothing
    // else gets that.
    let destroy = |label: &str| {
        capabilities().iter().any(|c| {
            strings(c, "windows")
                .iter()
                .any(|w| matches_window(w, label))
                && strings(c, "permissions")
                    .iter()
                    .any(|p| p == "core:window:allow-destroy")
        })
    };
    assert!(destroy("file-viewer-0123456789ab"));
    assert!(!destroy("main"));
    assert!(!destroy("task-history"));

    // PDF export windows (hidden, printing model-derived HTML) and anything unknown: nothing.
    for label in ["pdf-export-1", "some-other-window", "file-viewer"] {
        assert!(
            capabilities().iter().all(|c| !strings(c, "windows")
                .iter()
                .any(|w| matches_window(w, label))),
            "{label} matches a capability"
        );
    }
}
