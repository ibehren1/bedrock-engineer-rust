//! Filesystem logic behind the file commands, kept free of Tauri so it can be tested with
//! `tempfile`. Ports of `src/main/handlers/file-handlers.ts` and the self-contained parts of
//! `src/preload/file.ts`.

use base64::Engine;
use common::agent_files::parse_agent_file;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const CATEGORY: &str = "file:ipc";

/// `{ agents, error? }` returned by `readSharedAgents` / `readDirectoryAgents`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AgentList {
    pub agents: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AgentList {
    fn error(message: impl Into<String>) -> Self {
        Self {
            agents: Vec::new(),
            error: Some(message.into()),
        }
    }
}

/// `{ success, filePath?, directory?, error? }` of the chat export handlers.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportImage {
    pub filename: String,
    pub base64: String,
}

/// `sanitizeForFilesystem`: strip reserved and control characters, collapse whitespace, drop
/// trailing dots, trim, and cap at 100 UTF-16 code units.
pub fn sanitize_for_filesystem(name: &str) -> String {
    let stripped: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .filter(|c| !('\u{0000}'..='\u{001f}').contains(c))
        .collect();
    // `.replace(/\s+/g, ' ')`
    let mut collapsed = String::with_capacity(stripped.len());
    let mut in_ws = false;
    for c in stripped.chars() {
        if is_js_whitespace(c) {
            if !in_ws {
                collapsed.push(' ');
            }
            in_ws = true;
        } else {
            collapsed.push(c);
            in_ws = false;
        }
    }
    // `.replace(/\.+$/, '')`, then `.trim()`
    let trimmed = collapsed
        .trim_end_matches('.')
        .trim_matches(is_js_whitespace);
    // `.slice(0, 100)` counts UTF-16 code units; never split a character.
    let mut out = String::new();
    let mut units = 0;
    for c in trimmed.chars() {
        units += c.len_utf16();
        if units > 100 {
            break;
        }
        out.push(c);
    }
    out
}

/// JS `\s` / `String.prototype.trim` whitespace.
fn is_js_whitespace(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// Node's `Buffer.from(s, 'base64')`: lenient about padding, whitespace, and the URL-safe
/// alphabet; stops at the first invalid character instead of failing.
pub fn decode_base64_lenient(s: &str) -> Vec<u8> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/')
        .collect();
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    // A dangling single character carries no full byte; Node ignores it.
    let usable = if cleaned.len() % 4 == 1 {
        &cleaned[..cleaned.len() - 1]
    } else {
        &cleaned[..]
    };
    engine.decode(usable).unwrap_or_default()
}

/// `save-chat-to-markdown`: `<projectPath>/<title>/<title>.md`, images under `images/`.
pub fn save_chat_to_markdown(
    project_path: &Path,
    title: &str,
    markdown: &str,
    images: &[ExportImage],
) -> ExportResult {
    let safe_title = match sanitize_for_filesystem(title) {
        t if t.is_empty() => "chat-export".to_string(),
        t => t,
    };
    let export_dir = project_path.join(&safe_title);
    let images_dir = export_dir.join("images");
    let result = (|| -> std::io::Result<std::path::PathBuf> {
        // images/ only when there are images, so a Mermaid-only chat leaves no empty folder.
        fs::create_dir_all(if images.is_empty() {
            &export_dir
        } else {
            &images_dir
        })?;
        for image in images {
            fs::write(
                images_dir.join(&image.filename),
                decode_base64_lenient(&image.base64),
            )?;
        }
        let markdown_path = export_dir.join(format!("{safe_title}.md"));
        fs::write(&markdown_path, markdown)?;
        Ok(markdown_path)
    })();
    match result {
        Ok(markdown_path) => {
            tracing::info!(category = CATEGORY, markdown_path = %markdown_path.display(), image_count = images.len(), "Chat exported to markdown successfully");
            ExportResult {
                success: true,
                file_path: Some(markdown_path.to_string_lossy().into_owned()),
                directory: Some(export_dir.to_string_lossy().into_owned()),
                error: None,
            }
        }
        Err(e) => {
            tracing::error!(category = CATEGORY, title, error = %e, "Failed to export chat to markdown");
            ExportResult {
                success: false,
                error: Some(e.to_string()),
                ..Default::default()
            }
        }
    }
}

/// The export folder of `save-chat-to-docx` / `save-chat-to-pdf` (`<projectPath>/<title>/`, same
/// as the markdown export) and the document path in it, `<title>.<extension>`.
pub fn chat_export_paths(project_path: &Path, title: &str, extension: &str) -> (PathBuf, PathBuf) {
    let safe_title = match sanitize_for_filesystem(title) {
        t if t.is_empty() => "chat-export".to_string(),
        t => t,
    };
    let export_dir = project_path.join(&safe_title);
    let file = export_dir.join(format!("{safe_title}.{extension}"));
    (export_dir, file)
}

/// Write a finished chat document (`kind` is `docx` or `pdf`, also its extension) to
/// [`chat_export_paths`], creating the folder. `bytes` is where the document came from; its
/// error is reported the same way as a write failure.
pub fn save_chat_document(
    project_path: &Path,
    title: &str,
    kind: &str,
    bytes: Result<Vec<u8>, String>,
) -> ExportResult {
    let (export_dir, file) = chat_export_paths(project_path, title, kind);
    let result = fs::create_dir_all(&export_dir)
        .map_err(|e| e.to_string())
        .and(bytes)
        .and_then(|b| fs::write(&file, b).map_err(|e| e.to_string()));
    match result {
        Ok(()) => {
            tracing::info!(category = CATEGORY, path = %file.display(), "Chat exported to {kind} successfully");
            ExportResult {
                success: true,
                file_path: Some(file.to_string_lossy().into_owned()),
                directory: Some(export_dir.to_string_lossy().into_owned()),
                error: None,
            }
        }
        Err(e) => {
            tracing::error!(category = CATEGORY, title, error = %e, "Failed to export chat to {kind}");
            ExportResult {
                success: false,
                error: Some(e),
                ..Default::default()
            }
        }
    }
}

/// `get-local-image`: the file as a `data:image/<ext>;base64,…` URL. The extension is whatever
/// follows the last `.` of the path, lowercased (`png` if the path ends with a dot).
pub fn local_image_data_url(path: &str) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    let ext = if ext.is_empty() { "png".into() } else { ext };
    Ok(format!(
        "data:image/{ext};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    ))
}

fn ignore_file(project_path: &Path) -> std::path::PathBuf {
    project_path.join(".bedrock-engineer").join(".ignore")
}

/// `read-project-ignore`: `{ content, exists }`; a missing file is `{ content: '', exists: false }`.
pub fn read_project_ignore(project_path: &Path) -> std::io::Result<Value> {
    let path = ignore_file(project_path);
    match fs::read_to_string(&path) {
        Ok(content) => {
            tracing::info!(category = CATEGORY, project_path = %project_path.display(), ignore_file_path = %path.display(), "Project ignore file read successfully");
            Ok(json!({ "content": content, "exists": true }))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            tracing::info!(category = CATEGORY, project_path = %project_path.display(), ignore_file_path = %path.display(), "Project ignore file does not exist");
            Ok(json!({ "content": "", "exists": false }))
        }
        Err(e) => {
            tracing::error!(category = CATEGORY, project_path = %project_path.display(), error = %e, "Failed to read project ignore file");
            Err(e)
        }
    }
}

/// `write-project-ignore`: `{ success }`, creating `.bedrock-engineer/` if needed.
pub fn write_project_ignore(project_path: &Path, content: &str) -> Value {
    let path = ignore_file(project_path);
    let result = fs::create_dir_all(project_path.join(".bedrock-engineer"))
        .and_then(|_| fs::write(&path, content));
    match result {
        Ok(()) => {
            tracing::info!(category = CATEGORY, project_path = %project_path.display(), ignore_file_path = %path.display(), "Project ignore file written successfully");
            json!({ "success": true })
        }
        Err(e) => {
            tracing::error!(category = CATEGORY, project_path = %project_path.display(), error = %e, "Failed to write project ignore file");
            json!({ "success": false })
        }
    }
}

fn sorted_files(dir: &Path, keep: impl Fn(&str) -> bool) -> std::io::Result<Vec<String>> {
    let mut files: Vec<String> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| keep(f))
        .collect();
    files.sort();
    Ok(files)
}

/// `window.file.readSharedAgents` (preload): every `.yaml` / `.yml` / `.json` agent under
/// `<projectPath>/.bedrock-engineer/agents`, flagged `isShared` with its `sharedFilePath`.
/// Unparseable files are logged and skipped.
pub fn read_shared_agents(project_path: Option<&Path>) -> AgentList {
    let Some(project_path) = project_path else {
        return AgentList::error("Project path not set");
    };
    let dir = project_path.join(".bedrock-engineer").join("agents");
    if !dir.exists() {
        return AgentList::default();
    }
    let files = match sorted_files(&dir, |f| {
        f.ends_with(".yaml") || f.ends_with(".yml") || f.ends_with(".json")
    }) {
        Ok(f) => f,
        Err(e) => return AgentList::error(e.to_string()),
    };
    let agents = files
        .iter()
        .filter_map(|file| {
            let path = dir.join(file);
            let loaded = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|c| parse_agent_file(&c, file));
            match loaded {
                Ok(Value::Object(mut agent)) => {
                    agent.insert("isShared".into(), json!(true));
                    agent.insert(
                        "sharedFilePath".into(),
                        json!(path.to_string_lossy().into_owned()),
                    );
                    Some(Value::Object(agent))
                }
                Ok(_) => {
                    tracing::error!(category = CATEGORY, file = %file, "Error parsing agent file: not an object");
                    None
                }
                Err(e) => {
                    tracing::error!(category = CATEGORY, file = %file, error = %e, "Error parsing agent file");
                    None
                }
            }
        })
        .collect();
    AgentList {
        agents,
        error: None,
    }
}

/// `window.file.readDirectoryAgents` (preload): the bundled `directory-agents/*.yaml|yml`, flagged
/// `directoryOnly` (and not shared/custom). A missing directory is an empty list.
pub fn read_directory_agents(dir: &Path) -> AgentList {
    if !dir.exists() {
        tracing::info!(category = CATEGORY, dir = %dir.display(), "Directory agents directory not found");
        return AgentList::default();
    }
    let files = match sorted_files(dir, |f| f.ends_with(".yaml") || f.ends_with(".yml")) {
        Ok(f) => f,
        Err(e) => return AgentList::error(e.to_string()),
    };
    let agents = files
        .iter()
        .filter_map(|file| {
            let loaded = fs::read_to_string(dir.join(file))
                .map_err(|e| e.to_string())
                .and_then(|c| common::agent_files::parse_yaml(&c));
            match loaded {
                Ok(Value::Object(mut agent)) => {
                    agent.insert("directoryOnly".into(), json!(true));
                    agent.insert("isShared".into(), json!(false));
                    agent.insert("isCustom".into(), json!(false));
                    Some(Value::Object(agent))
                }
                Ok(_) => {
                    tracing::error!(category = CATEGORY, file = %file, "Error parsing agent file: not an object");
                    None
                }
                Err(e) => {
                    tracing::error!(category = CATEGORY, file = %file, error = %e, "Error parsing agent file");
                    None
                }
            }
        })
        .collect();
    AgentList {
        agents,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn chat_documents_go_next_to_the_markdown_export() {
        let dir = TempDir::new().unwrap();
        let r = save_chat_document(dir.path(), "My: chat?", "docx", Ok(b"PK".to_vec()));
        let folder = dir.path().join("My chat");
        assert_eq!(
            r,
            ExportResult {
                success: true,
                file_path: Some(folder.join("My chat.docx").to_string_lossy().into_owned()),
                directory: Some(folder.to_string_lossy().into_owned()),
                error: None,
            }
        );
        assert_eq!(fs::read(folder.join("My chat.docx")).unwrap(), b"PK");

        let r = save_chat_document(dir.path(), "", "pdf", Err("render failed".into()));
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({ "success": false, "error": "render failed" })
        );
        // The folder is created before the document is produced, as in Electron.
        assert!(dir.path().join("chat-export").is_dir());
    }

    #[test]
    fn sanitize_matches_ts() {
        assert_eq!(
            sanitize_for_filesystem("a/b\\c:d*e?f\"g<h>i|j"),
            "abcdefghij"
        );
        assert_eq!(
            sanitize_for_filesystem("  many \t\n spaces  "),
            "many spaces"
        );
        assert_eq!(
            sanitize_for_filesystem("ends with dots..."),
            "ends with dots"
        );
        // Trailing dots go before trim, so a dot before trailing space survives.
        assert_eq!(sanitize_for_filesystem("dot. "), "dot.");
        assert_eq!(sanitize_for_filesystem("ctrl\u{0007}char"), "ctrlchar");
        assert_eq!(sanitize_for_filesystem("..."), "");
        assert_eq!(sanitize_for_filesystem(""), "");
        assert_eq!(
            sanitize_for_filesystem("日本語のタイトル"),
            "日本語のタイトル"
        );
        let long = "x".repeat(150);
        assert_eq!(sanitize_for_filesystem(&long).len(), 100);
        // 99 ASCII + an astral char (2 UTF-16 units) would exceed 100: dropped whole.
        let s = format!("{}😀", "a".repeat(99));
        assert_eq!(sanitize_for_filesystem(&s), "a".repeat(99));
    }

    #[test]
    fn base64_lenient() {
        assert_eq!(decode_base64_lenient("aGVsbG8="), b"hello");
        assert_eq!(decode_base64_lenient("aGVsbG8"), b"hello");
        assert_eq!(decode_base64_lenient("aGVs\nbG8="), b"hello");
        assert_eq!(decode_base64_lenient("-_8="), vec![0xfb, 0xff]);
        assert_eq!(decode_base64_lenient(""), b"");
    }

    #[test]
    fn markdown_export_writes_files() {
        let d = TempDir::new().unwrap();
        let r = save_chat_to_markdown(
            d.path(),
            "My: Chat?",
            "# hi",
            &[ExportImage {
                filename: "diagram-1.png".into(),
                base64: "aGVsbG8=".into(),
            }],
        );
        assert!(r.success);
        let dir = d.path().join("My Chat");
        assert_eq!(r.directory.as_deref(), Some(dir.to_str().unwrap()));
        assert_eq!(fs::read_to_string(dir.join("My Chat.md")).unwrap(), "# hi");
        assert_eq!(
            fs::read(dir.join("images/diagram-1.png")).unwrap(),
            b"hello"
        );
    }

    #[test]
    fn markdown_export_without_images_and_blank_title() {
        let d = TempDir::new().unwrap();
        let r = save_chat_to_markdown(d.path(), "///", "x", &[]);
        assert!(r.success);
        let dir = d.path().join("chat-export");
        assert!(dir.join("chat-export.md").exists());
        assert!(!dir.join("images").exists());
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("error").is_none());
        assert!(v.get("filePath").is_some());
    }

    #[test]
    fn markdown_export_failure_is_result() {
        let d = TempDir::new().unwrap();
        let file = d.path().join("not-a-dir");
        fs::write(&file, "").unwrap();
        let r = save_chat_to_markdown(&file, "t", "x", &[]);
        assert!(!r.success);
        assert!(r.error.is_some());
    }

    #[test]
    fn local_image_data_url_uses_extension() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("pic.JPEG");
        fs::write(&p, b"hello").unwrap();
        assert_eq!(
            local_image_data_url(p.to_str().unwrap()).unwrap(),
            "data:image/jpeg;base64,aGVsbG8="
        );
        let p = d.path().join("dot.");
        fs::write(&p, b"").unwrap();
        assert_eq!(
            local_image_data_url(p.to_str().unwrap()).unwrap(),
            "data:image/png;base64,"
        );
        assert!(local_image_data_url(d.path().join("missing.png").to_str().unwrap()).is_err());
    }

    #[test]
    fn project_ignore_round_trip() {
        let d = TempDir::new().unwrap();
        assert_eq!(
            read_project_ignore(d.path()).unwrap(),
            json!({ "content": "", "exists": false })
        );
        assert_eq!(
            write_project_ignore(d.path(), "node_modules\n"),
            json!({ "success": true })
        );
        assert_eq!(
            read_project_ignore(d.path()).unwrap(),
            json!({ "content": "node_modules\n", "exists": true })
        );
        let file = d.path().join("f");
        fs::write(&file, "").unwrap();
        assert_eq!(
            write_project_ignore(&file, "x"),
            json!({ "success": false })
        );
    }

    #[test]
    fn shared_agents_read_like_preload() {
        let d = TempDir::new().unwrap();
        assert_eq!(
            read_shared_agents(None),
            AgentList::error("Project path not set")
        );
        assert_eq!(read_shared_agents(Some(d.path())), AgentList::default());

        // Joined per component so the expected path uses the platform's separator, as the
        // reported `sharedFilePath` does.
        let dir = d.path().join(".bedrock-engineer").join("agents");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("b.yaml"), "id: keep-me\nname: B\nmcpTools: [x]\n").unwrap();
        fs::write(dir.join("a.json"), r#"{"name":"A"}"#).unwrap();
        fs::write(dir.join("broken.yml"), "name: [unclosed").unwrap();
        fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let r = read_shared_agents(Some(d.path()));
        assert_eq!(r.error, None);
        assert_eq!(r.agents.len(), 2);
        assert_eq!(r.agents[0]["name"], "A");
        assert_eq!(r.agents[1]["id"], "keep-me");
        assert_eq!(r.agents[1]["isShared"], true);
        assert_eq!(r.agents[1]["mcpTools"], json!(["x"]));
        assert_eq!(
            r.agents[1]["sharedFilePath"],
            dir.join("b.yaml").to_str().unwrap()
        );
    }

    #[test]
    fn directory_agents_flags() {
        let d = TempDir::new().unwrap();
        assert_eq!(
            read_directory_agents(&d.path().join("missing")),
            AgentList::default()
        );
        fs::write(d.path().join("a.yaml"), "name: A\nisShared: true\n").unwrap();
        fs::write(d.path().join("b.json"), r#"{"name":"B"}"#).unwrap();
        fs::write(d.path().join("c.yml"), "just a string").unwrap();
        let r = read_directory_agents(d.path());
        assert_eq!(r.agents.len(), 1);
        assert_eq!(
            r.agents[0],
            json!({ "name": "A", "isShared": false, "directoryOnly": true, "isCustom": false })
        );
    }

    #[test]
    fn bundled_directory_agents_load() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../src/renderer/src/assets/directory-agents");
        let r = read_directory_agents(&dir);
        assert_eq!(r.error, None);
        let yaml_count = fs::read_dir(&dir)
            .unwrap()
            .filter(|e| {
                let n = e.as_ref().unwrap().file_name();
                let n = n.to_string_lossy();
                n.ends_with(".yaml") || n.ends_with(".yml")
            })
            .count();
        assert_eq!(r.agents.len(), yaml_count);
        assert!(r.agents.iter().all(|a| a["directoryOnly"] == true));
    }
}
