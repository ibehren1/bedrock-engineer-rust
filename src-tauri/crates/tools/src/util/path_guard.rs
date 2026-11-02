//! Where the model-facing file tools may read and write (TOOLS-2).
//!
//! A path is usable when it resolves, after following symlinks, inside the project folder
//! (`projectPath`) or one of the extra folders the user listed in Settings → Workspace
//! (`agentFileFolders`). Relative paths resolve against the project folder. Inside those
//! folders a few places are refused anyway: the app's own data folder (settings with AWS keys,
//! chat history, the approval files) and the usual credential folders in the home directory, so
//! adding a broad folder such as the home directory doesn't hand those over.

use std::path::{Component, Path, PathBuf};

use crate::context::ToolSettings;

/// What a tool is about to do with a path (only used in messages today; reads and writes are
/// confined the same way).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// Home-relative places never given to a tool, even inside an allowed folder.
const SECRET_PLACES: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".kube",
    ".azure",
    ".docker/config.json",
    ".config/gcloud",
    ".config/gh",
    ".netrc",
    ".git-credentials",
    ".npmrc",
    ".pypirc",
    "Library/Keychains",
];

/// `.` and `..` removed without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path` with symlinks resolved as far as it exists; the missing tail is appended as is.
fn canonicalize_lenient(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut tail = Vec::new();
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            let mut out = canonical;
            out.extend(tail.iter().rev());
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

impl ToolSettings {
    /// The project folder followed by the extra folders, as configured (absolute ones only).
    pub fn agent_folders(&self) -> Vec<PathBuf> {
        self.project_path
            .iter()
            .chain(self.agent_file_folders.iter())
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .collect()
    }

    /// `path` as an absolute path the tools may use, or why not.
    pub fn confine(&self, path: &str, access: Access) -> Result<PathBuf, String> {
        let roots = self.agent_folders();
        let verb = match access {
            Access::Read => "read",
            Access::Write => "write to",
        };
        let outside = || {
            format!(
                "Access denied: cannot {verb} {path}. Agents can only use files in the project folder{} and the folders listed under Settings → Workspace → Agent folders.",
                self.project_path
                    .as_deref()
                    .map(|p| format!(" ({p})"))
                    .unwrap_or_default()
            )
        };
        if path.is_empty() || path.contains('\0') {
            return Err(outside());
        }
        let given = Path::new(path);
        let absolute = if given.is_absolute() {
            given.to_path_buf()
        } else {
            match self.project_path.as_deref().filter(|p| !p.is_empty()) {
                Some(project) => Path::new(project).join(given),
                None => {
                    return Err(format!(
                        "Access denied: {path} is a relative path and no project folder is set. Choose a project folder in Settings, or use an absolute path inside one of the agent folders."
                    ))
                }
            }
        };
        let lexical = normalize(&absolute);
        let canonical = canonicalize_lenient(&lexical);
        let inside = |target: &Path, canonical_roots: bool| {
            roots.iter().any(|root| {
                let root = normalize(root);
                let root = if canonical_roots {
                    canonicalize_lenient(&root)
                } else {
                    root
                };
                target.starts_with(&root)
            })
        };
        // Both checks: the lexical one catches `..`, the canonical one symlinks that leave.
        if !inside(&lexical, false) || !inside(&canonical, true) {
            return Err(outside());
        }

        let mut refused: Vec<PathBuf> = Vec::new();
        if let Some(user_data) = self.user_data_path.as_deref().filter(|p| !p.is_empty()) {
            refused.push(PathBuf::from(user_data));
        }
        if let Some(home) = home_dir() {
            refused.extend(SECRET_PLACES.iter().map(|p| home.join(p)));
        }
        for place in refused {
            let place = canonicalize_lenient(&normalize(&place));
            if canonical.starts_with(&place) {
                return Err(format!(
                    "Access denied: cannot {verb} {path}. The app's data folder and credential folders such as ~/.ssh and ~/.aws are never available to agents."
                ));
            }
        }
        // The path as given (made absolute), so messages show what the model asked for; the
        // checks above already followed its symlinks.
        Ok(lexical)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn settings(project: &Path, extra: &[&Path], user_data: Option<&Path>) -> ToolSettings {
        ToolSettings {
            project_path: Some(project.to_string_lossy().into_owned()),
            agent_file_folders: extra
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            user_data_path: user_data.map(|p| p.to_string_lossy().into_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn project_and_extra_folders_only() {
        let project = tempfile::tempdir().unwrap();
        let extra = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let s = settings(project.path(), &[extra.path()], None);
        let p = |path: &Path| path.to_string_lossy().into_owned();

        assert!(s
            .confine(&p(&project.path().join("a.txt")), Access::Write)
            .is_ok());
        assert!(s.confine("src/new/file.rs", Access::Write).is_ok());
        assert!(s.confine(&p(&extra.path().join("x")), Access::Read).is_ok());
        let err = s
            .confine(&p(&other.path().join("x")), Access::Read)
            .unwrap_err();
        assert!(err.starts_with("Access denied"), "{err}");
        assert!(s.confine("../escape.txt", Access::Write).is_err());
        assert!(s.confine("/etc/passwd", Access::Read).is_err());
        assert!(s.confine("", Access::Read).is_err());
    }

    #[test]
    fn symlinks_leaving_the_folders_are_refused() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), project.path().join("link")).unwrap();
        let s = settings(project.path(), &[], None);
        assert!(s.confine("link/secret", Access::Read).is_err());
        assert!(s.confine("link", Access::Write).is_err());
    }

    #[test]
    fn app_data_and_credentials_are_refused_even_inside_an_allowed_folder() {
        let home = tempfile::tempdir().unwrap();
        let user_data = home.path().join("appdata");
        std::fs::create_dir(&user_data).unwrap();
        let s = settings(home.path(), &[], Some(&user_data));
        let err = s.confine("appdata/config.json", Access::Read).unwrap_err();
        assert!(err.contains("never available"), "{err}");
        // Credential folders are under the real home directory.
        if let Some(real_home) = home_dir() {
            let s = settings(&real_home, &[], None);
            assert!(s
                .confine(
                    &real_home.join(".ssh/id_ed25519").to_string_lossy(),
                    Access::Read
                )
                .unwrap_err()
                .contains("never available"));
            assert!(s
                .confine(
                    &real_home.join(".aws/credentials").to_string_lossy(),
                    Access::Read
                )
                .is_err());
        }
    }

    #[test]
    fn relative_paths_need_a_project() {
        let s = ToolSettings::default();
        assert!(s
            .confine("a.txt", Access::Read)
            .unwrap_err()
            .contains("no project folder is set"));
    }
}
