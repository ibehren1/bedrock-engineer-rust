//! Port of the `save-website-content` IPC handler (`src/main/handlers/file-handlers.ts`).

use crate::context::ToolSettings;
use crate::util::node_io::NodeIoError;
use crate::util::path_guard::Access;
use std::path::{Path, PathBuf};

/// `format: 'html' | 'txt'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    Html,
    Txt,
}

impl SaveFormat {
    fn extension(self) -> &'static str {
        match self {
            SaveFormat::Html => ".html",
            SaveFormat::Txt => ".txt",
        }
    }
}

/// `{ success, filePath }` / `{ success: false, error }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveResult {
    Saved(String),
    Failed(String),
}

/// `new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)`.
fn timestamp() -> String {
    crate::util::js::iso_now()
        .replace([':', '.'], "-")
        .chars()
        .take(19)
        .collect()
}

/// Save fetched content under `directory` (or `<projectPath>/downloads`), never
/// overwriting: `name.ext`, then `name_1.ext`, `name_2.ext`, ...
pub async fn save_website_content(
    content: &str,
    url: &str,
    filename: Option<&str>,
    directory: Option<&str>,
    format: SaveFormat,
    settings: &ToolSettings,
) -> SaveResult {
    let result: Result<String, String> = async {
        // Confined like the file tools (TOOLS-8): relative directories land in the project.
        let target_dir: PathBuf = settings.confine(
            directory.filter(|d| !d.is_empty()).unwrap_or("downloads"),
            Access::Write,
        )?;
        if let Some(name) = filename.filter(|f| !f.is_empty()) {
            if name.contains(['/', '\\', '\0']) || name == "." || name == ".." {
                return Err(format!(
                    "Invalid filename \"{name}\": use a plain file name, and the directory option for the folder."
                ));
            }
        }
        let dir_str = target_dir.to_string_lossy().into_owned();
        tokio::fs::create_dir_all(&target_dir)
            .await
            .map_err(|e| NodeIoError::new(&e, "mkdir", &dir_str, None).message)?;

        let ext = format.extension();
        let final_name = match filename.filter(|f| !f.is_empty()) {
            Some(f) if f.ends_with(ext) => f.to_string(),
            Some(f) => format!("{f}{ext}"),
            None => match url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
            {
                Some(host) => {
                    let domain = host.strip_prefix("www.").unwrap_or(&host).to_string();
                    format!("{domain}_{}{ext}", timestamp())
                }
                None => format!("website_{}{ext}", timestamp()),
            },
        };

        let mut final_path = target_dir.join(&final_name);
        let (stem, extension) = {
            let p = Path::new(&final_name);
            let extension = p
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let stem = final_name
                .strip_suffix(&extension)
                .unwrap_or(&final_name)
                .to_string();
            (stem, extension)
        };
        // `create_new`, so a file that appears between the check and the write is never
        // overwritten: each taken name moves on to the next counter.
        let mut counter = 1;
        loop {
            let opened = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&final_path)
                .await;
            match opened {
                Ok(mut file) => {
                    use tokio::io::AsyncWriteExt;
                    let path_str = final_path.to_string_lossy().into_owned();
                    file.write_all(content.as_bytes())
                        .await
                        .map_err(|e| NodeIoError::new(&e, "write", &path_str, None).message)?;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && counter < 10_000 => {
                    final_path = target_dir.join(format!("{stem}_{counter}{extension}"));
                    counter += 1;
                }
                Err(e) => {
                    let path_str = final_path.to_string_lossy().into_owned();
                    return Err(NodeIoError::new(&e, "open", &path_str, None).message);
                }
            }
        }
        let path_str = final_path.to_string_lossy().into_owned();
        Ok(path_str)
    }
    .await;
    match result {
        Ok(p) => SaveResult::Saved(p),
        Err(e) => SaveResult::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_settings() -> ToolSettings {
        ToolSettings {
            agent_file_folders: vec![std::env::temp_dir().to_string_lossy().into_owned()],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn refuses_traversal_and_outside_directories() {
        let project = tempfile::tempdir().unwrap();
        let settings = ToolSettings {
            project_path: Some(project.path().to_string_lossy().into_owned()),
            ..Default::default()
        };
        for (filename, directory) in [
            (Some("../../escape"), None),
            (Some("a/b"), None),
            (None, Some("/etc")),
            (None, Some("../outside")),
        ] {
            let r = save_website_content(
                "x",
                "https://x.com",
                filename,
                directory,
                SaveFormat::Html,
                &settings,
            )
            .await;
            assert!(
                matches!(r, SaveResult::Failed(_)),
                "{filename:?} {directory:?}: {r:?}"
            );
        }
    }

    #[tokio::test]
    async fn saves_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_string_lossy().into_owned();
        let a = save_website_content(
            "one",
            "https://x.com",
            Some("page"),
            Some(&d),
            SaveFormat::Html,
            &open_settings(),
        )
        .await;
        let b = save_website_content(
            "two",
            "https://x.com",
            Some("page.html"),
            Some(&d),
            SaveFormat::Html,
            &open_settings(),
        )
        .await;
        let SaveResult::Saved(a) = a else { panic!() };
        let SaveResult::Saved(b) = b else { panic!() };
        assert!(a.ends_with("page.html"));
        assert!(b.ends_with("page_1.html"));
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "two");
    }

    #[tokio::test]
    async fn default_name_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().into_owned();
        let r = save_website_content(
            "x",
            "https://www.example.com/a",
            None,
            None,
            SaveFormat::Txt,
            &ToolSettings {
                project_path: Some(p.clone()),
                ..Default::default()
            },
        )
        .await;
        let SaveResult::Saved(path) = r else { panic!() };
        let name = Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            Path::new(&path).parent(),
            Some(dir.path().join("downloads").as_path()),
            "{path}"
        );
        assert!(
            name.starts_with("example.com_") && name.ends_with(".txt"),
            "{name}"
        );
        assert_eq!(name.len(), "example.com_".len() + 19 + 4);
    }
}
