//! The knowledgebase folder layout and its one-time migration.
//!
//! Current layout: `<root>/kbs/<kbId>.db`, one SQLite file per knowledgebase, shared by every
//! agent that attaches it (agents list the ids in `localKnowledgeBases`). The on-device model
//! stays in `<root>/.models` and the user guide's index in `<root>/helpAgent/user-guide.db`.
//!
//! Earlier builds kept each agent's knowledgebases in `<root>/<agentId>/<kbId>.db`.
//! [`migrate_files`] moves them into `kbs/` (renames, no re-ingest), journaled in
//! `<root>/layout.json` so an interrupted run picks up where it stopped:
//!
//! 1. **Planned**: the journal lists every move and each agent's new references. Written
//!    before any file is touched.
//! 2. **FilesMoved**: every file is in `kbs/`. The app now rewrites the agents' references
//!    (see `apply_layout_refs` in the app) and calls [`mark_done`].
//! 3. **Done**: nothing left to do.
//!
//! A knowledgebase whose id is already taken (a duplicated agent copied the files with the same
//! ids) gets a new id and its owner's name as a suffix, so both copies stay; distinct ids that
//! share a name get the suffix too, so names stay unique. Folders that belong to no agent are
//! moved as well (suffixed with the folder name) and attached to nobody.
//!
//! Nothing here deletes a knowledgebase or a non-empty folder.

pub use common::agent::LocalKnowledgeBaseRef as LocalKbRef;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Folder (under the root) holding every knowledgebase.
pub const KBS_DIR: &str = "kbs";
/// The migration journal (under the root).
pub const JOURNAL_FILE: &str = "layout.json";

const JOURNAL_VERSION: u32 = 1;

pub fn kbs_dir(root: &Path) -> PathBuf {
    root.join(KBS_DIR)
}

pub fn kb_file(root: &Path, kb_id: &str) -> PathBuf {
    kbs_dir(root).join(format!("{kb_id}.db"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutState {
    Planned,
    FilesMoved,
    Done,
}

/// An agent from `customAgents`, in store order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAgent {
    pub id: String,
    pub name: String,
}

/// One knowledgebase file to move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMove {
    /// The legacy owner folder (an agent id).
    pub folder: String,
    pub old_id: String,
    pub new_id: String,
    /// The name after the move.
    pub name: String,
    pub description: String,
    /// The name got a suffix: written into the file after the move.
    #[serde(default)]
    pub renamed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    pub version: u32,
    pub state: LayoutState,
    #[serde(default)]
    pub moves: Vec<PlannedMove>,
    /// Agent id → its `localKnowledgeBases` after the migration (agents with a legacy folder).
    #[serde(default)]
    pub refs: BTreeMap<String, Vec<LocalKbRef>>,
}

impl Journal {
    fn done() -> Self {
        Self {
            version: JOURNAL_VERSION,
            state: LayoutState::Done,
            moves: Vec::new(),
            refs: BTreeMap::new(),
        }
    }
}

fn journal_path(root: &Path) -> PathBuf {
    root.join(JOURNAL_FILE)
}

/// The journal, `None` when there is none yet.
pub fn read_journal(root: &Path) -> Result<Option<Journal>, String> {
    let path = journal_path(root);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("Unreadable {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Cannot read {}: {e}", path.display())),
    }
}

fn write_journal(root: &Path, journal: &Journal) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(journal).map_err(|e| e.to_string())?;
    common::json_file::write_atomic(&journal_path(root), &bytes)
        .map_err(|e| format!("Cannot write {}: {e}", journal_path(root).display()))
}

/// Knowledgebase ids (`<id>.db`) in a folder, sorted.
pub(crate) fn db_ids(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".db")?.to_string();
            common::json_file::is_safe_file_id(&id).then_some(id)
        })
        .collect();
    ids.sort();
    ids
}

/// Folders under the root in the old per-agent layout that still hold knowledgebase files:
/// any folder with an id-like name and `.db` files, except `kbs`, `.models` and the user
/// guide's `helpAgent`. Sorted.
pub fn legacy_folders(root: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| common::json_file::is_safe_file_id(n))
        .filter(|n| n != KBS_DIR && n != crate::help::HELP_OWNER && n != crate::service::MODELS_DIR)
        .filter(|n| !db_ids(&root.join(n)).is_empty())
        .collect();
    out.sort();
    out
}

/// Name and description from a knowledgebase file, read without changing it.
fn read_meta(path: &Path) -> Option<(String, String)> {
    crate::store::register_vec();
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let get = |k: &str| -> Option<String> {
        conn.query_row("SELECT value FROM meta WHERE key = ?1", [k], |r| r.get(0))
            .ok()
    };
    Some((get("name")?, get("description").unwrap_or_default()))
}

fn plan(root: &Path, agents: &[LegacyAgent], folders: &[String]) -> Journal {
    // (folder, suffix label, owned by an agent)
    let mut order: Vec<(String, String, bool)> = Vec::new();
    for a in agents {
        if folders.contains(&a.id) && !order.iter().any(|(f, _, _)| f == &a.id) {
            let label = if a.name.trim().is_empty() {
                a.id.clone()
            } else {
                a.name.trim().to_string()
            };
            order.push((a.id.clone(), label, true));
        }
    }
    for f in folders {
        if !order.iter().any(|(o, _, _)| o == f) {
            order.push((f.clone(), f.clone(), false));
        }
    }

    // Anything already in `kbs/` keeps its id and name.
    let mut taken_ids: HashSet<String> = HashSet::new();
    let mut taken_names: HashSet<String> = HashSet::new();
    for id in db_ids(&kbs_dir(root)) {
        if let Some((name, _)) = read_meta(&kb_file(root, &id)) {
            taken_names.insert(name.trim().to_lowercase());
        }
        taken_ids.insert(id);
    }

    let mut moves = Vec::new();
    let mut refs: BTreeMap<String, Vec<LocalKbRef>> = BTreeMap::new();
    for (folder, label, owned) in order {
        let dir = root.join(&folder);
        for old_id in db_ids(&dir) {
            let (name, description) = read_meta(&dir.join(format!("{old_id}.db")))
                .map(|(n, d)| (n.trim().to_string(), d))
                .filter(|(n, _)| !n.is_empty())
                .unwrap_or_else(|| (old_id.clone(), String::new()));
            let collision = taken_ids.contains(&old_id);
            let new_id = if collision {
                loop {
                    let id = crate::service::new_id("kb");
                    if !taken_ids.contains(&id) {
                        break id;
                    }
                }
            } else {
                old_id.clone()
            };
            let mut final_name = name.clone();
            if collision || taken_names.contains(&name.to_lowercase()) {
                final_name = format!("{name} ({label})");
            }
            let base = final_name.clone();
            let mut n = 2;
            while taken_names.contains(&final_name.to_lowercase()) {
                final_name = format!("{base} {n}");
                n += 1;
            }
            taken_ids.insert(new_id.clone());
            taken_names.insert(final_name.to_lowercase());
            if owned {
                refs.entry(folder.clone()).or_default().push(LocalKbRef {
                    id: new_id.clone(),
                    name: final_name.clone(),
                    description: description.clone(),
                });
            }
            moves.push(PlannedMove {
                folder: folder.clone(),
                old_id,
                new_id,
                renamed: final_name != name,
                name: final_name,
                description,
            });
        }
    }
    Journal {
        version: JOURNAL_VERSION,
        state: LayoutState::Planned,
        moves,
        refs,
    }
}

/// Fold the write-ahead log into the main file and leave WAL mode, so the database is one
/// file that can be renamed. Fails (and the move waits for a later launch) when another
/// connection holds the file.
fn checkpoint_to_single_file(path: &Path) -> Result<(), String> {
    crate::store::register_vec();
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    let mode: String = conn
        .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    conn.close().map_err(|(_, e)| e.to_string())?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err(format!("Could not leave WAL mode (journal mode is {mode})"));
    }
    // Checkpointed and closed: anything left is stale.
    for f in crate::store::db_files(path).iter().skip(1) {
        if f.exists() {
            let _ = std::fs::remove_file(f);
        }
    }
    Ok(())
}

fn write_name(path: &Path, name: &str, description: &str) -> Result<(), String> {
    crate::store::register_vec();
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    for (k, v) in [("name", name), ("description", description)] {
        conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [k, v],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.close().map_err(|(_, e)| e.to_string())
}

fn move_one(root: &Path, m: &PlannedMove) -> Result<(), String> {
    let src = root.join(&m.folder).join(format!("{}.db", m.old_id));
    let dest = kb_file(root, &m.new_id);
    if !src.exists() {
        if !dest.exists() {
            tracing::warn!(folder = %m.folder, kb_id = %m.old_id, "Knowledge base to move is gone");
            return Ok(());
        }
    } else {
        if dest.exists() {
            return Err(format!("{} already exists", dest.display()));
        }
        checkpoint_to_single_file(&src)?;
        std::fs::create_dir_all(kbs_dir(root)).map_err(|e| e.to_string())?;
        std::fs::rename(&src, &dest)
            .map_err(|e| format!("Cannot move {} to {}: {e}", src.display(), dest.display()))?;
    }
    if m.renamed && read_meta(&dest).is_none_or(|(n, _)| n != m.name) {
        write_name(&dest, &m.name, &m.description)?;
    }
    Ok(())
}

fn move_files(root: &Path, journal: &Journal) -> bool {
    let mut ok = true;
    for m in &journal.moves {
        if let Err(e) = move_one(root, m) {
            tracing::warn!(folder = %m.folder, kb_id = %m.old_id, error = %e,
                "Could not move knowledge base; retrying on next launch");
            ok = false;
        }
    }
    let folders: HashSet<&str> = journal.moves.iter().map(|m| m.folder.as_str()).collect();
    for f in folders {
        let dir = root.join(f);
        if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&dir);
        }
    }
    ok
}

/// Move legacy knowledgebases into `kbs/` (see the module docs). Returns the journal when the
/// files are all moved and the agents' references still need rewriting (then call
/// [`mark_done`]); `None` when there is nothing to do (yet).
pub fn migrate_files(root: &Path, agents: &[LegacyAgent]) -> Result<Option<Journal>, String> {
    let mut journal = match read_journal(root)? {
        Some(j) => j,
        None => {
            let folders = legacy_folders(root);
            if folders.is_empty() {
                if root.is_dir() {
                    write_journal(root, &Journal::done())?;
                }
                return Ok(None);
            }
            let j = plan(root, agents, &folders);
            write_journal(root, &j)?;
            tracing::info!(
                knowledge_bases = j.moves.len(),
                renamed = j.moves.iter().filter(|m| m.old_id != m.new_id).count(),
                "Moving knowledge bases to the shared folder"
            );
            j
        }
    };
    match journal.state {
        LayoutState::Done => Ok(None),
        LayoutState::FilesMoved => Ok(Some(journal)),
        LayoutState::Planned => {
            if !move_files(root, &journal) {
                return Ok(None);
            }
            journal.state = LayoutState::FilesMoved;
            write_journal(root, &journal)?;
            Ok(Some(journal))
        }
    }
}

/// The agents' references are rewritten: the migration is finished.
pub fn mark_done(root: &Path) -> Result<(), String> {
    let mut journal = read_journal(root)?.unwrap_or_else(Journal::done);
    journal.state = LayoutState::Done;
    write_journal(root, &journal)
}

/// Knowledgebases that got a new id in the migration: `(old owner agent id, old id, new id)`.
/// Saved chats still name them by the old owner and id.
pub fn renamed(root: &Path) -> Vec<(String, String, String)> {
    read_journal(root)
        .ok()
        .flatten()
        .map(|j| {
            j.moves
                .into_iter()
                .filter(|m| m.old_id != m.new_id)
                .map(|m| (m.folder, m.old_id, m.new_id))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::meta;
    use crate::store::{KbStore, NewChunk};

    fn agent(id: &str, name: &str) -> LegacyAgent {
        LegacyAgent {
            id: id.into(),
            name: name.into(),
        }
    }

    /// A legacy knowledgebase `<root>/<folder>/<id>.db` named `name` with one passage.
    fn legacy(root: &Path, folder: &str, id: &str, name: &str) {
        let mut m = meta(3);
        m.name = name.into();
        let s = KbStore::create(&root.join(folder).join(format!("{id}.db")), &m).unwrap();
        s.add_source(
            "s1",
            crate::types::SourceKind::File,
            "/a.md",
            "a.md",
            None,
            "t",
        )
        .unwrap();
        s.replace_document(
            "s1",
            "/a.md",
            "a.md",
            "h",
            5,
            &[NewChunk {
                heading_path: String::new(),
                text: format!("passage of {name}"),
                embedding: vec![1.0, 0.0, 0.0],
            }],
        )
        .unwrap();
    }

    fn name_of(path: &Path) -> String {
        KbStore::open(path).unwrap().meta().unwrap().name
    }

    #[test]
    fn plain_move_keeps_ids_and_refs() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        legacy(root, "a1", "kb_one", "Docs");
        legacy(root, "a1", "kb_two", "Notes");
        let j = migrate_files(root, &[agent("a1", "Writer")])
            .unwrap()
            .unwrap();
        assert_eq!(j.state, LayoutState::FilesMoved);
        assert!(kb_file(root, "kb_one").is_file());
        assert!(kb_file(root, "kb_two").is_file());
        assert!(!root.join("a1").exists(), "empty folder removed");
        assert_eq!(
            j.refs["a1"],
            vec![
                LocalKbRef {
                    id: "kb_one".into(),
                    name: "Docs".into(),
                    description: "d".into()
                },
                LocalKbRef {
                    id: "kb_two".into(),
                    name: "Notes".into(),
                    description: "d".into()
                },
            ]
        );
        assert!(renamed(root).is_empty());
        assert_eq!(name_of(&kb_file(root, "kb_one")), "Docs");
        // Stays FilesMoved until the app marks it done; then nothing more happens.
        assert!(migrate_files(root, &[]).unwrap().is_some());
        mark_done(root).unwrap();
        assert!(migrate_files(root, &[]).unwrap().is_none());
        assert_eq!(
            read_journal(root).unwrap().unwrap().state,
            LayoutState::Done
        );
    }

    #[test]
    fn nothing_to_move_writes_done() {
        let d = tempfile::tempdir().unwrap();
        assert!(migrate_files(d.path(), &[agent("a", "A")])
            .unwrap()
            .is_none());
        assert_eq!(
            read_journal(d.path()).unwrap().unwrap().state,
            LayoutState::Done
        );
        // No root yet (fresh install): nothing is created.
        let missing = d.path().join("nope");
        assert!(migrate_files(&missing, &[]).unwrap().is_none());
        assert!(!missing.exists());
    }

    #[test]
    fn duplicated_agent_copy_gets_new_id_and_suffix() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        // "copy" was duplicated from "orig": same kb id in both folders.
        legacy(root, "orig", "kb_x", "Docs");
        legacy(root, "copy", "kb_x", "Docs");
        let agents = [agent("orig", "Original"), agent("copy", "Copy of Original")];
        let j = migrate_files(root, &agents).unwrap().unwrap();
        assert_eq!(j.refs["orig"][0].id, "kb_x");
        assert_eq!(j.refs["orig"][0].name, "Docs");
        let copy = &j.refs["copy"][0];
        assert_ne!(copy.id, "kb_x");
        assert!(copy.id.starts_with("kb_"));
        assert_eq!(copy.name, "Docs (Copy of Original)");
        assert_eq!(name_of(&kb_file(root, &copy.id)), "Docs (Copy of Original)");
        assert_eq!(name_of(&kb_file(root, "kb_x")), "Docs");
        assert_eq!(
            renamed(root),
            vec![("copy".to_string(), "kb_x".to_string(), copy.id.clone())]
        );
    }

    #[test]
    fn distinct_ids_sharing_a_name_get_suffixes() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        legacy(root, "a", "kb_1", "Docs");
        legacy(root, "b", "kb_2", "docs");
        legacy(root, "b", "kb_3", "Docs");
        let j = migrate_files(root, &[agent("a", "Alice"), agent("b", "Bob")])
            .unwrap()
            .unwrap();
        let names: Vec<&str> = j.moves.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Docs", "docs (Bob)", "Docs (Bob) 2"]);
        assert!(j.moves.iter().all(|m| m.old_id == m.new_id));
        assert_eq!(name_of(&kb_file(root, "kb_3")), "Docs (Bob) 2");
    }

    #[test]
    fn unflushed_wal_rows_survive_the_move() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        // Rows written while the connection stays open sit in the WAL. Copy the file pair as a
        // crash would leave it (no checkpoint, no shared-memory file).
        let live = d.path().join("live").join("kb_w.db");
        let store = KbStore::create(&live, &meta(3)).unwrap();
        for i in 0..20 {
            store
                .add_source(
                    &format!("s{i}"),
                    crate::types::SourceKind::File,
                    &format!("/{i}"),
                    "t",
                    None,
                    "t",
                )
                .unwrap();
        }
        let [main, wal, _] = crate::store::db_files(&live);
        assert!(
            std::fs::metadata(&wal).unwrap().len() > 0,
            "rows are in the WAL"
        );
        let path = root.join("a").join("kb_w.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(&main, &path).unwrap();
        std::fs::copy(&wal, &crate::store::db_files(&path)[1]).unwrap();
        drop(store);
        std::fs::remove_dir_all(d.path().join("live")).unwrap();

        let j = migrate_files(root, &[agent("a", "A")]).unwrap().unwrap();
        assert_eq!(j.state, LayoutState::FilesMoved);
        let moved = kb_file(root, "kb_w");
        assert!(!crate::store::db_files(&moved)[1].exists());
        assert!(!root.join("a").exists());
        assert_eq!(
            KbStore::open(&moved).unwrap().list_sources().unwrap().len(),
            20
        );
    }

    #[test]
    fn resumes_from_planned_half_moved() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        legacy(root, "orig", "kb_x", "Docs");
        legacy(root, "copy", "kb_x", "Docs");
        legacy(root, "copy", "kb_y", "Other");
        let agents = [agent("orig", "Original"), agent("copy", "Copy")];
        let folders = legacy_folders(root);
        let j = plan(root, &agents, &folders);
        write_journal(root, &j).unwrap();
        // Crash after the first two moves (the second without its rename).
        move_one(root, &j.moves[0]).unwrap();
        let second = &j.moves[1];
        assert!(second.renamed);
        let src = root.join("copy").join("kb_x.db");
        checkpoint_to_single_file(&src).unwrap();
        std::fs::rename(&src, kb_file(root, &second.new_id)).unwrap();

        // The next launch follows the same plan, even if the agents changed.
        let resumed = migrate_files(root, &[]).unwrap().unwrap();
        assert_eq!(resumed.moves, j.moves);
        assert_eq!(resumed.refs, j.refs);
        assert_eq!(name_of(&kb_file(root, &second.new_id)), "Docs (Copy)");
        assert!(kb_file(root, "kb_y").is_file());
        assert!(kb_file(root, "kb_x").is_file());
        assert!(!root.join("orig").exists() && !root.join("copy").exists());
        // And from FilesMoved: the same journal again, no file work.
        let again = migrate_files(root, &[]).unwrap().unwrap();
        assert_eq!(again.refs, j.refs);
    }

    #[test]
    fn a_move_that_cannot_finish_waits() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        legacy(root, "a", "kb_1", "Docs");
        legacy(root, "a", "kb_2", "Notes");
        let j = plan(root, &[agent("a", "A")], &legacy_folders(root));
        write_journal(root, &j).unwrap();
        // Something else sits at kb_2's destination.
        std::fs::create_dir_all(kbs_dir(root)).unwrap();
        std::fs::write(kb_file(root, "kb_2"), "x").unwrap();
        assert!(migrate_files(root, &[]).unwrap().is_none());
        assert_eq!(
            read_journal(root).unwrap().unwrap().state,
            LayoutState::Planned
        );
        assert!(kb_file(root, "kb_1").is_file());
        assert!(root.join("a").join("kb_2.db").is_file(), "left in place");
        std::fs::remove_file(kb_file(root, "kb_2")).unwrap();
        assert!(migrate_files(root, &[]).unwrap().is_some());
        assert!(!root.join("a").exists());
    }

    #[test]
    fn help_models_and_orphans() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        legacy(root, crate::help::HELP_OWNER, "user-guide", "Guide");
        std::fs::create_dir_all(root.join(".models").join("e5")).unwrap();
        std::fs::write(root.join(".models").join("e5").join("model.db"), "m").unwrap();
        legacy(root, "a", "kb_1", "Docs");
        legacy(root, "gone-agent", "kb_1", "Docs");
        legacy(root, "gone-agent", "kb_2", "Orphaned");
        // A non-database file keeps the orphan folder.
        std::fs::write(root.join("gone-agent").join("notes.txt"), "keep").unwrap();

        let j = migrate_files(root, &[agent("a", "A")]).unwrap().unwrap();
        assert!(root
            .join(crate::help::HELP_OWNER)
            .join("user-guide.db")
            .is_file());
        assert!(root.join(".models").join("e5").join("model.db").is_file());
        assert!(!kb_file(root, "user-guide").exists());
        assert_eq!(j.refs.keys().collect::<Vec<_>>(), ["a"]);
        let orphan: Vec<&PlannedMove> = j
            .moves
            .iter()
            .filter(|m| m.folder == "gone-agent")
            .collect();
        assert_eq!(orphan.len(), 2);
        assert_eq!(orphan[0].name, "Docs (gone-agent)");
        assert_ne!(orphan[0].new_id, "kb_1");
        assert_eq!(orphan[1].new_id, "kb_2");
        assert_eq!(orphan[1].name, "Orphaned");
        assert!(kb_file(root, &orphan[0].new_id).is_file());
        assert!(kb_file(root, "kb_2").is_file());
        assert!(root.join("gone-agent").join("notes.txt").is_file());
        assert!(legacy_folders(root).is_empty());
    }
}
