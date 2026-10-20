//! Debounced recursive watching of the project folder.
//!
//! Raw events are collected for [`DEBOUNCE`] and delivered as one [`Changes`] batch of
//! root-relative paths (deduplicated, in first-seen order). Paths under [`crate::SKIP_NAMES`] or
//! matched by the root `.gitignore` are dropped (the matcher is reloaded when `.gitignore`
//! itself changes); access-only events are ignored.

use crate::{canonical_root, gitignore_matcher, is_skipped, relative_to, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, NoCache};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// One batch of changes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    /// The watched (canonical) project folder.
    pub root: String,
    /// Root-relative paths that were created, modified, renamed or removed.
    pub paths: Vec<String>,
}

/// A running watcher; dropping it stops watching.
pub struct ProjectWatcher {
    root: PathBuf,
    _debouncer: Debouncer<RecommendedWatcher, NoCache>,
}

impl std::fmt::Debug for ProjectWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectWatcher")
            .field("root", &self.root)
            .finish()
    }
}

impl ProjectWatcher {
    /// Watch `root` recursively, calling `on_change` from the debouncer's thread for every
    /// non-empty batch. Can take a while on large trees (inotify adds a watch per folder).
    pub fn start(root: &Path, on_change: impl Fn(Changes) + Send + 'static) -> Result<Self> {
        let root = canonical_root(root)?;
        let display = root.display().to_string();
        let filter_root = root.clone();
        let mut matcher = gitignore_matcher(&root);
        let handler = move |result: DebounceEventResult| {
            let events = match result {
                Ok(events) => events,
                Err(errors) => {
                    for e in errors {
                        tracing::warn!(category = "workspace-files", error = %e, "Watch error");
                    }
                    return;
                }
            };
            let mut seen = HashSet::new();
            let mut paths = Vec::new();
            for event in events {
                if matches!(event.kind, EventKind::Access(_)) {
                    continue;
                }
                for p in &event.paths {
                    let Some(rel) = relative_to(&filter_root, p) else {
                        continue;
                    };
                    if rel == ".gitignore" {
                        matcher = gitignore_matcher(&filter_root);
                    }
                    if rel.is_empty() || is_skipped(&matcher, &rel) {
                        continue;
                    }
                    if seen.insert(rel.clone()) {
                        paths.push(rel);
                    }
                }
            }
            if !paths.is_empty() {
                on_change(Changes {
                    root: display.clone(),
                    paths,
                });
            }
        };
        let io =
            |e: notify::Error| crate::Error::Io(format!("Can't watch the project folder: {e}"));
        let mut debouncer = new_debouncer_opt::<_, RecommendedWatcher, NoCache>(
            DEBOUNCE,
            None,
            handler,
            NoCache,
            notify::Config::default(),
        )
        .map_err(io)?;
        debouncer
            .watch(&root, RecursiveMode::Recursive)
            .map_err(io)?;
        Ok(Self {
            root,
            _debouncer: debouncer,
        })
    }

    /// The canonical folder being watched.
    pub fn root(&self) -> &Path {
        &self.root
    }
}
