//! Port of `src/main/mcp/command-resolver.ts`.

use std::path::{Path, PathBuf};

/// Candidate locations checked for a bare command name, in order.
pub fn command_candidates(command: &str, cwd: &Path, home: &str) -> Vec<PathBuf> {
    let bin = cwd.join("node_modules").join(".bin");
    vec![
        // Node.js
        bin.join(command),
        bin.join(format!("{command}.cmd")),
        bin.join(format!("{command}.ps1")),
        // system
        PathBuf::from(format!("/usr/local/bin/{command}")),
        PathBuf::from(format!("/opt/homebrew/bin/{command}")),
        PathBuf::from(format!("/usr/bin/{command}")),
        PathBuf::from(format!("/bin/{command}")),
        // Python / pipx
        Path::new(home).join(".local").join("bin").join(command),
        // Windows
        PathBuf::from(format!("C:\\Program Files\\nodejs\\{command}.exe")),
        PathBuf::from(format!("C:\\Windows\\System32\\{command}.exe")),
    ]
}

/// `resolveCommand`: paths are returned unchanged; bare names resolve to the first existing
/// well-known location, else stay as-is for a `PATH` lookup.
pub fn resolve_command(command: &str) -> String {
    if command.contains('/') || command.contains('\\') {
        return command.to_string();
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    command_candidates(command, &cwd, &home)
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| command.to_string())
}

/// Where `command` would be found by a `PATH` search: `path` split on `sep`, trying the bare name
/// and then each of `exts` appended (Windows' `PATHEXT`). A command that already names a path is
/// checked as it is, with the same extensions. `None` when nothing exists.
///
/// On Windows MCP servers are started through `cmd /c`, and `cmd` itself always starts, so a missing
/// command never surfaces as "not found"; this lookup reports it up front, the way Node's
/// cross-spawn turns `cmd`'s exit into `ENOENT`.
pub fn find_in_path(command: &str, path: &str, sep: char, exts: &[String]) -> Option<PathBuf> {
    let candidates = |base: PathBuf| {
        std::iter::once(base.clone()).chain(exts.iter().map(move |ext| {
            let mut name = base.clone().into_os_string();
            name.push(ext);
            PathBuf::from(name)
        }))
    };
    if command.contains('/') || command.contains('\\') {
        return candidates(PathBuf::from(command)).find(|p| p.is_file());
    }
    path.split(sep)
        .filter(|dir| !dir.is_empty())
        .flat_map(|dir| candidates(Path::new(dir).join(command)))
        .find(|p| p.is_file())
}

/// Windows' `PATHEXT` as a list, or its usual default when unset.
pub fn path_extensions(pathext: Option<&str>) -> Vec<String> {
    pathext
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(".COM;.EXE;.BAT;.CMD")
        .split(';')
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_commands_with_or_without_an_extension() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::write(bin.join("npx.cmd"), "").unwrap();
        std::fs::write(bin.join("plain"), "").unwrap();
        let path = format!(";{};", bin.display());
        let exts = path_extensions(Some(".exe;.cmd"));
        assert_eq!(
            find_in_path("npx", &path, ';', &exts),
            Some(bin.join("npx.cmd"))
        );
        assert_eq!(
            find_in_path("plain", &path, ';', &exts),
            Some(bin.join("plain"))
        );
        assert_eq!(find_in_path("missing", &path, ';', &exts), None);
        // Directories are not commands.
        std::fs::create_dir(bin.join("dir")).unwrap();
        assert_eq!(find_in_path("dir", &path, ';', &exts), None);
        // A path is checked as it is.
        let full = bin.join("npx").to_string_lossy().into_owned();
        assert_eq!(
            find_in_path(&full, "", ';', &exts),
            Some(bin.join("npx.cmd"))
        );
    }

    #[test]
    fn pathext_defaults_when_unset() {
        assert_eq!(path_extensions(None), [".COM", ".EXE", ".BAT", ".CMD"]);
        assert_eq!(path_extensions(Some(" ")), [".COM", ".EXE", ".BAT", ".CMD"]);
        assert_eq!(path_extensions(Some(".EXE;.PS1")), [".EXE", ".PS1"]);
    }

    #[test]
    fn paths_are_left_alone() {
        assert_eq!(resolve_command("./server.js"), "./server.js");
        assert_eq!(resolve_command("C:\\x\\y.exe"), "C:\\x\\y.exe");
    }

    #[test]
    fn unknown_commands_fall_back_to_path_lookup() {
        assert_eq!(
            resolve_command("definitely-not-a-real-command-xyz"),
            "definitely-not-a-real-command-xyz"
        );
    }

    #[test]
    fn prefers_local_node_modules_bin() {
        let dir = tempfile::TempDir::new().unwrap();
        let bin = dir.path().join("node_modules").join(".bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("tool"), "").unwrap();
        let first = command_candidates("tool", dir.path(), "/home/x")
            .into_iter()
            .find(|p| p.exists())
            .unwrap();
        assert_eq!(first, bin.join("tool"));
    }
}
