//! Moving a chat's files into its folder (`chats/<chat-name>/`), run when an existing chat is
//! opened, and keeping the chat's list of files (`touchedFiles`) pointing at them.
//!
//! Chats from before the per-chat folder wrote wherever the model chose: folders it made up in
//! the project root, Code Interpreter's `.bedrock-engineer/workspaces/workspace-*` folders, and
//! the attachments folder of the earlier layout. Their paths are in the chat's `touchedFiles`.
//!
//! A file is moved only when it is plainly the chat's own:
//!
//! - it is a regular file (not a symlink) that still exists, outside `chats/`;
//! - no other chat lists it, and git does not track it (the caller passes both sets);
//! - it is in a Code Interpreter workspace, or it was created after the chat started (files
//!   whose creation time the OS can't tell are left alone), and no part of its path is hidden
//!   (`.git`, `.bedrock-engineer/agents`, `.vscode`, ...).
//!
//! It keeps its path relative to the project (`report/x.md` → `chats/<chat>/report/x.md`);
//! Code Interpreter files drop the workspace folder (`…/workspace-*/plot.png` →
//! `chats/<chat>/plot.png`). Nothing is overwritten: a file whose destination exists stays.
//! Folders left empty by the moves are removed, up to the project folder.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::manager::{ensure_chat_dir, resolve_chat_dir, AttachmentPaths};
use crate::types::{ATTACHMENTS_ROOT_DIRNAME, CHATS_ROOT_DIRNAME};
use crate::MARKER_FILE;

/// What the caller knows about the chat and the project.
#[derive(Debug, Clone)]
pub struct TidyInput<'a> {
    /// The chat's `touchedFiles` (project-relative, `/`-separated).
    pub touched: &'a [String],
    /// Files other chats list in their `touchedFiles`; never moved.
    pub others: &'a HashSet<String>,
    /// Files git tracks in the project (project-relative); never moved.
    pub tracked: &'a HashSet<String>,
    /// When the chat started (epoch ms). Files created before it are not moved.
    pub since_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovedFile {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TidyResult {
    /// The chat's folder, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    /// Files moved, project-relative.
    pub moved: Vec<MovedFile>,
    /// The chat's `touchedFiles` after the moves and path fixes.
    pub touched: Vec<String>,
    /// Whether `touched` differs from the input (the caller saves it then).
    pub changed: bool,
}

const WORKSPACES_PREFIX: &str = ".bedrock-engineer/workspaces/";

/// `/`-joined normal components of a project-relative path; `None` for absolute paths or `..`.
fn clean_relative(path: &str) -> Option<Vec<String>> {
    let path = path.replace('\\', "/");
    let mut parts = Vec::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then_some(parts)
}

fn join_rel(base: &Path, parts: &[String]) -> PathBuf {
    parts
        .iter()
        .fold(base.to_path_buf(), |p, part| p.join(part))
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

fn created_ms(path: &Path) -> Option<f64> {
    let created = fs::metadata(path).ok()?.created().ok()?;
    Some(created.duration_since(UNIX_EPOCH).ok()?.as_secs_f64() * 1000.0)
}

/// `path` relative to `project`, `/`-separated.
fn rel_string(project: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(project).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Remove `dir` and its parents while they are empty, stopping at `stop` (not removed).
fn prune_empty_dirs(mut dir: &Path, stop: &Path) {
    while dir != stop && dir.starts_with(stop) {
        if fs::remove_dir(dir).is_err() {
            return;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return,
        }
    }
}

/// Where a touched path of the chat's own folder under an older name now is: `chats/<old>/x`
/// → `chats/<current>/x` when the old one is gone and the new one exists.
fn follow_renamed_chat_dir(project: &Path, chat_dir: &Path, parts: &[String]) -> Option<String> {
    if parts.len() < 3 || parts[0] != CHATS_ROOT_DIRNAME {
        return None;
    }
    if join_rel(project, parts).exists() {
        return None;
    }
    let candidate = join_rel(chat_dir, &parts[2..]);
    candidate
        .exists()
        .then(|| rel_string(project, &candidate))
        .flatten()
}

/// Where a touched path in the earlier layout's attachments folder now is:
/// `attachments/<old>/x` → `chats/<chat>/attachments/x`.
fn follow_moved_attachment(project: &Path, chat_dir: &Path, parts: &[String]) -> Option<String> {
    if parts.len() < 3 || parts[0] != ATTACHMENTS_ROOT_DIRNAME {
        return None;
    }
    if join_rel(project, parts).exists() {
        return None;
    }
    let candidate = join_rel(&chat_dir.join(ATTACHMENTS_ROOT_DIRNAME), &parts[2..]);
    candidate
        .exists()
        .then(|| rel_string(project, &candidate))
        .flatten()
}

/// Rewrite the chat's touched paths that point at an older name of its folder (after a rename).
/// Moves nothing. Returns the new list when anything changed.
pub fn follow_chat_dir_rename(
    paths: &AttachmentPaths,
    session_id: &str,
    touched: &[String],
) -> Option<Vec<String>> {
    let project = paths.project_path.as_deref()?;
    let chat_dir = resolve_chat_dir(paths, session_id).ok()?;
    if !chat_dir.exists() {
        return None;
    }
    let mut changed = false;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in touched {
        let next = clean_relative(entry)
            .and_then(|parts| follow_renamed_chat_dir(project, &chat_dir, &parts))
            .inspect(|_| changed = true)
            .unwrap_or_else(|| entry.clone());
        if seen.insert(next.clone()) {
            out.push(next);
        }
    }
    changed.then_some(out)
}

/// Move the chat's files into its folder (see the module docs).
pub fn tidy_chat_files(
    paths: &AttachmentPaths,
    session_id: &str,
    input: &TidyInput<'_>,
) -> Result<TidyResult> {
    let project = paths
        .project_path
        .clone()
        .ok_or(crate::error::Error::NoProjectPath)?;
    // Resolving moves an earlier-layout attachments folder in and follows the title.
    let mut chat_dir = resolve_chat_dir(paths, session_id)?;

    let mut moved = Vec::new();
    let mut touched_out: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let mut changed = false;
    let mut push = |entry: String, out: &mut Vec<String>| {
        if seen.insert(entry.clone()) {
            out.push(entry);
        }
    };

    for entry in input.touched {
        let Some(parts) = clean_relative(entry) else {
            push(entry.clone(), &mut touched_out);
            continue;
        };
        // The ownership marker is the app's, not a file of the chat.
        if parts.last().map(String::as_str) == Some(MARKER_FILE) {
            changed = true;
            continue;
        }
        if let Some(next) = follow_renamed_chat_dir(&project, &chat_dir, &parts)
            .or_else(|| follow_moved_attachment(&project, &chat_dir, &parts))
        {
            changed = true;
            push(next, &mut touched_out);
            continue;
        }

        let rel = parts.join("/");
        let source = join_rel(&project, &parts);
        let in_workspace = rel.starts_with(WORKSPACES_PREFIX)
            && parts.len() > 3
            && parts[2].starts_with("workspace-");
        let movable = parts[0] != CHATS_ROOT_DIRNAME
            && parts[0] != ATTACHMENTS_ROOT_DIRNAME
            && is_regular_file(&source)
            && !input.others.contains(&rel)
            && !input.tracked.contains(&rel)
            && if in_workspace {
                parts[3..].iter().all(|p| !p.starts_with('.'))
            } else {
                parts.iter().all(|p| !p.starts_with('.'))
                    && match (input.since_ms, created_ms(&source)) {
                        (Some(since), Some(created)) => created >= since,
                        _ => false,
                    }
            };
        if !movable {
            push(rel, &mut touched_out);
            continue;
        }

        if !chat_dir.exists() {
            chat_dir = ensure_chat_dir(paths, session_id)?;
        }
        let dest_parts = if in_workspace {
            &parts[3..]
        } else {
            &parts[..]
        };
        let dest = join_rel(&chat_dir, dest_parts);
        if dest.exists() {
            push(rel, &mut touched_out);
            continue;
        }
        let result = dest
            .parent()
            .map(fs::create_dir_all)
            .transpose()
            .and_then(|_| fs::rename(&source, &dest));
        match result {
            Ok(()) => {
                let to = rel_string(&project, &dest).unwrap_or_else(|| rel.clone());
                tracing::info!(from = %rel, to = %to, "Moved a chat's file into its folder");
                if let Some(parent) = source.parent() {
                    prune_empty_dirs(parent, &project);
                }
                moved.push(MovedFile {
                    from: rel,
                    to: to.clone(),
                });
                changed = true;
                push(to, &mut touched_out);
            }
            Err(e) => {
                tracing::warn!(file = %rel, error = %e, "Could not move a chat's file into its folder");
                push(rel, &mut touched_out);
            }
        }
    }
    Ok(TidyResult {
        directory: chat_dir
            .exists()
            .then(|| chat_dir.to_string_lossy().into_owned()),
        moved,
        touched: touched_out,
        changed: changed || input.touched.len() != seen.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::tests::{Fixture, SESSION_ID};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn moves_the_chats_own_files_and_leaves_everything_else() {
        let fx = Fixture::new();
        fx.set_chat_title(SESSION_ID, "OCR benchmark");
        let project = fx.project.path();
        write(project, "bench/README.md", "r");
        write(project, "bench/run.py", "p");
        write(
            project,
            ".bedrock-engineer/workspaces/workspace-20261109-s_1/plot.png",
            "i",
        );
        write(
            project,
            ".bedrock-engineer/workspaces/workspace-20261109-s_1/other.txt",
            "o",
        );
        write(project, "src/main.rs", "tracked");
        write(project, "shared.md", "another chat's");
        write(project, ".vscode/settings.json", "{}");

        let touched = s(&[
            "bench/README.md",
            "bench/run.py",
            ".bedrock-engineer/workspaces/workspace-20261109-s_1/plot.png",
            "src/main.rs",
            "shared.md",
            ".vscode/settings.json",
            "gone.txt",
        ]);
        let others: HashSet<String> = s(&["shared.md"]).into_iter().collect();
        let tracked: HashSet<String> = s(&["src/main.rs"]).into_iter().collect();
        let result = tidy_chat_files(
            &fx.paths,
            SESSION_ID,
            &TidyInput {
                touched: &touched,
                others: &others,
                tracked: &tracked,
                since_ms: Some(0.0),
            },
        )
        .unwrap();

        let chat = project.join("chats/ocr-benchmark");
        assert!(chat.join("bench/README.md").exists());
        assert!(chat.join("bench/run.py").exists());
        assert!(chat.join("plot.png").exists());
        assert!(!project.join("bench").exists(), "emptied folder is removed");
        // The workspace keeps the file this chat didn't list.
        assert!(project
            .join(".bedrock-engineer/workspaces/workspace-20261109-s_1/other.txt")
            .exists());
        for kept in ["src/main.rs", "shared.md", ".vscode/settings.json"] {
            assert!(project.join(kept).exists(), "{kept}");
        }
        assert_eq!(result.moved.len(), 3);
        assert!(result.changed);
        assert_eq!(
            result.touched,
            s(&[
                "chats/ocr-benchmark/bench/README.md",
                "chats/ocr-benchmark/bench/run.py",
                "chats/ocr-benchmark/plot.png",
                "src/main.rs",
                "shared.md",
                ".vscode/settings.json",
                "gone.txt",
            ])
        );
    }

    #[test]
    fn files_older_than_the_chat_stay() {
        let fx = Fixture::new();
        write(fx.project.path(), "notes/old.md", "o");
        let touched = s(&["notes/old.md"]);
        let empty = HashSet::new();
        let result = tidy_chat_files(
            &fx.paths,
            SESSION_ID,
            &TidyInput {
                touched: &touched,
                others: &empty,
                tracked: &empty,
                since_ms: Some(f64::MAX),
            },
        )
        .unwrap();
        assert!(result.moved.is_empty());
        assert!(!result.changed);
        assert!(fx.project.path().join("notes/old.md").exists());
        assert!(
            !fx.chats_root().exists(),
            "no folder made when nothing moves"
        );
    }

    #[test]
    fn never_overwrites_and_rewrites_attachment_and_renamed_paths() {
        let fx = Fixture::new();
        fx.set_chat_title(SESSION_ID, "First");
        fx.add(SESSION_ID, &[("pic.png", b"\x89PNG\r\n\x1a\n")]);
        let project = fx.project.path();
        write(project, "chats/first/report.md", "in folder");
        write(project, "report.md", "loose");
        fx.set_chat_title(SESSION_ID, "Second");

        let touched = s(&[
            "chats/first/report.md",
            "report.md",
            "attachments/first-abcdef/.chat-session.json",
        ]);
        let empty = HashSet::new();
        let result = tidy_chat_files(
            &fx.paths,
            SESSION_ID,
            &TidyInput {
                touched: &touched,
                others: &empty,
                tracked: &empty,
                since_ms: Some(0.0),
            },
        )
        .unwrap();
        assert_eq!(result.touched, s(&["chats/second/report.md", "report.md"]));
        assert!(
            result.moved.is_empty(),
            "destination exists: nothing overwritten"
        );
        assert_eq!(
            fs::read_to_string(project.join("report.md")).unwrap(),
            "loose"
        );
    }

    #[test]
    fn follows_a_rename_of_the_chat_folder() {
        let fx = Fixture::new();
        fx.set_chat_title(SESSION_ID, "Before");
        let dir = ensure_chat_dir(&fx.paths, SESSION_ID).unwrap();
        fs::write(dir.join("a.md"), "a").unwrap();
        fx.set_chat_title(SESSION_ID, "After");
        crate::manager::rename_chat_dir(&fx.paths, SESSION_ID);

        assert_eq!(
            follow_chat_dir_rename(&fx.paths, SESSION_ID, &s(&["chats/before/a.md", "x.md"])),
            Some(s(&["chats/after/a.md", "x.md"]))
        );
        assert_eq!(
            follow_chat_dir_rename(&fx.paths, SESSION_ID, &s(&["chats/after/a.md"])),
            None
        );
    }
}
