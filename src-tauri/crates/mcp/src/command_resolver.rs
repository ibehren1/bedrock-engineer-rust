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

#[cfg(test)]
mod tests {
    use super::*;

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
