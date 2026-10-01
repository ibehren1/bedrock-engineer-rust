//! First-run defaults and migrations, ported line-by-line from `init()` in
//! `src/preload/store.ts`.
//!
//! The TS code mixes two checks: `if (!value)` (JS falsy: missing, `null`,
//! `false`, `0`, `""`) and `if (value === undefined)` (missing only). Each key
//! keeps the check its TS counterpart uses, because the difference is visible
//! to users (e.g. an empty `selectedAgentId` is reset to `softwareAgent`, but an
//! empty `userName` is kept). Keys are initialized in the TS order so a fresh
//! config file has the same key order electron-store would write.

use serde_json::{json, Map, Value};

/// Process environment the defaults depend on. Injected so tests can cover
/// every platform from any host.
#[derive(Debug, Clone)]
pub struct Env {
    pub windows: bool,
    /// `USERPROFILE` on Windows, `HOME` elsewhere.
    pub home: Option<String>,
    /// `ComSpec` (Windows only).
    pub comspec: Option<String>,
}

impl Env {
    pub fn current() -> Self {
        let windows = cfg!(windows);
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Env {
            windows,
            home: var(if windows { "USERPROFILE" } else { "HOME" }),
            comspec: var("ComSpec"),
        }
    }
}

/// JS truthiness of a JSON value (`undefined` is represented by `None`).
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_) | Value::Object(_)) => true,
    }
}

fn code_interpreter_default() -> Value {
    json!({ "memoryLimit": "256m", "cpuLimit": 0.5, "timeout": 30 })
}

/// Applies defaults and migrations in place.
pub fn init(data: &mut Map<String, Value>, env: &Env) {
    falsy(data, "userDataPath", json!(""));
    // TS sets `projectPath` to the env var even when it is unset; electron-store
    // then stores nothing, so skipping the write matches.
    if !truthy(data.get("projectPath")) {
        if let Some(home) = &env.home {
            data.insert("projectPath".into(), json!(home));
        }
    }

    // TS checks the nested `advancedSetting.keybinding` and replaces the whole
    // `advancedSetting` object when it is missing.
    let keybinding = data
        .get("advancedSetting")
        .and_then(|a| a.get("keybinding"));
    if !truthy(keybinding) {
        data.insert(
            "advancedSetting".into(),
            json!({ "keybinding": { "sendMsgKey": "Enter" } }),
        );
    }

    missing(data, "language", json!("en"));
    falsy(
        data,
        "aws",
        json!({ "region": "us-west-2", "accessKeyId": "", "secretAccessKey": "" }),
    );
    falsy(
        data,
        "inferenceParams",
        json!({ "maxTokens": 4096, "temperature": 0.5, "topP": 0.9 }),
    );
    falsy(
        data,
        "thinkingMode",
        json!({ "type": "enabled", "budget_tokens": 4096 }),
    );
    missing(data, "interleaveThinking", json!(false));
    falsy(data, "customAgents", json!([]));
    falsy(data, "selectedAgentId", json!("softwareAgent"));
    falsy(data, "knowledgeBases", json!([]));

    let shell = if env.windows {
        env.comspec.clone().unwrap_or_else(|| "cmd.exe".into())
    } else {
        "/bin/bash".into()
    };
    falsy(data, "shell", json!(shell));

    falsy(
        data,
        "bedrockSettings",
        json!({
            "enableRegionFailover": false,
            "availableFailoverRegions": [],
            "enableInferenceProfiles": false,
            "visibleModelIds": []
        }),
    );
    falsy(
        data,
        "guardrailSettings",
        json!({
            "enabled": false,
            "guardrailIdentifier": "",
            "guardrailVersion": "DRAFT",
            "trace": "enabled"
        }),
    );
    missing(data, "lightProcessingModel", Value::Null);
    missing(data, "planMode", json!(false));

    // Old format stored `{ enabled: boolean }`; reset it to the container limits.
    let old_format = data
        .get("codeInterpreterTool")
        .and_then(|c| c.get("enabled"))
        .is_some_and(Value::is_boolean);
    if !truthy(data.get("codeInterpreterTool")) || old_format {
        data.insert("codeInterpreterTool".into(), code_interpreter_default());
    }

    falsy(
        data,
        "dockerSandboxTool",
        json!({ "memoryLimit": "2g", "cpuLimit": 2.0, "timeout": 300 }),
    );

    // `selectedVoiceId` is deliberately not initialized: Nova Sonic voice chat is
    // dropped. An existing value is left untouched.

    missing(data, "userEmoji", json!(""));
    missing(data, "userName", json!(""));
    missing(data, "sidebarHiddenItems", json!([]));
    missing(data, "hiddenDefaultAgentIds", json!([]));
    missing(data, "agentOrder", json!([]));
    missing(data, "appTheme", json!("dim"));

    // 'midnight' was renamed 'charcoal'. The renderer falls back to the default
    // theme for unknown values, so without this users who picked 'midnight'
    // would silently be switched to 'dim'.
    if data.get("appTheme").and_then(Value::as_str) == Some("midnight") {
        data.insert("appTheme".into(), json!("charcoal"));
    }

    missing(data, "appFontSans", json!("inter"));
    missing(data, "appFontMono", json!("jetbrains"));
}

/// `if (!value)` in TS.
fn falsy(data: &mut Map<String, Value>, key: &str, default: Value) {
    if !truthy(data.get(key)) {
        data.insert(key.to_string(), default);
    }
}

/// `if (value === undefined)` in TS.
fn missing(data: &mut Map<String, Value>, key: &str, default: Value) {
    data.entry(key.to_string()).or_insert(default);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix() -> Env {
        Env {
            windows: false,
            home: Some("/Users/me".into()),
            comspec: None,
        }
    }

    fn fresh(env: &Env) -> Map<String, Value> {
        let mut m = Map::new();
        init(&mut m, env);
        m
    }

    fn with(pairs: Value) -> Map<String, Value> {
        let mut m = pairs.as_object().unwrap().clone();
        init(&mut m, &unix());
        m
    }

    #[test]
    fn fresh_config_matches_ts_defaults_in_order() {
        let m = fresh(&unix());
        let keys: Vec<&str> = m.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "userDataPath",
                "projectPath",
                "advancedSetting",
                "language",
                "aws",
                "inferenceParams",
                "thinkingMode",
                "interleaveThinking",
                "customAgents",
                "selectedAgentId",
                "knowledgeBases",
                "shell",
                "bedrockSettings",
                "guardrailSettings",
                "lightProcessingModel",
                "planMode",
                "codeInterpreterTool",
                "dockerSandboxTool",
                "userEmoji",
                "userName",
                "sidebarHiddenItems",
                "hiddenDefaultAgentIds",
                "agentOrder",
                "appTheme",
                "appFontSans",
                "appFontMono",
            ]
        );
        assert_eq!(m["projectPath"], json!("/Users/me"));
        assert_eq!(m["shell"], json!("/bin/bash"));
        assert_eq!(
            m["inferenceParams"],
            json!({ "maxTokens": 4096, "temperature": 0.5, "topP": 0.9 })
        );
        assert_eq!(
            m["thinkingMode"],
            json!({ "type": "enabled", "budget_tokens": 4096 })
        );
        assert_eq!(m["dockerSandboxTool"]["memoryLimit"], json!("2g"));
        assert_eq!(m["guardrailSettings"]["guardrailVersion"], json!("DRAFT"));
        assert_eq!(m["lightProcessingModel"], Value::Null);
        assert!(!m.contains_key("selectedVoiceId"));
    }

    #[test]
    fn windows_shell_and_home() {
        let env = Env {
            windows: true,
            home: Some("C:\\Users\\me".into()),
            comspec: Some("C:\\Windows\\system32\\cmd.exe".into()),
        };
        let m = fresh(&env);
        assert_eq!(m["shell"], json!("C:\\Windows\\system32\\cmd.exe"));
        assert_eq!(m["projectPath"], json!("C:\\Users\\me"));

        let m = fresh(&Env {
            comspec: None,
            ..env
        });
        assert_eq!(m["shell"], json!("cmd.exe"));
    }

    #[test]
    fn missing_home_leaves_project_path_unset() {
        let m = fresh(&Env {
            home: None,
            ..unix()
        });
        assert!(!m.contains_key("projectPath"));
    }

    #[test]
    fn falsy_keys_are_reset() {
        let m = with(json!({
            "projectPath": "",
            "aws": null,
            "selectedAgentId": "",
            "shell": "",
            "customAgents": null,
            "thinkingMode": false,
            "inferenceParams": 0,
        }));
        assert_eq!(m["projectPath"], json!("/Users/me"));
        assert_eq!(m["aws"]["region"], json!("us-west-2"));
        assert_eq!(m["selectedAgentId"], json!("softwareAgent"));
        assert_eq!(m["shell"], json!("/bin/bash"));
        assert_eq!(m["customAgents"], json!([]));
        assert_eq!(m["thinkingMode"]["type"], json!("enabled"));
        assert_eq!(m["inferenceParams"]["maxTokens"], json!(4096));
    }

    #[test]
    fn undefined_only_keys_keep_falsy_values() {
        let m = with(json!({
            "language": "",
            "interleaveThinking": false,
            "lightProcessingModel": null,
            "planMode": false,
            "userEmoji": "",
            "userName": "",
            "appTheme": "",
        }));
        assert_eq!(m["language"], json!(""));
        assert_eq!(m["lightProcessingModel"], Value::Null);
        assert_eq!(m["userName"], json!(""));
        assert_eq!(m["appTheme"], json!(""));
    }

    #[test]
    fn empty_arrays_and_objects_are_truthy() {
        let m = with(json!({ "customAgents": [], "aws": {} }));
        assert_eq!(m["customAgents"], json!([]));
        assert_eq!(m["aws"], json!({}));
    }

    #[test]
    fn existing_values_preserved() {
        let m = with(json!({
            "aws": { "region": "eu-west-1", "profile": "dev" },
            "selectedAgentId": "custom",
            "agentOrder": ["a", "b"],
        }));
        assert_eq!(m["aws"], json!({ "region": "eu-west-1", "profile": "dev" }));
        assert_eq!(m["selectedAgentId"], json!("custom"));
        assert_eq!(m["agentOrder"], json!(["a", "b"]));
    }

    #[test]
    fn advanced_setting_replaced_when_keybinding_missing() {
        let m = with(json!({ "advancedSetting": { "other": 1 } }));
        assert_eq!(
            m["advancedSetting"],
            json!({ "keybinding": { "sendMsgKey": "Enter" } })
        );
        let m = with(
            json!({ "advancedSetting": { "keybinding": { "sendMsgKey": "Cmd+Enter" }, "other": 1 } }),
        );
        assert_eq!(m["advancedSetting"]["other"], json!(1));
        assert_eq!(
            m["advancedSetting"]["keybinding"]["sendMsgKey"],
            json!("Cmd+Enter")
        );
    }

    #[test]
    fn code_interpreter_old_format_migrated() {
        let m = with(json!({ "codeInterpreterTool": { "enabled": true } }));
        assert_eq!(m["codeInterpreterTool"], code_interpreter_default());
        let custom = json!({ "memoryLimit": "1g", "cpuLimit": 1.0, "timeout": 60 });
        let m = with(json!({ "codeInterpreterTool": custom.clone() }));
        assert_eq!(m["codeInterpreterTool"], custom);
    }

    #[test]
    fn midnight_migrated_to_charcoal_others_untouched() {
        assert_eq!(
            with(json!({ "appTheme": "midnight" }))["appTheme"],
            json!("charcoal")
        );
        assert_eq!(
            with(json!({ "appTheme": "light" }))["appTheme"],
            json!("light")
        );
    }

    #[test]
    fn selected_voice_id_preserved_not_created() {
        assert!(!fresh(&unix()).contains_key("selectedVoiceId"));
        let m = with(json!({ "selectedVoiceId": "tiffany" }));
        assert_eq!(m["selectedVoiceId"], json!("tiffany"));
    }

    #[test]
    fn init_is_idempotent() {
        let once = fresh(&unix());
        let mut twice = once.clone();
        init(&mut twice, &unix());
        assert_eq!(once, twice);
    }
}
