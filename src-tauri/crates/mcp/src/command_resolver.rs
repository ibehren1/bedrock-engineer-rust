//! Port of `src/main/mcp/command-resolver.ts`.

use std::path::{Path, PathBuf};

/// Candidate locations checked for a bare command name, in order.
///
/// There is deliberately no `<cwd>/node_modules/.bin` entry (the upstream TS checked one): the
/// app's working directory is whatever it was launched from, so a planted `npx` there would
/// hijack every stdio server.
pub fn command_candidates(command: &str, home: &str) -> Vec<PathBuf> {
    vec![
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

/// A regular file the current user may execute (on Windows: any regular file).
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// `resolveCommand`: absolute paths are returned unchanged; bare names resolve to the first
/// executable file in a well-known location, else stay as-is for a `PATH` lookup. Relative paths
/// (`./server`, `bin/server`) are refused: they would resolve against the app's own working
/// directory, which the user doesn't choose.
pub fn resolve_command(command: &str) -> Result<String, String> {
    if command.contains('/') || command.contains('\\') {
        if !Path::new(command).is_absolute() {
            return Err(format!(
                "MCP server command \"{command}\" is a relative path; use an absolute path or a command name found on PATH"
            ));
        }
        return Ok(command.to_string());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    Ok(command_candidates(command, &home)
        .into_iter()
        .find(|p| is_executable_file(p))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| command.to_string()))
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
    fn absolute_paths_are_left_alone() {
        let abs = if cfg!(windows) { "C:\\x\\y.exe" } else { "/opt/x/server" };
        assert_eq!(resolve_command(abs).unwrap(), abs);
    }

    #[test]
    fn relative_paths_are_refused() {
        for command in ["./server.js", "bin/server", "..\\x\\y.exe"] {
            let err = resolve_command(command).unwrap_err();
            assert!(err.contains("relative path"), "{command}: {err}");
        }
    }

    #[test]
    fn unknown_commands_fall_back_to_path_lookup() {
        assert_eq!(
            resolve_command("definitely-not-a-real-command-xyz").unwrap(),
            "definitely-not-a-real-command-xyz"
        );
    }

    // A `node_modules/.bin` in the app's working directory must never be picked up.
    #[test]
    fn ignores_node_modules_bin_in_the_working_directory() {
        let candidates = command_candidates("tool", "/home/x");
        assert!(candidates
            .iter()
            .all(|p| !p.to_string_lossy().contains("node_modules")));
    }

    #[cfg(unix)]
    #[test]
    fn only_executable_regular_files_match() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain");
        std::fs::write(&plain, "").unwrap();
        assert!(!is_executable_file(&plain));
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable_file(&plain));
        assert!(!is_executable_file(dir.path()));
    }
}
