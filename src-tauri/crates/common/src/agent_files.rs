//! Agent config files — the filesystem logic of `src/main/handlers/agent-handlers.ts`.
//!
//! Covers shared agents under `<project>/.bedrock-engineer/agents/` (read, save, delete),
//! exporting an agent to a portable YAML file, importing an agent file, and the pure halves of
//! the organization (S3) handlers. Dialogs, the store, and S3 I/O stay in the app crate: every
//! function here takes the project path / chosen file path explicitly and returns the same JSON
//! shape the IPC handler returned.

use crate::validation::{validate_custom_agent, ValidationContext};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::path::{Component, Path, PathBuf};

const CATEGORY: &str = "agents:ipc";

/// Shared agents directory, relative to the project path.
pub const SHARED_AGENTS_SUBDIR: &str = ".bedrock-engineer/agents";

/// Agent config file formats accepted when reading or importing.
pub const AGENT_FILE_EXTENSIONS: &[&str] = &["yaml", "yml", "json"];

/// Fields that describe *this copy* of an agent rather than the agent itself; stripped before an
/// agent is exported so the file stays portable.
pub const INSTANCE_ONLY_AGENT_FIELDS: &[&str] = &[
    "id",
    "isShared",
    "isCustom",
    "directoryOnly",
    "organizationId",
    "sharedFilePath",
    "mcpTools",
];

/// File format for saved agents (`options.format`, default YAML).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentFileFormat {
    Json,
    #[default]
    Yaml,
}

impl AgentFileFormat {
    pub fn extension(self) -> &'static str {
        match self {
            AgentFileFormat::Json => ".json",
            AgentFileFormat::Yaml => ".yaml",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            AgentFileFormat::Json => "application/json",
            AgentFileFormat::Yaml => "application/x-yaml",
        }
    }
}

/// The union of the shapes the agent file handlers return; absent fields are omitted.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileResult {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canceled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<AgentFileFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s3_key: Option<String>,
}

impl AgentFileResult {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(message.into()),
            ..Default::default()
        }
    }

    pub fn canceled() -> Self {
        Self {
            success: false,
            canceled: Some(true),
            ..Default::default()
        }
    }
}

/// `{ agents, error }` returned by `read-shared-agents` / `load-organization-agents`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AgentListResult {
    pub agents: Vec<Value>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------------------------

/// JS `name.toLowerCase().replace(/[^a-z0-9]+/g, '-')`.
fn dash_runs(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut in_run = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    out
}

/// JS `.replace(/(^-|-$)/g, '')` — removes at most one leading and one trailing dash.
fn trim_one_dash(s: &str) -> &str {
    let s = s.strip_prefix('-').unwrap_or(s);
    s.strip_suffix('-').unwrap_or(s)
}

/// Turn an agent name into a filename stem (`toAgentFileSlug`); `custom-agent` when empty.
pub fn to_agent_file_slug(name: &str) -> String {
    let slug = trim_one_dash(&dash_runs(name)).to_string();
    if slug.is_empty() {
        "custom-agent".to_string()
    } else {
        slug
    }
}

/// `Math.random().toString(36).substring(2, 9)` — seven random base-36 characters.
fn random_base36_7() -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    let mut n = hasher.finish();
    let mut s = String::new();
    for _ in 0..7 {
        s.push(std::char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    s
}

fn to_base36(mut n: u128) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut digits = Vec::new();
    while n > 0 {
        digits.push(std::char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    digits.iter().rev().collect()
}

/// `Date.now().toString(36)`
fn now_base36() -> String {
    to_base36(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default(),
    )
}

/// Node's `path.resolve(p)`: absolute against the cwd, with `.` and `..` folded lexically.
pub fn resolve_path(p: &Path) -> PathBuf {
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The shared agents directory for a project, or `None` when no project folder is selected.
pub fn shared_agents_dir(project_path: Option<&Path>) -> Option<PathBuf> {
    project_path
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| resolve_path(&p.join(SHARED_AGENTS_SUBDIR)))
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn has_agent_extension(name: &str) -> bool {
    name.ends_with(".json") || name.ends_with(".yml") || name.ends_with(".yaml")
}

/// Strip `.json`, `.yml` or `.yaml` (`/\.(json|ya?ml)$/`).
fn strip_agent_extension(name: &str) -> &str {
    for ext in [".json", ".yml", ".yaml"] {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem;
        }
    }
    name
}

/// Parse YAML text into JSON. An empty document is `null` (js-yaml returns `undefined`).
pub fn parse_yaml(content: &str) -> Result<Value, String> {
    if content.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_norway::from_str::<Value>(content).map_err(|e| e.to_string())
}

/// Serialize an agent to YAML (the counterpart of `yaml.dump(agent, { indent: 2, ... })`).
pub fn dump_yaml(value: &Value) -> Result<String, String> {
    serde_norway::to_string(value).map_err(|e| e.to_string())
}

/// `parseAgentFile`: JSON for `.json`, YAML otherwise.
pub fn parse_agent_file(content: &str, file_path: &str) -> Result<Value, String> {
    if file_path.ends_with(".json") {
        serde_json::from_str(content).map_err(|e| e.to_string())
    } else {
        parse_yaml(content)
    }
}

fn serialize_agent(agent: &Value, format: AgentFileFormat) -> Result<String, String> {
    match format {
        AgentFileFormat::Json => serde_json::to_string_pretty(agent).map_err(|e| e.to_string()),
        AgentFileFormat::Yaml => dump_yaml(agent),
    }
}

fn agent_name(agent: &Value) -> Result<&str, String> {
    agent
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Cannot read properties of agent: name is not a string".to_string())
}

// ---------------------------------------------------------------------------------------------
// shared agents
// ---------------------------------------------------------------------------------------------

/// Prepare one shared agent file's parsed content the way `loadSharedAgents` does.
fn prepare_shared_agent(mut agent: Value, file: &str, file_path: &Path) -> Result<Value, String> {
    let obj = agent
        .as_object_mut()
        .ok_or_else(|| format!("{file} does not contain an agent object"))?;

    let needs_id = match obj.get("id") {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.is_empty() || !s.starts_with("shared-"),
        Some(Value::Bool(false)) => true,
        Some(_) => return Err("agent.id.startsWith is not a function".to_string()),
    };
    if needs_id {
        let safe_name = strip_agent_extension(file).to_lowercase();
        obj.insert(
            "id".into(),
            Value::String(format!("shared-{safe_name}-{}", random_base36_7())),
        );
    }
    obj.insert("isShared".into(), Value::Bool(true));
    obj.insert(
        "sharedFilePath".into(),
        Value::String(file_path.to_string_lossy().into_owned()),
    );
    obj.remove("mcpTools");

    let result = validate_custom_agent(
        &agent,
        Some(&ValidationContext {
            source: Some("shared-agents"),
            file_path: Some(file),
        }),
    );
    Ok(result.data)
}

/// `read-shared-agents`: load every `.json` / `.yml` / `.yaml` file in the project's shared
/// agents directory. Unreadable files are logged and skipped. Files are returned in name order.
pub fn load_shared_agents(project_path: Option<&Path>) -> AgentListResult {
    let Some(agents_dir) = shared_agents_dir(project_path) else {
        return AgentListResult::default();
    };
    if !agents_dir.exists() {
        return AgentListResult::default();
    }

    let entries = match fs::read_dir(&agents_dir) {
        Ok(entries) => entries,
        Err(e) => {
            return AgentListResult {
                agents: Vec::new(),
                error: Some(e.to_string()),
            }
        }
    };
    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| has_agent_extension(name))
        .collect();
    files.sort();

    let agents = files
        .iter()
        .filter_map(|file| {
            let file_path = agents_dir.join(file);
            let loaded = fs::read_to_string(&file_path)
                .map_err(|e| e.to_string())
                .and_then(|content| parse_agent_file(&content, file))
                .and_then(|agent| prepare_shared_agent(agent, file, &file_path));
            match loaded {
                Ok(agent) => Some(agent),
                Err(error) => {
                    tracing::error!(category = CATEGORY, file = %file, error = %error, "Error reading agent file");
                    None
                }
            }
        })
        .collect();

    AgentListResult {
        agents,
        error: None,
    }
}

/// `save-shared-agent`: write `agent` into the shared agents directory under a unique
/// filename derived from its name, with a fresh `shared-…` id.
pub fn save_shared_agent(
    project_path: Option<&Path>,
    agent: &Value,
    format: AgentFileFormat,
) -> AgentFileResult {
    let Some(agents_dir) = shared_agents_dir(project_path) else {
        return AgentFileResult::error("No project path selected");
    };
    match save_shared_agent_inner(&agents_dir, agent, format) {
        Ok(file_path) => AgentFileResult {
            success: true,
            file_path: Some(file_path.to_string_lossy().into_owned()),
            format: Some(format),
            ..Default::default()
        },
        Err(e) => {
            tracing::error!(category = CATEGORY, error = %e, "Error saving shared agent");
            AgentFileResult::error(e)
        }
    }
}

fn save_shared_agent_inner(
    agents_dir: &Path,
    agent: &Value,
    format: AgentFileFormat,
) -> Result<PathBuf, String> {
    fs::create_dir_all(agents_dir).map_err(|e| e.to_string())?;
    let name = agent_name(agent)?;
    let safe_file_name = to_agent_file_slug(name);
    let ext = format.extension();

    let mut file_name = format!("{safe_file_name}{ext}");
    let mut count = 1;
    while agents_dir.join(&file_name).exists() {
        file_name = format!("{safe_file_name}-{count}{ext}");
        count += 1;
    }

    let new_id = format!("shared-{}-{}", dash_runs(name), now_base36());
    let mut shared = agent.as_object().cloned().unwrap_or_default();
    shared.insert("id".into(), Value::String(new_id));
    shared.insert("isShared".into(), Value::Bool(true));
    shared.remove("mcpTools");
    shared.remove("sharedFilePath");

    let file_path = agents_dir.join(&file_name);
    let content = serialize_agent(&Value::Object(shared), format)?;
    fs::write(&file_path, content).map_err(|e| e.to_string())?;
    Ok(file_path)
}

/// What the user is asked before a shared agent file is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePrompt {
    pub file_path: PathBuf,
    /// Buttons are `["Cancel", "Delete"]`, default and cancel = Cancel, type `warning`.
    pub message: String,
    pub detail: String,
}

/// First half of `delete-shared-agent`: check that `file_path` is a file directly inside this
/// project's shared agents directory. `Err` carries the result to return without prompting.
pub fn prepare_shared_agent_delete(
    project_path: Option<&Path>,
    file_path: &Path,
) -> Result<DeletePrompt, Box<AgentFileResult>> {
    let Some(agents_dir) = shared_agents_dir(project_path) else {
        return Err(Box::new(AgentFileResult::error("No project path selected")));
    };
    let file_path = resolve_path(file_path);
    if file_path.parent() != Some(agents_dir.as_path()) {
        tracing::warn!(
            category = CATEGORY,
            file_path = %file_path.display(),
            agents_dir = %agents_dir.display(),
            "Refused to delete a file outside the shared agents directory"
        );
        return Err(Box::new(AgentFileResult::error(
            "That file is not a shared agent of this project",
        )));
    }
    if !file_path.exists() {
        return Err(Box::new(AgentFileResult::error(format!(
            "File no longer exists: {}",
            file_path.display()
        ))));
    }
    Ok(DeletePrompt {
        message: format!(
            "Delete the shared agent file \"{}\"?",
            file_name_of(&file_path)
        ),
        detail: format!(
            "{}\n\nThe agent stops appearing for anyone who opens this project. Your own copy of the agent is not affected.",
            file_path.display()
        ),
        file_path,
    })
}

/// Second half of `delete-shared-agent`, once the user confirmed.
pub fn delete_confirmed_shared_agent(prompt: &DeletePrompt) -> AgentFileResult {
    match fs::remove_file(&prompt.file_path) {
        Ok(()) => {
            tracing::info!(category = CATEGORY, file_path = %prompt.file_path.display(), "Deleted shared agent file");
            AgentFileResult {
                success: true,
                file_path: Some(prompt.file_path.to_string_lossy().into_owned()),
                ..Default::default()
            }
        }
        Err(e) => {
            tracing::error!(category = CATEGORY, file_path = %prompt.file_path.display(), error = %e, "Error deleting shared agent file");
            AgentFileResult::error(e.to_string())
        }
    }
}

/// The whole `delete-shared-agent` flow; `confirm` shows the prompt and returns true for Delete.
pub fn delete_shared_agent(
    project_path: Option<&Path>,
    file_path: &Path,
    confirm: impl FnOnce(&DeletePrompt) -> bool,
) -> AgentFileResult {
    match prepare_shared_agent_delete(project_path, file_path) {
        Err(result) => *result,
        Ok(prompt) => {
            if confirm(&prompt) {
                delete_confirmed_shared_agent(&prompt)
            } else {
                AgentFileResult::canceled()
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// export / import
// ---------------------------------------------------------------------------------------------

/// The filename `export-agent-yaml` offers in the save dialog (`<slug>.yaml`).
pub fn export_default_file_name(agent_name: &str) -> String {
    format!("{}.yaml", to_agent_file_slug(agent_name))
}

/// `agent` without [`INSTANCE_ONLY_AGENT_FIELDS`].
pub fn portable_agent(agent: &Value) -> Value {
    let mut obj: Map<String, Value> = agent.as_object().cloned().unwrap_or_default();
    for field in INSTANCE_ONLY_AGENT_FIELDS {
        obj.remove(*field);
    }
    Value::Object(obj)
}

/// Pre-dialog check of `export-agent-yaml`: `Err` when there is no agent name.
pub fn check_exportable(agent: Option<&Value>) -> Result<&str, Box<AgentFileResult>> {
    agent
        .and_then(|a| a.get("name"))
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| Box::new(AgentFileResult::error("No agent to export")))
}

/// Post-dialog half of `export-agent-yaml`: write the portable agent to `target`.
pub fn export_agent_yaml(agent: &Value, target: &Path) -> AgentFileResult {
    let result = dump_yaml(&portable_agent(agent))
        .and_then(|content| fs::write(target, content).map_err(|e| e.to_string()));
    match result {
        Ok(()) => {
            tracing::info!(
                category = CATEGORY,
                agent_name = agent.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
                file_path = %target.display(),
                "Exported agent YAML"
            );
            AgentFileResult {
                success: true,
                file_path: Some(target.to_string_lossy().into_owned()),
                ..Default::default()
            }
        }
        Err(e) => {
            tracing::error!(category = CATEGORY, error = %e, "Error exporting agent YAML");
            AgentFileResult::error(e)
        }
    }
}

/// Post-dialog half of `import-agent-file`: read and check the file the user picked.
///
/// Fails closed on unparseable files, non-object content, and a missing/blank `name`,
/// `description` or `system`. The full schema check is a logged warning only.
pub fn import_agent_file(file_path: &Path) -> AgentFileResult {
    let path_str = file_path.to_string_lossy().into_owned();
    let base = file_name_of(file_path);
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(category = CATEGORY, error = %e, "Error importing agent file");
            return AgentFileResult::error(e.to_string());
        }
    };

    let parsed = match parse_agent_file(&content, &path_str) {
        Ok(v) => v,
        Err(e) => {
            let kind = if path_str.ends_with(".json") {
                "JSON"
            } else {
                "YAML"
            };
            return AgentFileResult::error(format!("{base} is not valid {kind}: {e}"));
        }
    };

    let Value::Object(mut agent) = parsed else {
        return AgentFileResult::error(format!("{base} does not contain an agent"));
    };
    if agent.get("id").is_none_or(Value::is_null) {
        agent.insert("id".into(), Value::String(String::new()));
    }
    if agent.get("scenarios").is_none_or(Value::is_null) {
        agent.insert("scenarios".into(), Value::Array(Vec::new()));
    }
    agent.remove("mcpTools");
    let agent = Value::Object(agent);

    validate_custom_agent(
        &agent,
        Some(&ValidationContext {
            source: Some("import-agent"),
            file_path: Some(&path_str),
        }),
    );

    let missing: Vec<&str> = ["name", "description", "system"]
        .into_iter()
        .filter(|field| {
            agent
                .get(*field)
                .and_then(Value::as_str)
                .is_none_or(|s| s.trim().is_empty())
        })
        .collect();
    if !missing.is_empty() {
        return AgentFileResult::error(format!(
            "{base} is missing required agent {}: {}",
            if missing.len() == 1 {
                "field"
            } else {
                "fields"
            },
            missing.join(", ")
        ));
    }

    tracing::info!(
        category = CATEGORY,
        agent_name = agent.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
        file_path = %path_str,
        "Imported agent file"
    );
    AgentFileResult {
        success: true,
        agent: Some(agent),
        file_path: Some(path_str),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------------
// organization (S3) — pure halves; the app crate does the S3 calls
// ---------------------------------------------------------------------------------------------

/// Whether an S3 key is an agent file (`.yaml`, `.yml`, `.json`).
pub fn is_agent_file_key(key: &str) -> bool {
    has_agent_extension(key)
}

/// Turn one downloaded organization agent file into the agent the renderer sees
/// (`load-organization-agents`, per file).
pub fn organization_agent_from_content(
    content: &str,
    key: &str,
    organization_id: &str,
) -> Result<Value, String> {
    let mut agent = parse_agent_file(content, key)?;
    let obj = agent
        .as_object_mut()
        .ok_or_else(|| format!("{key} does not contain an agent object"))?;
    let org_id = format!(
        "org-{organization_id}-{}-{}",
        strip_agent_extension(key).to_lowercase(),
        random_base36_7()
    );
    obj.insert("id".into(), Value::String(org_id));
    obj.insert("isShared".into(), Value::Bool(false));
    obj.insert("isCustom".into(), Value::Bool(false));
    obj.insert("directoryOnly".into(), Value::Bool(false));
    obj.insert(
        "organizationId".into(),
        Value::String(organization_id.to_string()),
    );
    obj.remove("mcpTools");

    Ok(validate_custom_agent(
        &agent,
        Some(&ValidationContext {
            source: Some("organization-agents"),
            file_path: Some(key),
        }),
    )
    .data)
}

/// An organization upload prepared by [`organization_agent_upload`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationUpload {
    pub s3_key: String,
    pub body: String,
    pub content_type: &'static str,
}

/// Build the S3 key and body for `save-agent-to-organization`.
pub fn organization_agent_upload(
    agent: &Value,
    organization_id: &str,
    prefix: Option<&str>,
    format: AgentFileFormat,
) -> Result<OrganizationUpload, String> {
    let name = agent_name(agent)?;
    let safe_file_name = to_agent_file_slug(name);
    let ext = format.extension();
    let s3_key = match prefix.filter(|p| !p.is_empty()) {
        Some(prefix) => format!("{prefix}/{safe_file_name}{ext}"),
        None => format!("{safe_file_name}{ext}"),
    };
    let mut shared = agent.as_object().cloned().unwrap_or_default();
    shared.insert(
        "id".into(),
        Value::String(format!("shared-{}-{}", dash_runs(name), now_base36())),
    );
    shared.insert("isShared".into(), Value::Bool(true));
    shared.insert(
        "organizationId".into(),
        Value::String(organization_id.to_string()),
    );
    shared.remove("mcpTools");
    shared.remove("sharedFilePath");
    Ok(OrganizationUpload {
        s3_key,
        body: serialize_agent(&Value::Object(shared), format)?,
        content_type: format.content_type(),
    })
}

#[cfg(test)]
mod tests {
    //! Port of `src/main/handlers/agent-handlers.test.ts`. The Electron dialogs the TS test mocks
    //! are the app crate's job; here the "dialog result" is the path passed in.
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn agent() -> Value {
        json!({
            "id": "custom_agent_abc12345",
            "name": "Release Notes Writer",
            "description": "Turns a diff into release notes",
            "system": "You write release notes.",
            "scenarios": [],
            "tools": ["readFiles"],
            "isCustom": true
        })
    }

    struct Project {
        dir: TempDir,
    }

    impl Project {
        fn new() -> Self {
            let dir = tempfile::Builder::new()
                .prefix("agent-handlers-")
                .tempdir()
                .unwrap();
            fs::create_dir_all(dir.path().join(".bedrock-engineer").join("agents")).unwrap();
            Self { dir }
        }
        fn path(&self) -> &Path {
            self.dir.path()
        }
        fn agents_dir(&self) -> PathBuf {
            resolve_path(&self.path().join(".bedrock-engineer").join("agents"))
        }
        fn write_shared_agent(&self, file_name: &str, agent: &Value) -> PathBuf {
            let p = self.agents_dir().join(file_name);
            fs::write(&p, dump_yaml(agent).unwrap()).unwrap();
            p
        }
    }

    // describe('read-shared-agents')
    #[test]
    fn reports_the_file_each_agent_came_from() {
        let project = Project::new();
        let file_path = project.write_shared_agent("writer.yaml", &agent());

        let result = load_shared_agents(Some(project.path()));

        assert_eq!(result.agents.len(), 1);
        assert_eq!(
            result.agents[0]["sharedFilePath"],
            json!(file_path.to_string_lossy())
        );
        assert_eq!(result.agents[0]["isShared"], json!(true));
        assert!(result.agents[0]["id"]
            .as_str()
            .unwrap()
            .starts_with("shared-writer-"));
    }

    #[test]
    fn no_project_means_no_shared_agents() {
        assert_eq!(load_shared_agents(None), AgentListResult::default());
    }

    // describe('save-shared-agent')
    #[test]
    fn does_not_write_shared_file_path_into_the_file_it_creates() {
        let project = Project::new();
        let mut a = agent();
        a["sharedFilePath"] = json!("/somewhere/else/writer.yaml");

        let result = save_shared_agent(Some(project.path()), &a, AgentFileFormat::Yaml);

        assert!(result.success);
        let written = parse_yaml(&fs::read_to_string(result.file_path.unwrap()).unwrap()).unwrap();
        assert!(written.get("sharedFilePath").is_none());
        assert_eq!(written["isShared"], json!(true));
    }

    #[test]
    fn save_adds_a_suffix_when_the_name_is_taken() {
        let project = Project::new();
        let first = save_shared_agent(Some(project.path()), &agent(), AgentFileFormat::Json);
        let second = save_shared_agent(Some(project.path()), &agent(), AgentFileFormat::Json);
        assert!(first
            .file_path
            .unwrap()
            .ends_with("release-notes-writer.json"));
        assert!(second
            .file_path
            .unwrap()
            .ends_with("release-notes-writer-1.json"));
    }

    // describe('export-agent-yaml')
    #[test]
    fn writes_yaml_without_the_fields_that_describe_this_particular_copy() {
        let project = Project::new();
        let target = project.path().join("exported.yaml");
        let mut a = agent();
        a["isShared"] = json!(true);
        a["directoryOnly"] = json!(false);
        a["organizationId"] = json!("org-1");
        a["sharedFilePath"] = json!("/project/.bedrock-engineer/agents/writer.yaml");
        a["mcpTools"] = json!([{ "toolSpec": { "name": "whatever" } }]);

        let result = export_agent_yaml(&a, &target);

        assert_eq!(
            result,
            AgentFileResult {
                success: true,
                file_path: Some(target.to_string_lossy().into_owned()),
                ..Default::default()
            }
        );
        let written = parse_yaml(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({
                "name": "Release Notes Writer",
                "description": "Turns a diff into release notes",
                "system": "You write release notes.",
                "scenarios": [],
                "tools": ["readFiles"]
            })
        );
    }

    #[test]
    fn reports_cancellation_rather_than_an_error() {
        // The app returns this when the save dialog is dismissed.
        assert_eq!(
            serde_json::to_value(AgentFileResult::canceled()).unwrap(),
            json!({ "success": false, "canceled": true })
        );
    }

    #[test]
    fn offers_the_agent_name_as_the_filename() {
        assert_eq!(
            export_default_file_name("Release Notes Writer"),
            "release-notes-writer.yaml"
        );
        assert_eq!(check_exportable(Some(&agent())), Ok("Release Notes Writer"));
        assert_eq!(
            check_exportable(Some(&json!({}))),
            Err(Box::new(AgentFileResult::error("No agent to export")))
        );
    }

    // describe('delete-shared-agent')
    #[test]
    fn deletes_the_file_once_the_user_confirms() {
        let project = Project::new();
        let file_path = project.write_shared_agent("writer.yaml", &agent());

        let result = delete_shared_agent(Some(project.path()), &file_path, |_| true);

        assert_eq!(
            result,
            AgentFileResult {
                success: true,
                file_path: Some(file_path.to_string_lossy().into_owned()),
                ..Default::default()
            }
        );
        assert!(!file_path.exists());
    }

    #[test]
    fn leaves_the_file_alone_when_the_user_cancels() {
        let project = Project::new();
        let file_path = project.write_shared_agent("writer.yaml", &agent());

        let result = delete_shared_agent(Some(project.path()), &file_path, |_| false);

        assert_eq!(result, AgentFileResult::canceled());
        assert!(file_path.exists());
    }

    #[test]
    fn refuses_a_path_outside_the_shared_agents_directory_without_prompting() {
        let project = Project::new();
        let outside = project.path().join("important.yaml");
        fs::write(&outside, "keep me").unwrap();
        let mut prompted = false;

        let result = delete_shared_agent(Some(project.path()), &outside, |_| {
            prompted = true;
            true
        });

        assert!(!result.success);
        assert!(!prompted);
        assert!(outside.exists());
    }

    #[test]
    fn refuses_a_path_that_escapes_the_shared_agents_directory_with_dotdot() {
        let project = Project::new();
        let outside = project.path().join("important.yaml");
        fs::write(&outside, "keep me").unwrap();

        let escaping = project
            .agents_dir()
            .join("..")
            .join("..")
            .join("important.yaml");
        let result = delete_shared_agent(Some(project.path()), &escaping, |_| true);

        assert!(!result.success);
        assert!(outside.exists());
    }

    // describe('import-agent-file')
    fn pick_file(project: &Project, file_name: &str, content: &str) -> PathBuf {
        let p = project.path().join(file_name);
        fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn accepts_a_file_produced_by_export_agent_yaml() {
        let project = Project::new();
        let exported = project.path().join("exported.yaml");
        export_agent_yaml(&agent(), &exported);

        let result = import_agent_file(&exported);

        assert!(result.success);
        let a = result.agent.unwrap();
        assert_eq!(a["name"], json!("Release Notes Writer"));
        assert_eq!(a["system"], json!("You write release notes."));
        // The renderer assigns the real id; the file carries none
        assert_eq!(a["id"], json!(""));
    }

    #[test]
    fn reads_json_as_well_as_yaml() {
        let project = Project::new();
        let p = pick_file(&project, "agent.json", &agent().to_string());

        let result = import_agent_file(&p);

        assert!(result.success);
        assert_eq!(result.agent.unwrap()["name"], json!("Release Notes Writer"));
    }

    #[test]
    fn rejects_a_file_missing_the_fields_an_agent_cannot_work_without() {
        let project = Project::new();
        let content = dump_yaml(&json!({ "name": "Half An Agent", "description": "" })).unwrap();
        let p = pick_file(&project, "partial.yaml", &content);

        let result = import_agent_file(&p);

        assert!(!result.success);
        let error = result.error.unwrap();
        assert!(error.contains("description"));
        assert!(error.contains("system"));
        assert!(result.agent.is_none());
    }

    #[test]
    fn rejects_a_file_that_does_not_contain_an_agent_at_all() {
        let project = Project::new();
        let content = dump_yaml(&json!(["not", "an", "agent"])).unwrap();
        let p = pick_file(&project, "list.yaml", &content);

        let result = import_agent_file(&p);

        assert!(!result.success);
        assert!(result.error.unwrap().contains("does not contain an agent"));
    }

    #[test]
    fn reports_unparseable_yaml_with_the_file_name() {
        let project = Project::new();
        let p = pick_file(&project, "broken.yaml", "name: [unclosed\n");

        let result = import_agent_file(&p);

        assert!(!result.success);
        assert!(result.error.unwrap().contains("broken.yaml"));
    }

    #[test]
    fn reports_cancellation_when_the_picker_is_dismissed() {
        assert_eq!(
            serde_json::to_value(AgentFileResult::canceled()).unwrap(),
            json!({ "success": false, "canceled": true })
        );
    }

    // Organization helpers (no TS test; logic from the handlers)
    #[test]
    fn organization_agents_are_marked_and_keys_built() {
        let a = organization_agent_from_content(
            &dump_yaml(&agent()).unwrap(),
            "team/Writer.yaml",
            "acme",
        )
        .unwrap();
        assert!(a["id"]
            .as_str()
            .unwrap()
            .starts_with("org-acme-team/writer-"));
        assert_eq!(a["organizationId"], json!("acme"));
        assert_eq!(a["isCustom"], json!(false));

        let up = organization_agent_upload(&agent(), "acme", Some("team"), AgentFileFormat::Yaml)
            .unwrap();
        assert_eq!(up.s3_key, "team/release-notes-writer.yaml");
        assert_eq!(up.content_type, "application/x-yaml");
        let body = parse_yaml(&up.body).unwrap();
        assert_eq!(body["organizationId"], json!("acme"));
        assert!(body["id"]
            .as_str()
            .unwrap()
            .starts_with("shared-release-notes-writer-"));
    }

    #[test]
    fn slug_matches_js_regex() {
        assert_eq!(to_agent_file_slug("  Hello, World!  "), "hello-world");
        assert_eq!(to_agent_file_slug("***"), "custom-agent");
        assert_eq!(dash_runs("A b"), "a-b");
    }
}
