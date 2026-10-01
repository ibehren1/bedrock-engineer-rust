//! Per-chat attachments folders (port of `attachmentsManager.ts` and `chatSessionTitle.ts`).
//!
//! ## Folder ownership
//!
//! Folders are named `<title-slug>-<shortId>` where `shortId` is only 6 hex chars of a hash, so
//! two chats (or a folder the user made) can share a suffix. The TS code picked "the first folder
//! ending in `-<shortId>`" and renamed / deleted it. Here a folder counts as a chat's only when
//! it is proven to be:
//!
//! - it holds a [`MARKER_FILE`] (`.chat-session.json`, `{"sessionId": ...}`) naming the chat —
//!   written whenever this build creates or uses the folder (dotfiles are ignored by listings,
//!   request context and the Electron build); or
//! - it has no marker (made by the Electron build) and its name is exactly the one the chat's
//!   current title gives ([`to_folder_name`]); the marker is written once that matches.
//!
//! A marker-less folder carrying an old title can't be proven and is left alone (the chat gets a
//! new folder). "Delete all" removes only folders with a marker or whose name has the app's exact
//! shape ([`is_app_folder_name`]); anything else under `attachments/` is kept.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::error::{Error, Result};
use crate::file_naming::{
    resolve_inside, resolve_lexically, sanitize_attachment_name, unique_name_in,
};
use crate::folder_naming::{slugify_title, to_folder_name, to_short_id};
use crate::image_validation::{is_image_extension, validate_image_bytes};
use crate::types::*;
use common::json_file::{is_safe_file_id, parse_lenient, write_atomic};

/// The two store values the TS code read on every call (`projectPath`, `userDataPath`).
///
/// Build one from the store per operation so a changed project directory takes effect
/// immediately, as it did in Electron.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttachmentPaths {
    /// Project directory; attachments live at `<project>/attachments/`.
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

/// Read a chat's current title from the session file the chat history writes.
pub fn read_chat_title(paths: &AttachmentPaths, session_id: &str) -> Option<String> {
    if !is_safe_file_id(session_id) {
        return None;
    }
    let user_data = paths.user_data_path.as_deref()?;
    let file = user_data
        .join("chat-sessions")
        .join(format!("{session_id}.json"));
    let raw = fs::read_to_string(file).ok()?;
    let parsed: serde_json::Value = parse_lenient(&raw).ok()?;
    parsed.get("title")?.as_str().map(str::to_string)
}

/// Ownership marker inside each chat folder this build creates or verifies.
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

/// True for names [`to_folder_name`] can produce: `<slug>-<6 hex>`, where the slug is 1-40 of
/// `[a-z0-9]` in dash-separated runs (no leading, trailing or doubled dash).
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

/// Folder names under `root` ending in `-<shortId>` (real directories only), in name order.
fn suffix_candidates(root: &Path, session_id: &str) -> Vec<String> {
    let suffix = format!("-{}", to_short_id(session_id));
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(&suffix))
        .collect();
    names.sort();
    names
}

/// The folder proven to belong to `session_id` (see the module docs), writing the marker into a
/// verified legacy folder. `expected` is the name the chat's current title gives.
fn find_owned_folder(root: &Path, session_id: &str, expected: &str) -> Option<PathBuf> {
    let mut legacy_match = None;
    for name in suffix_candidates(root, session_id) {
        let dir = root.join(&name);
        match read_marker(&dir) {
            Some(owner) if owner == session_id => return Some(dir),
            Some(_) => {}
            None if name == expected => legacy_match = Some(dir),
            None => {}
        }
    }
    let dir = legacy_match?;
    ensure_marker(&dir, session_id);
    Some(dir)
}

/// The name for a chat's folder: [`to_folder_name`], or — when a folder of that name exists
/// but belongs to someone else — `<slug>-<n>-<shortId>` for the first free `n`.
fn claim_folder_name(root: &Path, session_id: &str, title: Option<&str>) -> String {
    let desired = to_folder_name(session_id, title);
    let free_or_ours = |name: &str| {
        let dir = root.join(name);
        !dir.exists() || (is_real_dir(&dir) && read_marker(&dir).as_deref() == Some(session_id))
    };
    if free_or_ours(&desired) {
        return desired;
    }
    let slug = title.map(slugify_title).filter(|s| !s.is_empty());
    let slug = slug.as_deref().unwrap_or("session");
    let short = to_short_id(session_id);
    (2..)
        .map(|n| format!("{slug}-{n}-{short}"))
        .find(|name| free_or_ours(name))
        .expect("an unused name")
}

/// `<projectPath>/attachments`. Errors when no project directory is configured.
pub fn get_attachments_root(paths: &AttachmentPaths) -> Result<PathBuf> {
    Ok(paths.project_path()?.join(ATTACHMENTS_ROOT_DIRNAME))
}

/// Create the root and a self-ignoring `.gitignore` (written once) so attachments stay out of
/// the user's repository.
fn ensure_attachments_root(paths: &AttachmentPaths) -> Result<PathBuf> {
    let root = get_attachments_root(paths)?;
    fs::create_dir_all(&root)?;

    let gitignore = root.join(".gitignore");
    if !gitignore.exists() {
        fs::write(
            &gitignore,
            "# Chat-scoped attachments saved by Bedrock Engineer.\n*\n",
        )?;
    }
    Ok(root)
}

/// The folder a chat's attachments live in, renamed to follow the current chat title when the
/// two disagree. Creates nothing.
pub fn resolve_attachments_dir(paths: &AttachmentPaths, session_id: &str) -> Result<PathBuf> {
    let root = get_attachments_root(paths)?;
    let title = read_chat_title(paths, session_id);
    let expected = to_folder_name(session_id, title.as_deref());
    let Some(existing) = find_owned_folder(&root, session_id, &expected) else {
        // New folder: the title's name unless another chat (or the user) already has it.
        return Ok(root.join(claim_folder_name(&root, session_id, title.as_deref())));
    };
    let desired = root.join(expected);

    if resolve_lexically(&existing) == resolve_lexically(&desired) {
        return Ok(existing);
    }

    // A collision means another chat already owns that name; leave this folder alone.
    if desired.exists() {
        return Ok(existing);
    }

    match fs::rename(&existing, &desired) {
        Ok(()) => Ok(desired),
        Err(_) => Ok(existing),
    }
}

/// [`resolve_attachments_dir`] plus creating the folders themselves.
pub fn ensure_attachments_dir(paths: &AttachmentPaths, session_id: &str) -> Result<PathBuf> {
    ensure_attachments_root(paths)?;
    let directory = resolve_attachments_dir(paths, session_id)?;
    fs::create_dir_all(&directory)?;
    ensure_marker(&directory, session_id);
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

/// Explicit rename hook for the title-change path.
pub fn rename_attachments_dir(paths: &AttachmentPaths, session_id: &str) -> RenameResult {
    let not_renamed = RenameResult {
        renamed: false,
        directory: None,
    };
    let Ok(root) = get_attachments_root(paths) else {
        return not_renamed;
    };
    let title = read_chat_title(paths, session_id);
    let expected = to_folder_name(session_id, title.as_deref());
    let Some(before) = find_owned_folder(&root, session_id, &expected) else {
        return not_renamed;
    };

    let after = resolve_attachments_dir(paths, session_id).unwrap_or_else(|_| before.clone());
    RenameResult {
        renamed: resolve_lexically(&after) != resolve_lexically(&before),
        directory: Some(path_string(&after)),
    }
}

/// Delete a chat's attachments folder, used when the chat itself is deleted. Only a folder
/// proven to be the chat's (see the module docs) is deleted.
pub fn remove_all_attachments(paths: &AttachmentPaths, session_id: &str) -> Result<RemovedFolder> {
    let Ok(root) = get_attachments_root(paths) else {
        return Ok(RemovedFolder { removed: false });
    };
    let title = read_chat_title(paths, session_id);
    let expected = to_folder_name(session_id, title.as_deref());
    let Some(directory) = find_owned_folder(&root, session_id, &expected) else {
        return Ok(RemovedFolder { removed: false });
    };
    if !directory.exists() {
        return Ok(RemovedFolder { removed: false });
    }
    fs::remove_dir_all(&directory)?;
    Ok(RemovedFolder { removed: true })
}

/// Of the given chats, which ones have at least one (non-hidden) attached file, in the order
/// asked for. Errors (no project directory, vanished folder) just mean "no files".
pub fn list_session_ids_with_attachments(
    paths: &AttachmentPaths,
    session_ids: &[String],
) -> Vec<String> {
    session_ids
        .iter()
        .filter(|session_id| {
            // Renames a stale folder to follow the chat title, which is wanted here anyway.
            let Ok(directory) = resolve_attachments_dir(paths, session_id) else {
                return false;
            };
            match fs::read_dir(&directory) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .any(|e| !e.file_name().to_string_lossy().starts_with('.')),
                Err(_) => false,
            }
        })
        .cloned()
        .collect()
}

/// Delete every attachments folder (for "delete all chats"), working off the folders on disk so
/// chats the sidebar hides are reached too. Files at the root (the `.gitignore`), symlinks, and
/// folders that neither carry a [`MARKER_FILE`] nor have the app's exact name shape
/// ([`is_app_folder_name`]) are kept.
pub fn remove_every_attachments_folder(paths: &AttachmentPaths) -> Result<RemovedFolders> {
    let Ok(root) = get_attachments_root(paths) else {
        return Ok(RemovedFolders { removed: 0 });
    };
    if !root.exists() {
        return Ok(RemovedFolders { removed: 0 });
    }

    let mut removed = 0;
    for entry in sorted_entries(&root)? {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if read_marker(&entry.path()).is_none() && !is_app_folder_name(&name) {
            tracing::info!(folder = %name, "Keeping a folder the app did not create");
            continue;
        }
        fs::remove_dir_all(entry.path())?;
        removed += 1;
    }
    Ok(RemovedFolders { removed })
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

        pub fn attachments_root(&self) -> PathBuf {
            self.project.path().join("attachments")
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
        fn names_a_folder_after_the_chat_title_plus_the_chat_short_id() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Fix the auth bug");
            let dir = ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(
                basename(&dir),
                format!("fix-the-auth-bug-{}", to_short_id(SESSION_ID))
            );
        }

        #[test]
        fn falls_back_to_the_short_id_when_the_chat_has_no_title_yet() {
            let fx = Fixture::new();
            let dir = ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(
                basename(&dir),
                format!("session-{}", to_short_id(SESSION_ID))
            );
        }

        #[test]
        fn writes_a_self_ignoring_gitignore_at_the_attachments_root_once() {
            let fx = Fixture::new();
            ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            let gitignore = fx.attachments_root().join(".gitignore");
            assert!(fs::read_to_string(&gitignore).unwrap().contains('*'));

            fs::write(&gitignore, "edited by hand").unwrap();
            ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(fs::read_to_string(&gitignore).unwrap(), "edited by hand");
        }

        #[test]
        fn renames_the_folder_to_follow_the_title_keeping_the_files() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "First title");
            fx.add(SESSION_ID, &[("notes.txt", b"hello")]);

            fx.set_chat_title(SESSION_ID, "Second title");
            let result = rename_attachments_dir(&fx.paths, SESSION_ID);

            assert!(result.renamed);
            assert_eq!(
                basename(Path::new(result.directory.as_deref().unwrap())),
                format!("second-title-{}", to_short_id(SESSION_ID))
            );
            assert_eq!(
                names(&list_attachments(&fx.paths, SESSION_ID)),
                ["notes.txt"]
            );
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
        fn leaves_the_folder_alone_when_the_target_name_is_already_taken() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Mine");
            fx.add(SESSION_ID, &[("a.txt", b"a")]);

            fx.set_chat_title(SESSION_ID, "Taken");
            fs::create_dir(
                fx.attachments_root()
                    .join(format!("taken-{}", to_short_id(SESSION_ID))),
            )
            .unwrap();

            let result = rename_attachments_dir(&fx.paths, SESSION_ID);
            assert!(!result.renamed);
            assert_eq!(
                basename(Path::new(result.directory.as_deref().unwrap())),
                format!("mine-{}", to_short_id(SESSION_ID))
            );
            assert_eq!(list_attachments(&fx.paths, SESSION_ID).files.len(), 1);
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
            let left: Vec<_> = fs::read_dir(fx.attachments_root())
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
            let dir = ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
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
        fn a_legacy_folder_matching_the_title_is_adopted() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "From Electron");
            let legacy = fx
                .attachments_root()
                .join(format!("from-electron-{}", to_short_id(SESSION_ID)));
            fs::create_dir_all(&legacy).unwrap();
            fs::write(legacy.join("old.txt"), "old").unwrap();

            assert_eq!(names(&list_attachments(&fx.paths, SESSION_ID)), ["old.txt"]);
            assert_eq!(read_marker(&legacy).as_deref(), Some(SESSION_ID));
            // Once adopted it follows title changes like any other folder.
            fx.set_chat_title(SESSION_ID, "Renamed");
            assert!(rename_attachments_dir(&fx.paths, SESSION_ID).renamed);
            assert_eq!(
                remove_all_attachments(&fx.paths, SESSION_ID).unwrap(),
                RemovedFolder { removed: true }
            );
        }

        #[test]
        fn a_new_folder_does_not_move_into_another_chats_folder() {
            let fx = Fixture::new();
            fx.set_chat_title(SESSION_ID, "Fix bug");
            let taken = fx
                .attachments_root()
                .join(format!("fix-bug-{}", to_short_id(SESSION_ID)));
            mark(&taken, "session_someone_else");

            let dir = ensure_attachments_dir(&fx.paths, SESSION_ID).unwrap();
            assert_eq!(
                basename(&dir),
                format!("fix-bug-2-{}", to_short_id(SESSION_ID))
            );
            assert_eq!(read_marker(&taken).as_deref(), Some("session_someone_else"));
            // And it is found again afterwards.
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            assert_eq!(
                list_attachments(&fx.paths, SESSION_ID).directory,
                dir.to_string_lossy()
            );
        }

        #[test]
        fn delete_all_keeps_folders_the_app_did_not_make() {
            let fx = Fixture::new();
            fx.add(SESSION_ID, &[("a.txt", b"a")]);
            let root = fx.attachments_root();
            // Electron-made folders (no marker, app name shape) go.
            fs::create_dir_all(root.join("old-chat-a1b2c3")).unwrap();
            fs::create_dir_all(root.join("session-00ff99")).unwrap();
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

            // Renamed chat, stale folder: the lookup keys on the short id, so it still matches.
            fx.set_chat_title(session_id, "New title");

            assert_eq!(
                list_session_ids_with_attachments(&fx.paths, &ids(&[session_id])),
                ids(&[session_id])
            );
            assert!(fx
                .attachments_root()
                .join(format!("new-title-{}", to_short_id(session_id)))
                .exists());
        }
    }
}
