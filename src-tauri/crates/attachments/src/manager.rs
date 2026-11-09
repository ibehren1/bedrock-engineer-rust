//! Per-chat folders and the attachments inside them (port of `attachmentsManager.ts` and
//! `chatSessionTitle.ts`, with a new layout).
//!
//! ## Layout
//!
//! Every chat has one folder in the project directory, `chats/<chat-name>/`, named after the
//! chat title ([`to_chat_folder_name`]) and renamed when the title changes. The agent saves the
//! files it makes for the chat there, and files the user attaches go in its `attachments/`
//! subfolder. When two chats' titles give the same name, the later one is `<name>-2`, `-3`, ...
//!
//! ## Folder ownership
//!
//! The name alone proves nothing (titles repeat, and the user can make folders too). A folder
//! belongs to a chat only when it holds a [`MARKER_FILE`] (`.chat-session.json`,
//! `{"sessionId": ...}`) naming the chat. Only such folders are renamed or deleted.
//!
//! ## Earlier layout
//!
//! Builds before this one kept only attachments, in `attachments/<title-slug>-<shortId>/`
//! (marker inside, or no marker for Electron-era folders whose name matches the chat's title
//! exactly, see [`find_legacy_folder`]). The first time a chat's folder is looked up, its old
//! folder is moved to `chats/<chat-name>/attachments/`; the `attachments/` root goes away once
//! nothing of the app's is left in it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::error::{Error, Result};
use crate::file_naming::{
    resolve_inside, resolve_lexically, sanitize_attachment_name, unique_name_in,
};
use crate::folder_naming::{to_chat_folder_name, to_folder_name, to_short_id};
use crate::image_validation::{is_image_extension, validate_image_bytes};
use crate::types::*;
use common::json_file::{is_safe_file_id, parse_lenient, write_atomic};

/// The two store values the TS code read on every call (`projectPath`, `userDataPath`).
///
/// Build one from the store per operation so a changed project directory takes effect
/// immediately, as it did in Electron.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttachmentPaths {
    /// Project directory; chat folders live at `<project>/chats/`.
    pub project_path: Option<PathBuf>,
    /// App user-data directory; chat titles are read from `<userData>/chat-sessions/<id>.json`.
    pub user_data_path: Option<PathBuf>,
}

impl AttachmentPaths {
    /// Empty strings count as unset, matching the TS truthiness checks.
    pub fn new(project_path: Option<&str>, user_data_path: Option<&str>) -> Self {
        let non_empty = |v: Option<&str>| v.filter(|s| !s.is_empty()).map(PathBuf::from);
        Self {
            project_path: non_empty(project_path),
            user_data_path: non_empty(user_data_path),
        }
    }

    fn project_path(&self) -> Result<&Path> {
        self.project_path.as_deref().ok_or(Error::NoProjectPath)
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// How much of a session file is read looking for its `title` before parsing the whole file.
const TITLE_PREFIX_BYTES: u64 = 64 * 1024;

/// Read a chat's current title from the session file the chat history writes.
///
/// Session files can be tens of megabytes and the title sits near the top, so the start of the
/// file is scanned first ([`top_level_title`]); the whole file is parsed only when that fails.
pub fn read_chat_title(paths: &AttachmentPaths, session_id: &str) -> Option<String> {
    use std::io::Read;
    if !is_safe_file_id(session_id) {
        return None;
    }
    let user_data = paths.user_data_path.as_deref()?;
    let file = user_data
        .join("chat-sessions")
        .join(format!("{session_id}.json"));
    let mut prefix = Vec::new();
    fs::File::open(&file)
        .ok()?
        .take(TITLE_PREFIX_BYTES)
        .read_to_end(&mut prefix)
        .ok()?;
    if let Some(title) = top_level_title(&prefix) {
        return Some(title);
    }
    let raw = fs::read_to_string(file).ok()?;
    let parsed: serde_json::Value = parse_lenient(&raw).ok()?;
    parsed.get("title")?.as_str().map(str::to_string)
}

/// The value of the top-level `"title"` key in the (possibly cut-off) JSON object `bytes`,
/// when it is a string that ends within `bytes`.
fn top_level_title(bytes: &[u8]) -> Option<String> {
    let mut depth = 0usize;
    let mut i = 0;
    // Whether the next string at depth 1 is an object key.
    let mut expect_key = false;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                expect_key = depth == 1;
            }
            b'[' => depth += 1,
            b'}' | b']' => depth = depth.checked_sub(1)?,
            b',' if depth == 1 => expect_key = true,
            b'"' => {
                let end = string_end(bytes, i)?;
                if depth == 1 && expect_key {
                    expect_key = false;
                    if &bytes[i..=end] == b"\"title\"" {
                        let mut j = end + 1;
                        while j < bytes.len()
                            && (bytes[j].is_ascii_whitespace() || bytes[j] == b':')
                        {
                            j += 1;
                        }
                        if bytes.get(j) != Some(&b'"') {
                            return None;
                        }
                        let value_end = string_end(bytes, j)?;
                        let literal = std::str::from_utf8(&bytes[j..=value_end]).ok()?;
                        return parse_lenient::<String>(literal).ok();
                    }
                }
                i = end;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Index of the closing quote of the JSON string opening at `start`.
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Whether `session_id` names a chat the chat history has a session file for. Runs that are
/// not chats (background agents keep their own sessions elsewhere) have no chat folder.
pub fn is_chat_session(paths: &AttachmentPaths, session_id: &str) -> bool {
    if !is_safe_file_id(session_id) {
        return false;
    }
    paths.user_data_path.as_deref().is_some_and(|user_data| {
        user_data
            .join("chat-sessions")
            .join(format!("{session_id}.json"))
            .exists()
    })
}

/// Ownership marker inside each chat folder.
pub const MARKER_FILE: &str = ".chat-session.json";

/// The `sessionId` recorded in a folder's marker, if it has a readable one.
fn read_marker(directory: &Path) -> Option<String> {
    let raw = fs::read_to_string(directory.join(MARKER_FILE)).ok()?;
    let parsed: serde_json::Value = parse_lenient(&raw).ok()?;
    parsed.get("sessionId")?.as_str().map(str::to_string)
}

fn write_marker(directory: &Path, session_id: &str) -> std::io::Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({ "sessionId": session_id }))
        .map_err(std::io::Error::other)?;
    write_atomic(&directory.join(MARKER_FILE), &body)
}

fn ensure_marker(directory: &Path, session_id: &str) {
    if read_marker(directory).as_deref() != Some(session_id) {
        if let Err(e) = write_marker(directory, session_id) {
            tracing::warn!(directory = %directory.display(), error = %e, "Failed to write chat folder marker");
        }
    }
}

/// True for names the earlier layout's [`to_folder_name`] can produce: `<slug>-<6 hex>`, where
/// the slug is 1-40 of `[a-z0-9]` in dash-separated runs (no leading, trailing or doubled dash).
pub fn is_app_folder_name(name: &str) -> bool {
    let Some((slug, short)) = name.rsplit_once('-') else {
        return false;
    };
    short.len() == 6
        && short
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        && !slug.is_empty()
        && slug.len() <= 40
        && slug.split('-').all(|run| {
            !run.is_empty()
                && run
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

/// Names of the real directories (not symlinks) directly inside `root`, in name order.
fn dir_names(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------------------------
// Chat folders: `<project>/chats/<chat-name>/`
// ---------------------------------------------------------------------------------------------

/// `<projectPath>/chats`. Errors when no project directory is configured.
pub fn get_chats_root(paths: &AttachmentPaths) -> Result<PathBuf> {
    Ok(paths.project_path()?.join(CHATS_ROOT_DIRNAME))
}

/// What the `chats/` root's `.gitignore` keeps out of the user's repository: the attached
/// files and the ownership markers. What the agent makes in a chat's folder is left visible.
const CHATS_GITIGNORE: &str = "# Chat folders made by Bedrock Engineer.\n# Files attached to a chat stay out of version control; what the agent makes does not.\n*/attachments/\n.chat-session.json\n";

/// Create the root and its `.gitignore` (written once, so the user can edit it).
fn ensure_chats_root(paths: &AttachmentPaths) -> Result<PathBuf> {
    let root = get_chats_root(paths)?;
    fs::create_dir_all(&root)?;
    let gitignore = root.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, CHATS_GITIGNORE)?;
    }
    Ok(root)
}

/// The folder under `root` whose marker names `session_id`.
fn find_chat_folder(root: &Path, session_id: &str) -> Option<PathBuf> {
    dir_names(root)
        .into_iter()
        .map(|name| root.join(name))
        .find(|dir| read_marker(dir).as_deref() == Some(session_id))
}

/// The name for a chat's folder: `desired`, or — when a folder of that name exists but belongs
/// to someone else — `<desired>-<n>` for the first free (or already ours) `n`.
fn claim_chat_folder_name(root: &Path, session_id: &str, desired: &str) -> String {
    let free_or_ours = |name: &str| {
        let dir = root.join(name);
        !dir.exists() || (is_real_dir(&dir) && read_marker(&dir).as_deref() == Some(session_id))
    };
    if free_or_ours(desired) {
        return desired.to_string();
    }
    (2..)
        .map(|n| format!("{desired}-{n}"))
        .find(|name| free_or_ours(name))
        .expect("an unused name")
}

/// The folder a chat's files live in, renamed to follow the current chat title when the two
/// disagree, and with the chat's attachments moved in from the earlier layout. Creates no folder
/// for a chat that has none yet: the path is where it will be.
pub fn resolve_chat_dir(paths: &AttachmentPaths, session_id: &str) -> Result<PathBuf> {
    let root = get_chats_root(paths)?;
    let title = read_chat_title(paths, session_id);
    let desired = to_chat_folder_name(session_id, title.as_deref());

    let existing = find_chat_folder(&root, session_id)
        .or_else(|| migrate_legacy_folder(paths, &root, session_id, &desired));
    let Some(existing) = existing else {
        return Ok(root.join(claim_chat_folder_name(&root, session_id, &desired)));
    };

    let target = root.join(claim_chat_folder_name(&root, session_id, &desired));
    if resolve_lexically(&existing) == resolve_lexically(&target) {
        return Ok(existing);
    }
    match fs::rename(&existing, &target) {
        Ok(()) => Ok(target),
        Err(e) => {
            tracing::warn!(from = %existing.display(), to = %target.display(), error = %e, "Failed to rename chat folder");
            Ok(existing)
        }
    }
}

/// [`resolve_chat_dir`] plus creating the folder and its marker.
pub fn ensure_chat_dir(paths: &AttachmentPaths, session_id: &str) -> Result<PathBuf> {
    ensure_chats_root(paths)?;
    let directory = resolve_chat_dir(paths, session_id)?;
    fs::create_dir_all(&directory)?;
    ensure_marker(&directory, session_id);
    Ok(directory)
}

/// Rename a chat's folder to follow its title (the title-change path). Not renamed when the
/// chat has no folder yet.
pub fn rename_chat_dir(paths: &AttachmentPaths, session_id: &str) -> RenameResult {
    let not_renamed = RenameResult {
        renamed: false,
        directory: None,
    };
    let Ok(root) = get_chats_root(paths) else {
        return not_renamed;
    };
    let before = find_chat_folder(&root, session_id);
    let Ok(after) = resolve_chat_dir(paths, session_id) else {
        return not_renamed;
    };
    if !after.exists() {
        return not_renamed;
    }
    RenameResult {
        renamed: before.is_some_and(|b| resolve_lexically(&b) != resolve_lexically(&after)),
        directory: Some(path_string(&after)),
    }
}

/// Delete a chat's folder (with everything in it), used when the chat itself is deleted. Only
/// a folder proven to be the chat's is deleted; an unmoved folder of the earlier layout goes too.
pub fn remove_chat_dir(paths: &AttachmentPaths, session_id: &str) -> Result<RemovedFolder> {
    let Ok(root) = get_chats_root(paths) else {
        return Ok(RemovedFolder { removed: false });
    };
    let mut removed = false;
    if let Some(directory) = find_chat_folder(&root, session_id) {
        fs::remove_dir_all(&directory)?;
        removed = true;
    }
    if let Some(legacy) = find_legacy_folder(paths, session_id) {
        fs::remove_dir_all(&legacy)?;
        removed = true;
        remove_legacy_root_if_empty(paths);
    }
    Ok(RemovedFolder { removed })
}

/// Delete every chat folder (for "delete all chats"), working off the folders on disk so chats
/// the sidebar hides are reached too. Folders without a marker naming a chat are kept, and so
/// are files at the root (the `.gitignore`) and symlinks. Folders of the earlier layout go too.
pub fn remove_every_chat_dir(paths: &AttachmentPaths) -> Result<RemovedFolders> {
    let mut removed = 0;
    if let Ok(root) = get_chats_root(paths) {
        for name in dir_names(&root) {
            let dir = root.join(&name);
            if read_marker(&dir).is_none() {
                tracing::info!(folder = %name, "Keeping a folder the app did not create");
                continue;
            }
            fs::remove_dir_all(&dir)?;
            removed += 1;
        }
    }
    removed += remove_every_legacy_folder(paths)?;
    Ok(RemovedFolders { removed })
}

// ---------------------------------------------------------------------------------------------
// The earlier layout: `<project>/attachments/<title-slug>-<shortId>/`
// ---------------------------------------------------------------------------------------------

fn legacy_root(paths: &AttachmentPaths) -> Result<PathBuf> {
    Ok(paths.project_path()?.join(ATTACHMENTS_ROOT_DIRNAME))
}

/// The earlier layout's folder proven to belong to `session_id`: the one whose marker names the
/// chat, or a marker-less one (Electron era) named exactly as the chat's current title gives.
fn find_legacy_folder(paths: &AttachmentPaths, session_id: &str) -> Option<PathBuf> {
    let root = legacy_root(paths).ok()?;
    let title = read_chat_title(paths, session_id);
    let expected = to_folder_name(session_id, title.as_deref());
    let suffix = format!("-{}", to_short_id(session_id));
    let mut legacy_match = None;
    for name in dir_names(&root)
        .into_iter()
        .filter(|n| n.ends_with(&suffix))
    {
        let dir = root.join(&name);
        match read_marker(&dir) {
            Some(owner) if owner == session_id => return Some(dir),
            Some(_) => {}
            None if name == expected => legacy_match = Some(dir),
            None => {}
        }
    }
    legacy_match
}

/// Move the chat's earlier-layout folder to `<root>/<name>/attachments/`, returning the new chat
/// folder. `None` when there is nothing to move or the move failed (the old folder stays put).
fn migrate_legacy_folder(
    paths: &AttachmentPaths,
    root: &Path,
    session_id: &str,
    desired: &str,
) -> Option<PathBuf> {
    let legacy = find_legacy_folder(paths, session_id)?;
    let moved = (|| -> Result<PathBuf> {
        ensure_chats_root(paths)?;
        let chat_dir = root.join(claim_chat_folder_name(root, session_id, desired));
        fs::create_dir_all(&chat_dir)?;
        ensure_marker(&chat_dir, session_id);
        let target = chat_dir.join(ATTACHMENTS_ROOT_DIRNAME);
        if target.exists() {
            return Err(Error::Io(std::io::Error::other(format!(
                "{} already exists",
                target.display()
            ))));
        }
        fs::rename(&legacy, &target)?;
        let _ = fs::remove_file(target.join(MARKER_FILE));
        Ok(chat_dir)
    })();
    match moved {
        Ok(chat_dir) => {
            tracing::info!(from = %legacy.display(), to = %chat_dir.display(), "Moved a chat's attachments into its chat folder");
            remove_legacy_root_if_empty(paths);
            Some(chat_dir)
        }
        Err(e) => {
            tracing::warn!(folder = %legacy.display(), error = %e, "Failed to move a chat's attachments into its chat folder");
            find_chat_folder(root, session_id)
        }
    }
}

/// Remove the earlier layout's `attachments/` root once only its `.gitignore` is left.
fn remove_legacy_root_if_empty(paths: &AttachmentPaths) {
    let Ok(root) = legacy_root(paths) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    let names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    if names.iter().all(|n| n == ".gitignore" || n == ".DS_Store") {
        let _ = fs::remove_dir_all(&root);
    }
}

/// "Delete all" for the earlier layout: folders with a marker or with the app's exact name
/// shape ([`is_app_folder_name`]); anything else under `attachments/` is kept.
fn remove_every_legacy_folder(paths: &AttachmentPaths) -> Result<usize> {
    let Ok(root) = legacy_root(paths) else {
        return Ok(0);
    };
    let mut removed = 0;
    for name in dir_names(&root) {
        let dir = root.join(&name);
        if read_marker(&dir).is_none() && !is_app_folder_name(&name) {
            tracing::info!(folder = %name, "Keeping a folder the app did not create");
            continue;
        }
        fs::remove_dir_all(&dir)?;
        removed += 1;
    }
    remove_legacy_root_if_empty(paths);
    Ok(removed)
}

// ---------------------------------------------------------------------------------------------
// Attachments: `<chat folder>/attachments/`
// ---------------------------------------------------------------------------------------------

/// The folder a chat's attachments live in (`<chat folder>/attachments`). Creates nothing.
pub fn resolve_attachments_dir(paths: &AttachmentPaths, session_id: &str) -> Result<PathBuf> {
    Ok(resolve_chat_dir(paths, session_id)?.join(ATTACHMENTS_ROOT_DIRNAME))
}

/// [`resolve_attachments_dir`] plus creating the chat folder and its attachments folder.
pub fn ensure_attachments_dir(paths: &AttachmentPaths, session_id: &str) -> Result<PathBuf> {
    let directory = ensure_chat_dir(paths, session_id)?.join(ATTACHMENTS_ROOT_DIRNAME);
    fs::create_dir_all(&directory)?;
    Ok(directory)
}

fn classify(file_name: &str) -> AttachmentKind {
    let ext = crate::file_naming::node_extname(file_name).to_lowercase();
    if is_image_extension(file_name) {
        AttachmentKind::Image
    } else if ext == ".pdf" {
        AttachmentKind::Pdf
    } else if ext == ".docx" {
        AttachmentKind::Docx
    } else if EXTRACTABLE_TEXT_EXTENSIONS.contains(&ext.as_str()) {
        AttachmentKind::Text
    } else {
        AttachmentKind::Other
    }
}

fn mtime_ms(metadata: &fs::Metadata) -> f64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn to_attachment(directory: &Path, name: &str, metadata: &fs::Metadata) -> ChatAttachment {
    let kind = classify(name);
    ChatAttachment {
        name: name.to_string(),
        path: path_string(&directory.join(name)),
        size: metadata.len(),
        mtime: mtime_ms(metadata),
        kind,
        extractable: matches!(
            kind,
            AttachmentKind::Text | AttachmentKind::Pdf | AttachmentKind::Docx
        ),
    }
}

/// Non-hidden entries of `directory`, in name order like Node's `readdirSync`.
fn sorted_entries(directory: &Path) -> std::io::Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(directory)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

/// Files currently attached to a chat, oldest first. A chat with no folder has none; with no
/// project directory configured the listing is `{ directory: "", files: [] }`.
pub fn list_attachments(paths: &AttachmentPaths, session_id: &str) -> AttachmentListing {
    let Ok(directory) = resolve_attachments_dir(paths, session_id) else {
        return AttachmentListing {
            directory: String::new(),
            files: Vec::new(),
        };
    };

    let mut files: Vec<ChatAttachment> = sorted_entries(&directory)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let metadata = fs::metadata(entry.path()).ok()?;
            Some(to_attachment(&directory, &name, &metadata))
        })
        .collect();
    files.sort_by(|a, b| a.mtime.total_cmp(&b.mtime));

    AttachmentListing {
        directory: path_string(&directory),
        files,
    }
}

fn save_one(
    directory: &Path,
    file: &NewAttachment,
    image_count: &mut usize,
) -> Result<ChatAttachment> {
    if is_image_extension(&file.name) {
        if let Some(problem) = validate_image_bytes(&file.name, &file.bytes, *image_count) {
            return Err(Error::Image(problem));
        }
        *image_count += 1;
    }

    let safe_name = unique_name_in(directory, &sanitize_attachment_name(&file.name));
    let target = resolve_inside(directory, &safe_name)?;
    fs::write(&target, &file.bytes)?;
    Ok(to_attachment(
        directory,
        &safe_name,
        &fs::metadata(&target)?,
    ))
}

/// Write dropped, pasted or picked files into the chat's folder. Per-file failures are reported
/// in `errors` without failing the batch; a missing project directory fails the whole call.
pub fn add_attachments(
    paths: &AttachmentPaths,
    session_id: &str,
    files: &[NewAttachment],
) -> Result<AttachmentAddResult> {
    let directory = ensure_attachments_dir(paths, session_id)?;
    let mut added = Vec::new();
    let mut errors = Vec::new();

    // The image cap counts what is already in the folder, not just this batch.
    let mut image_count = list_attachments(paths, session_id)
        .files
        .iter()
        .filter(|f| f.kind == AttachmentKind::Image)
        .count();

    for file in files {
        match save_one(&directory, file, &mut image_count) {
            Ok(attachment) => added.push(attachment),
            Err(error) => errors.push(AttachmentError {
                name: file.name.clone(),
                error: error.to_string(),
            }),
        }
    }

    let listing = list_attachments(paths, session_id);
    Ok(AttachmentAddResult {
        directory: listing.directory,
        files: listing.files,
        added,
        errors,
    })
}

/// Copy files the user picked with the native dialog, leaving the originals alone.
pub fn add_attachments_from_paths<P: AsRef<Path>>(
    paths: &AttachmentPaths,
    session_id: &str,
    source_paths: &[P],
) -> Result<AttachmentAddResult> {
    let mut files = Vec::new();
    let mut read_errors = Vec::new();

    for source in source_paths {
        let source = source.as_ref();
        let name = crate::file_naming::node_basename(&source.to_string_lossy()).to_string();
        match fs::read(source) {
            Ok(bytes) => files.push(NewAttachment { name, bytes }),
            Err(error) => read_errors.push(AttachmentError {
                name,
                error: error.to_string(),
            }),
        }
    }

    let mut result = add_attachments(paths, session_id, &files)?;
    read_errors.append(&mut result.errors);
    result.errors = read_errors;
    Ok(result)
}

/// Hard-delete one attached file. Names that would reach outside the folder are an error.
pub fn remove_attachment(
    paths: &AttachmentPaths,
    session_id: &str,
    name: &str,
) -> Result<AttachmentRemoveResult> {
    let directory = resolve_attachments_dir(paths, session_id)?;
    let target = resolve_inside(&directory, name)?;

    let mut removed = false;
    if target.exists() {
        fs::remove_file(&target)?;
        removed = true;
    }

    let listing = list_attachments(paths, session_id);
    Ok(AttachmentRemoveResult {
        directory: listing.directory,
        files: listing.files,
        removed,
    })
}

/// Of the given chats, which ones have at least one (non-hidden) attached file, in the order
/// asked for. Errors (no project directory, vanished folder) just mean "no files".
///
/// The sidebar asks this for every chat on a timer, so it is one read-only pass: the chat
/// folders are listed once and matched by their markers; nothing is renamed or moved, and no
/// session file is read. Earlier-layout folders count only when they carry a marker.
pub fn list_session_ids_with_attachments(
    paths: &AttachmentPaths,
    session_ids: &[String],
) -> Vec<String> {
    let has_visible_entry = |dir: &Path| match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| !e.file_name().to_string_lossy().starts_with('.')),
        Err(_) => false,
    };
    let mut with_files = std::collections::HashSet::new();
    if let Ok(root) = get_chats_root(paths) {
        for name in dir_names(&root) {
            let dir = root.join(name);
            if let Some(owner) = read_marker(&dir) {
                if has_visible_entry(&dir.join(ATTACHMENTS_ROOT_DIRNAME)) {
                    with_files.insert(owner);
                }
            }
        }
    }
    if let Ok(root) = legacy_root(paths) {
        for name in dir_names(&root) {
            let dir = root.join(name);
            if let Some(owner) = read_marker(&dir) {
                if has_visible_entry(&dir) {
                    with_files.insert(owner);
                }
            }
        }
    }
    session_ids
        .iter()
        .filter(|id| with_files.contains(id.as_str()))
        .cloned()
        .collect()
}

/// Old name of [`rename_chat_dir`], kept for the `chat-attachments-rename` handler.
pub fn rename_attachments_dir(paths: &AttachmentPaths, session_id: &str) -> RenameResult {
    rename_chat_dir(paths, session_id)
}

/// Old name of [`remove_chat_dir`]: deleting a chat deletes its whole folder.
pub fn remove_all_attachments(paths: &AttachmentPaths, session_id: &str) -> Result<RemovedFolder> {
    remove_chat_dir(paths, session_id)
}

/// Old name of [`remove_every_chat_dir`].
pub fn remove_every_attachments_folder(paths: &AttachmentPaths) -> Result<RemovedFolders> {
    remove_every_chat_dir(paths)
}

/// The earlier layout's `attachments/` root, kept for callers that still name it.
pub fn get_attachments_root(paths: &AttachmentPaths) -> Result<PathBuf> {
    legacy_root(paths)
}

#[cfg(test)]
pub(crate) mod tests {
    //! Port of `attachmentsManager.test.ts`.
    use super::*;
    use crate::folder_naming::to_short_id;
    use tempfile::TempDir;

    pub(crate) const SESSION_ID: &str = "session_1756900000000";

    pub(crate) struct Fixture {
        pub project: TempDir,
        pub user_data: TempDir,
        pub paths: AttachmentPaths,
    }

    impl Fixture {
        pub fn new() -> Self {
            let project = tempfile::tempdir().unwrap();
            let user_data = tempfile::tempdir().unwrap();
            let paths = AttachmentPaths {
                project_path: Some(project.path().to_path_buf()),
                user_data_path: Some(user_data.path().to_path_buf()),
            };
            Self {
                project,
                user_data,
                paths,
            }
        }

        pub fn set_chat_title(&self, session_id: &str, title: &str) {
            let dir = self.user_data.path().join("chat-sessions");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join(format!("{session_id}.json")),
                serde_json::json!({ "title": title }).to_string(),
            )
            .unwrap();
        }

        /// The earlier layout's root, `<project>/attachments`.
        pub fn attachments_root(&self) -> PathBuf {
            self.project.path().join("attachments")
        }

        pub fn chats_root(&self) -> PathBuf {
            self.project.path().join("chats")
        }

        pub fn add(&self, session_id: &str, files: &[(&str, &[u8])]) -> AttachmentAddResult {
            let files: Vec<NewAttachment> = files
                .iter()
                .map(|(name, bytes)| NewAttachment {
                    name: name.to_string(),
                    bytes: bytes.to_vec(),
                })
                .collect();
            add_attachments(&self.paths, session_id, &files).unwrap()
        }

        pub fn no_project(&mut self) {
            self.paths.project_path = None;
        }
    }

    fn basename(p: &Path) -> String {
        p.file_name().unwrap().to_string_lossy().into_owned()
    }

    fn names(listing: &AttachmentListing) -> Vec<String> {
        listing.files.iter().map(|f| f.name.clone()).collect()
    }

    mod folder_naming_and_discovery {
        use super::*;

        #[test]
        fn names_the_chat_folder_after_the_title_and_puts_attachments_inside_it() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Fix the auth bug");
            let dir = ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(
                dir,
                fx.chats_root().join("fix-the-auth-bug").join("attachments")
            );
            assert_eq!(
                ensure_chat_dir(&fx.paths, SESSION_ID).unwrap(),
                fx.chats_root().join("fix-the-auth-bug")
            );
        }

        #[test]
        fn falls_back_to_the_short_id_when_the_chat_has_no_title_yet() {
            let fx = Fixture::new();
            let dir = ensure_chat_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(basename(&dir), format!("chat-{}", to_short_id(SESSION_ID)));
        }

        #[test]
        fn resolving_creates_nothing_for_a_chat_without_a_folder() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Not yet");
            let dir = resolve_chat_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(dir, fx.chats_root().join("not-yet"));
            assert!(!fx.chats_root().exists());
        }

        #[test]
        fn writes_a_gitignore_for_attachments_and_markers_at_the_chats_root_once() {
            let fx = Fixture::new();
            ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            let gitignore = fx.chats_root().join(".gitignore");
            let body = fs::read_to_string(&gitignore).unwrap();
            assert!(
                body.contains("*/attachments/") && body.contains(MARKER_FILE),
                "{body}"
            );

            fs::write(&gitignore, "edited by hand").unwrap();
            ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(fs::read_to_string(&gitignore).unwrap(), "edited by hand");
        }

        #[test]
        fn renames_the_folder_to_follow_the_title_keeping_the_files() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "First title");
            fx.add(SESSION_ID, &[("notes.txt", b"hello")]);
            let chat = ensure_chat_dir(&fx.paths, SESSION_ID).unwrap();
            fs::write(chat.join("report.md"), "made by the agent").unwrap();

            fx.set_chat_title(SESSION_ID, "Second title");
            let result = rename_attachments_dir(&fx.paths, SESSION_ID);

            assert!(result.renamed);
            let renamed = PathBuf::from(result.directory.unwrap());
            assert_eq!(renamed, fx.chats_root().join("second-title"));
            assert!(renamed.join("report.md").exists());
            assert_eq!(
                names(&list_attachments(&fx.paths, SESSION_ID)),
                ["notes.txt"]
            );
        }

        #[test]
        fn finds_a_folder_the_user_renamed_and_names_it_after_the_title_again() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Planning");
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            fs::rename(
                fx.chats_root().join("planning"),
                fx.chats_root().join("whatever"),
            )
            .unwrap();

            assert_eq!(names(&list_attachments(&fx.paths, SESSION_ID)), ["a.txt"]);
            assert!(fx.chats_root().join("planning").exists());
        }

        #[test]
        fn finds_the_folder_again_after_a_title_change_even_with_no_explicit_rename_call() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Original");
            fx.add(SESSION_ID, &[("a.txt", b"a")]);

            fx.set_chat_title(SESSION_ID, "Renamed while the app was closed");
            assert_eq!(names(&list_attachments(&fx.paths, SESSION_ID)), ["a.txt"]);
        }

        #[test]
        fn numbers_the_folder_when_the_title_name_is_already_taken() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Mine");
            fx.add(SESSION_ID, &[("a.txt", b"a")]);

            fx.set_chat_title(SESSION_ID, "Taken");
            fs::create_dir_all(fx.chats_root().join("taken")).unwrap();

            let result = rename_attachments_dir(&fx.paths, SESSION_ID);
            assert!(result.renamed);
            assert_eq!(
                PathBuf::from(result.directory.unwrap()),
                fx.chats_root().join("taken-2")
            );
            assert_eq!(list_attachments(&fx.paths, SESSION_ID).files.len(), 1);
            // A second look keeps the numbered name rather than flipping back and forth.
            assert!(!rename_attachments_dir(&fx.paths, SESSION_ID).renamed);
        }

        #[test]
        fn keeps_two_chats_with_the_same_title_in_separate_folders() {
            let fx = Fixture::new();
            let other = "session_1756900000001";
            fx.set_chat_title(SESSION_ID, "Same title");
            fx.set_chat_title(other, "Same title");

            let first = ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            let second = ensure_attachments_dir(&fx.paths, other).unwrap();
            assert_ne!(first, second);
        }
    }

    mod adding_and_listing {
        use super::*;

        #[test]
        fn classifies_files_by_extension() {
            let fx = Fixture::new();
            fx.add(
                SESSION_ID,
                &[
                    ("notes.txt", b"text"),
                    ("sheet.xlsx", b"binary"),
                    ("shot.png", b"png"),
                ],
            );

            let listing = list_attachments(&fx.paths, SESSION_ID);
            let by_name = |n: &str| listing.files.iter().find(|f| f.name == n).unwrap().clone();
            assert_eq!(by_name("notes.txt").kind, AttachmentKind::Text);
            assert!(by_name("notes.txt").extractable);
            // Nothing in the app parses spreadsheets, so they are attached but not inlined.
            assert_eq!(by_name("sheet.xlsx").kind, AttachmentKind::Other);
            assert!(!by_name("sheet.xlsx").extractable);
            assert_eq!(by_name("shot.png").kind, AttachmentKind::Image);
        }

        #[test]
        fn de_dups_a_repeated_name_instead_of_overwriting_the_first_file() {
            let fx = Fixture::new();
            fx.add(SESSION_ID, &[("a.txt", b"first")]);
            let second = fx.add(SESSION_ID, &[("a.txt", b"second")]);

            assert_eq!(second.added[0].name, "a-1.txt");
            assert_eq!(
                fs::read_to_string(Path::new(&second.directory).join("a.txt")).unwrap(),
                "first"
            );
        }

        #[test]
        fn reports_a_rejected_file_without_failing_the_rest_of_the_batch() {
            let fx = Fixture::new();
            let oversized = vec![0u8; 4 * 1024 * 1024];
            let result = fx.add(SESSION_ID, &[("big.png", &oversized), ("fine.txt", b"ok")]);

            let added: Vec<_> = result.added.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(added, ["fine.txt"]);
            assert_eq!(result.errors.len(), 1);
            assert_eq!(result.errors[0].name, "big.png");
        }

        #[test]
        fn refuses_a_name_that_would_escape_the_folder() {
            let fx = Fixture::new();
            let result = fx.add(SESSION_ID, &[("../escaped.txt", b"nope")]);

            // The name is reduced to its basename rather than reaching the parent directory.
            assert_eq!(result.added[0].name, "escaped.txt");
            assert!(!fx.attachments_root().join("escaped.txt").exists());
        }

        #[test]
        fn has_no_files_for_a_chat_that_never_attached_anything() {
            let fx = Fixture::new();
            assert!(list_attachments(&fx.paths, SESSION_ID).files.is_empty());
        }
    }

    mod removing {
        use super::*;

        #[test]
        fn hard_deletes_one_file() {
            let fx = Fixture::new();
            let added = fx.add(SESSION_ID, &[("a.txt", b"a"), ("b.txt", b"b")]);

            let result = remove_attachment(&fx.paths, SESSION_ID, "a.txt").unwrap();
            assert!(result.removed);
            assert!(!Path::new(&added.directory).join("a.txt").exists());
            let remaining: Vec<_> = result.files.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(remaining, ["b.txt"]);
        }

        #[test]
        fn refuses_a_traversal_attempt() {
            let fx = Fixture::new();
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            let outside = fx.project.path().join("keep-me.txt");
            fs::write(&outside, "keep").unwrap();

            assert!(remove_attachment(&fx.paths, SESSION_ID, "../../keep-me.txt").is_err());
            assert!(outside.exists());
        }

        #[test]
        fn deletes_the_chat_folder_with_its_chat() {
            let fx = Fixture::new();
            let added = fx.add(SESSION_ID, &[("a.txt", b"a")]);

            assert_eq!(
                remove_all_attachments(&fx.paths, SESSION_ID).unwrap(),
                RemovedFolder { removed: true }
            );
            assert!(!Path::new(&added.directory).exists());
            // A second call is a no-op rather than an error.
            assert_eq!(
                remove_all_attachments(&fx.paths, SESSION_ID).unwrap(),
                RemovedFolder { removed: false }
            );
        }

        #[test]
        fn clears_every_folder_including_chats_the_sidebar_would_hide() {
            let fx = Fixture::new();
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            fx.add("session_1756900000002", &[("b.txt", b"b")]);

            assert_eq!(
                remove_every_attachments_folder(&fx.paths).unwrap(),
                RemovedFolders { removed: 2 }
            );
            let left: Vec<_> = fs::read_dir(fx.chats_root())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            assert_eq!(left, [".gitignore"]);
        }
    }

    mod folder_ownership {
        use super::*;

        fn mark(dir: &Path, session_id: &str) {
            fs::create_dir_all(dir).unwrap();
            fs::write(
                dir.join(MARKER_FILE),
                serde_json::json!({ "sessionId": session_id }).to_string(),
            )
            .unwrap();
        }

        #[test]
        fn new_folders_carry_a_marker_naming_the_chat() {
            let fx = Fixture::new();
            let dir = ensure_chat_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(read_marker(&dir).as_deref(), Some(SESSION_ID));
            // The marker is not an attachment.
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            assert_eq!(names(&list_attachments(&fx.paths, SESSION_ID)), ["a.txt"]);
        }

        #[test]
        fn another_chats_folder_with_the_same_short_id_is_never_touched() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Mine");
            let theirs = fx
                .attachments_root()
                .join(format!("theirs-{}", to_short_id(SESSION_ID)));
            mark(&theirs, "session_someone_else");
            fs::write(theirs.join("secret.txt"), "theirs").unwrap();

            assert!(list_attachments(&fx.paths, SESSION_ID).files.is_empty());
            assert!(!rename_attachments_dir(&fx.paths, SESSION_ID).renamed);
            assert_eq!(
                remove_all_attachments(&fx.paths, SESSION_ID).unwrap(),
                RemovedFolder { removed: false }
            );
            assert!(theirs.join("secret.txt").exists());
        }

        #[test]
        fn a_user_folder_ending_in_the_short_id_is_left_alone() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Chat work");
            let user_dir = fx
                .attachments_root()
                .join(format!("backup-{}", to_short_id(SESSION_ID)));
            fs::create_dir_all(&user_dir).unwrap();
            fs::write(user_dir.join("keep.txt"), "keep").unwrap();

            assert!(list_attachments(&fx.paths, SESSION_ID).files.is_empty());
            assert_eq!(
                remove_all_attachments(&fx.paths, SESSION_ID).unwrap(),
                RemovedFolder { removed: false }
            );
            fx.add(SESSION_ID, &[("mine.txt", b"m")]);
            assert!(user_dir.join("keep.txt").exists());
            assert!(!user_dir.join("mine.txt").exists());
            assert!(!user_dir.join(MARKER_FILE).exists());
        }

        #[test]
        fn a_legacy_folder_matching_the_title_moves_into_the_chat_folder() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "From Electron");
            let legacy = fx
                .attachments_root()
                .join(format!("from-electron-{}", to_short_id(SESSION_ID)));
            fs::create_dir_all(&legacy).unwrap();
            fs::write(legacy.join("old.txt"), "old").unwrap();
            fs::write(fx.attachments_root().join(".gitignore"), "*\n").unwrap();

            assert_eq!(names(&list_attachments(&fx.paths, SESSION_ID)), ["old.txt"]);
            let chat = fx.chats_root().join("from-electron");
            assert_eq!(read_marker(&chat).as_deref(), Some(SESSION_ID));
            assert!(chat.join("attachments/old.txt").exists());
            // Nothing of the app's is left in the old root, so it goes.
            assert!(!fx.attachments_root().exists());
            // Once moved it follows title changes like any other folder.
            fx.set_chat_title(SESSION_ID, "Renamed");
            assert!(rename_attachments_dir(&fx.paths, SESSION_ID).renamed);
            assert_eq!(
                remove_all_attachments(&fx.paths, SESSION_ID).unwrap(),
                RemovedFolder { removed: true }
            );
        }

        #[test]
        fn a_marked_legacy_folder_moves_and_its_marker_moves_up() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Current title");
            let legacy = fx
                .attachments_root()
                .join(format!("older-title-{}", to_short_id(SESSION_ID)));
            mark(&legacy, SESSION_ID);
            fs::write(legacy.join("a.txt"), "a").unwrap();
            // Someone else's folder keeps the old root alive.
            let theirs = fx.attachments_root().join("theirs-abcdef");
            mark(&theirs, "session_someone_else");

            let chat = ensure_chat_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(chat, fx.chats_root().join("current-title"));
            assert!(chat.join("attachments/a.txt").exists());
            assert!(!chat.join("attachments").join(MARKER_FILE).exists());
            assert!(theirs.exists());
        }

        #[test]
        fn a_new_folder_does_not_move_into_another_chats_folder() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Fix bug");
            let taken = fx.chats_root().join("fix-bug");
            mark(&taken, "session_someone_else");

            let dir = ensure_chat_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(basename(&dir), "fix-bug-2");
            assert_eq!(read_marker(&taken).as_deref(), Some("session_someone_else"));
            // And it is found again afterwards.
            let added = fx.add(SESSION_ID, &[("a.txt", b"a")]);
            assert_eq!(added.directory, dir.join("attachments").to_string_lossy());
        }

        #[test]
        fn delete_all_keeps_chat_folders_the_app_did_not_make() {
            let fx = Fixture::new();
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            fs::create_dir_all(fx.chats_root().join("my-own-folder")).unwrap();

            assert_eq!(
                remove_every_attachments_folder(&fx.paths).unwrap(),
                RemovedFolders { removed: 1 }
            );
            let mut left: Vec<_> = fs::read_dir(fx.chats_root())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            left.sort();
            assert_eq!(left, [".gitignore", "my-own-folder"]);
        }

        #[test]
        fn delete_all_keeps_legacy_folders_the_app_did_not_make() {
            let fx = Fixture::new();
            let root = fx.attachments_root();
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join(".gitignore"), "*\n").unwrap();
            // Electron-made folders (no marker, app name shape) go.
            fs::create_dir_all(root.join("old-chat-a1b2c3")).unwrap();
            fs::create_dir_all(root.join("session-00ff99")).unwrap();
            mark(&root.join("marked-whatever"), "session_x");
            // Anything else stays.
            for keep in [
                "Photos",
                "notes",
                "my_stuff-a1b2c3",
                "x-ABCDEF",
                "a--b-a1b2c3",
            ] {
                fs::create_dir_all(root.join(keep)).unwrap();
            }
            #[cfg(unix)]
            {
                let target = fx.project.path().join("elsewhere");
                fs::create_dir_all(&target).unwrap();
                fs::write(target.join("f.txt"), "f").unwrap();
                std::os::unix::fs::symlink(&target, root.join("linked-a1b2c3")).unwrap();
            }

            assert_eq!(
                remove_every_attachments_folder(&fx.paths).unwrap(),
                RemovedFolders { removed: 3 }
            );
            let mut left: Vec<_> = fs::read_dir(&root)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            left.sort();
            let mut expected = vec![
                ".gitignore",
                "Photos",
                "a--b-a1b2c3",
                "my_stuff-a1b2c3",
                "notes",
                "x-ABCDEF",
            ];
            if cfg!(unix) {
                expected.push("linked-a1b2c3");
                expected.sort();
                assert!(fx.project.path().join("elsewhere/f.txt").exists());
            }
            assert_eq!(left, expected);
        }

        #[test]
        fn app_folder_name_shape() {
            for ok in [
                "session-a1b2c3",
                "fix-the-auth-bug-00ff99",
                "x-abcdef",
                "a1-b2-c3d4e5",
            ] {
                assert!(is_app_folder_name(ok), "{ok}");
            }
            let long = format!("{}-abcdef", "a".repeat(41));
            for bad in [
                "Photos",
                "a1b2c3",
                "-a1b2c3",
                "x-a1b2c",
                "x-A1B2C3",
                "x--a1b2c3",
                "x_y-a1b2c3",
                "é-a1b2c3",
                long.as_str(),
            ] {
                assert!(!is_app_folder_name(bad), "{bad}");
            }
        }

        #[test]
        fn reads_the_title_from_the_start_of_a_big_session_file() {
            let fx = Fixture::new();
            let dir = fx.user_data.path().join("chat-sessions");
            fs::create_dir_all(&dir).unwrap();
            let big = "x".repeat(200 * 1024);
            fs::write(
                dir.join(format!("{SESSION_ID}.json")),
                format!(
                    r#"{{"id":"{SESSION_ID}","meta":{{"title":"nested"}},"title":"Say \"hi\"","messages":["{big}"]}}"#
                ),
            )
            .unwrap();
            assert_eq!(
                read_chat_title(&fx.paths, SESSION_ID).as_deref(),
                Some("Say \"hi\"")
            );
            // Title after the scanned prefix: the whole file is parsed.
            fs::write(
                dir.join(format!("{SESSION_ID}.json")),
                format!(r#"{{"messages":["{big}"],"title":"Late"}}"#),
            )
            .unwrap();
            assert_eq!(
                read_chat_title(&fx.paths, SESSION_ID).as_deref(),
                Some("Late")
            );
        }

        #[test]
        fn top_level_title_ignores_nested_and_string_contents() {
            assert_eq!(
                top_level_title(br#"{"a":"\"title\": \"no\"","title":"yes""#).as_deref(),
                Some("yes")
            );
            assert_eq!(
                top_level_title(br#"{"a":{"title":"no"},"b":["title"],"title" : "ok"}"#).as_deref(),
                Some("ok")
            );
            assert_eq!(top_level_title(br#"{"title":"cut off"#), None);
            assert_eq!(top_level_title(br#"{"title":null}"#), None);
        }

        #[test]
        fn unsafe_session_ids_read_no_title() {
            let fx = Fixture::new();
            fs::write(fx.user_data.path().join("x.json"), r#"{"title":"leak"}"#).unwrap();
            assert_eq!(read_chat_title(&fx.paths, "../x"), None);
        }

        #[test]
        fn lone_surrogate_titles_still_name_the_folder() {
            let fx = Fixture::new();
            let dir = fx.user_data.path().join("chat-sessions");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join(format!("{SESSION_ID}.json")),
                r#"{"title":"Plan \ud83d"}"#,
            )
            .unwrap();
            assert_eq!(
                read_chat_title(&fx.paths, SESSION_ID).as_deref(),
                Some("Plan \u{fffd}")
            );
        }
    }

    mod without_a_project_directory {
        use super::*;

        #[test]
        fn reports_no_files_instead_of_throwing() {
            let mut fx = Fixture::new();
            fx.no_project();
            assert_eq!(
                list_attachments(&fx.paths, SESSION_ID),
                AttachmentListing {
                    directory: String::new(),
                    files: vec![]
                }
            );
        }

        #[test]
        fn throws_when_asked_to_write_so_the_renderer_can_point_at_settings() {
            let mut fx = Fixture::new();
            fx.no_project();
            let files = [NewAttachment {
                name: "a.txt".into(),
                bytes: b"a".to_vec(),
            }];
            let error = add_attachments(&fx.paths, SESSION_ID, &files).unwrap_err();
            assert!(
                error
                    .to_string()
                    .to_lowercase()
                    .contains("project directory"),
                "{error}"
            );
        }

        #[test]
        fn empty_store_strings_count_as_unset() {
            assert_eq!(
                AttachmentPaths::new(Some(""), None),
                AttachmentPaths::default()
            );
        }
    }

    mod list_session_ids_with_attachments {
        use super::*;

        fn ids(values: &[&str]) -> Vec<String> {
            values.iter().map(|s| s.to_string()).collect()
        }

        #[test]
        fn returns_only_the_chats_that_actually_have_a_file() {
            let fx = Fixture::new();
            let with_file = "session_1756900000001";
            let empty_folder = "session_1756900000002";
            let no_folder = "session_1756900000003";

            fx.add(with_file, &[("notes.txt", b"hello")]);
            // A folder that exists but holds nothing must not light up the indicator.
            ensure_attachments_dir(&fx.paths, empty_folder).unwrap();

            let result = list_session_ids_with_attachments(
                &fx.paths,
                &ids(&[with_file, empty_folder, no_folder]),
            );
            assert_eq!(result, ids(&[with_file]));
        }

        #[test]
        fn ignores_dotfiles_so_a_stray_ds_store_does_not_count_as_an_attachment() {
            let fx = Fixture::new();
            let session_id = "session_1756900000004";
            let directory = ensure_attachments_dir(&fx.paths, session_id).unwrap();
            fs::write(directory.join(".DS_Store"), "junk").unwrap();

            assert!(list_session_ids_with_attachments(&fx.paths, &ids(&[session_id])).is_empty());
        }

        #[test]
        fn answers_for_many_chats_in_one_call_preserving_the_order_asked_for() {
            let fx = Fixture::new();
            let first = "session_1756900000005";
            let second = "session_1756900000006";
            fx.add(second, &[("b.txt", b"b")]);
            fx.add(first, &[("a.txt", b"a")]);

            assert_eq!(
                list_session_ids_with_attachments(&fx.paths, &ids(&[first, second])),
                ids(&[first, second])
            );
        }

        #[test]
        fn returns_nothing_rather_than_throwing_when_no_project_directory_is_set() {
            let mut fx = Fixture::new();
            fx.no_project();
            assert!(list_session_ids_with_attachments(&fx.paths, &ids(&["session_1"])).is_empty());
        }

        #[test]
        fn finds_a_chat_whose_folder_still_carries_an_old_title() {
            let fx = Fixture::new();
            let session_id = "session_1756900000007";
            fx.set_chat_title(session_id, "Old title");
            fx.add(session_id, &[("a.txt", b"a")]);

            // Renamed chat, stale folder: the lookup keys on the marker, so it still matches,
            // and (being read-only) leaves the folder for the chat itself to rename.
            fx.set_chat_title(session_id, "New title");

            assert_eq!(
                list_session_ids_with_attachments(&fx.paths, &ids(&[session_id])),
                ids(&[session_id])
            );
            assert!(fx.chats_root().join("old-title").exists());
        }
    }
}
