//! Which shell to start and where.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// A program to run on the PTY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
    pub program: OsString,
    pub args: Vec<OsString>,
    /// Set on top of the app's own environment, which the shell inherits.
    pub env: Vec<(OsString, OsString)>,
    /// Shown on the terminal's tab, e.g. `zsh`.
    pub display_name: String,
}

impl ShellCommand {
    /// `program args...` with the terminal variables set and `display_name` taken from the
    /// program's file name.
    pub fn new(program: impl Into<OsString>, args: &[&str]) -> Self {
        let program = program.into();
        Self {
            display_name: display_name(&program),
            program,
            args: args.iter().map(OsString::from).collect(),
            env: terminal_env(),
        }
    }
}

/// `zsh` for `/bin/zsh`, `powershell` for `C:\...\powershell.exe`.
pub fn display_name(program: &OsStr) -> String {
    // `Path` only splits on `\` on Windows; split by hand so the name is right everywhere.
    let text = program.to_string_lossy();
    let base = text.rsplit(['/', '\\']).next().unwrap_or(&text);
    let stem = Path::new(base)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| base.to_string());
    if stem.is_empty() {
        "shell".to_string()
    } else {
        stem
    }
}

fn terminal_env() -> Vec<(OsString, OsString)> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut env: Vec<(OsString, OsString)> = [
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
        ("TERM_PROGRAM", "BedrockEngineer"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect();
    // Apps started from the macOS Dock or Finder get no locale, which leaves shells in the C
    // locale and mangles non-ASCII input. Terminal emulators set one; so do we, if nothing has.
    #[cfg(unix)]
    if ["LANG", "LC_ALL", "LC_CTYPE"]
        .iter()
        .all(|k| std::env::var_os(k).is_none_or(|v| v.is_empty()))
    {
        env.push(("LANG".into(), "en_US.UTF-8".into()));
    }
    env
}

/// The user's shell as a login shell (`$SHELL -l`), or PowerShell on Windows.
pub fn default_shell() -> ShellCommand {
    #[cfg(unix)]
    {
        let program = unix_shell(std::env::var_os("SHELL").as_deref(), &|p| p.is_file());
        ShellCommand::new(program, &["-l"])
    }
    #[cfg(windows)]
    {
        let program = windows_shell(
            std::env::var_os("SystemRoot").as_deref(),
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("COMSPEC").as_deref(),
            &|p| p.is_file(),
        );
        let args: &[&str] = if display_name(&program) == "powershell" {
            &["-NoLogo"]
        } else {
            &[]
        };
        ShellCommand::new(program, args)
    }
}

/// `$SHELL` when it names an existing absolute path, else the platform default.
#[cfg(unix)]
pub fn unix_shell(shell_var: Option<&OsStr>, is_file: &dyn Fn(&Path) -> bool) -> OsString {
    if let Some(shell) = shell_var.filter(|s| !s.is_empty()) {
        let path = Path::new(shell);
        if path.is_absolute() && is_file(path) {
            return shell.to_os_string();
        }
    }
    let preferred = if cfg!(target_os = "macos") {
        "/bin/zsh"
    } else {
        "/bin/bash"
    };
    if is_file(Path::new(preferred)) {
        preferred.into()
    } else {
        "/bin/sh".into()
    }
}

/// Windows PowerShell from `%SystemRoot%` or `PATH`, else `%COMSPEC%`, else `cmd.exe`.
#[cfg(any(windows, test))]
pub fn windows_shell(
    system_root: Option<&OsStr>,
    path_var: Option<&OsStr>,
    comspec: Option<&OsStr>,
    is_file: &dyn Fn(&Path) -> bool,
) -> OsString {
    if let Some(root) = system_root.filter(|s| !s.is_empty()) {
        let candidate = PathBuf::from(root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        if is_file(&candidate) {
            return candidate.into_os_string();
        }
    }
    if let Some(path_var) = path_var {
        for dir in path_var
            .to_string_lossy()
            .split(';')
            .filter(|d| !d.is_empty())
        {
            let candidate = PathBuf::from(dir).join("powershell.exe");
            if is_file(&candidate) {
                return candidate.into_os_string();
            }
        }
    }
    match comspec.filter(|s| !s.is_empty()) {
        Some(comspec) => comspec.to_os_string(),
        None => "cmd.exe".into(),
    }
}

/// The project folder when it is an existing absolute directory, else the home directory.
pub fn resolve_cwd(project_path: Option<&Path>) -> PathBuf {
    if let Some(path) = project_path.filter(|p| p.is_absolute() && p.is_dir()) {
        return path.to_path_buf();
    }
    dirs::home_dir()
        .filter(|h| h.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names() {
        assert_eq!(display_name(OsStr::new("/bin/zsh")), "zsh");
        assert_eq!(
            display_name(OsStr::new(
                r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
            )),
            "powershell"
        );
        assert_eq!(display_name(OsStr::new("cmd.exe")), "cmd");
        assert_eq!(display_name(OsStr::new("")), "shell");
    }

    #[test]
    fn sets_the_terminal_variables() {
        let shell = ShellCommand::new("/bin/sh", &["-l"]);
        let get = |k: &str| {
            shell
                .env
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.to_string_lossy().into_owned())
        };
        assert_eq!(get("TERM").as_deref(), Some("xterm-256color"));
        assert_eq!(get("COLORTERM").as_deref(), Some("truecolor"));
        assert_eq!(get("TERM_PROGRAM").as_deref(), Some("BedrockEngineer"));
        assert_eq!(shell.args, vec![OsString::from("-l")]);
    }

    #[cfg(unix)]
    #[test]
    fn unix_shell_prefers_an_existing_absolute_shell_var() {
        let all = |_: &Path| true;
        let none = |_: &Path| false;
        assert_eq!(
            unix_shell(Some(OsStr::new("/usr/bin/fish")), &all),
            "/usr/bin/fish"
        );
        // Relative, empty or missing $SHELL: the platform default.
        let default = if cfg!(target_os = "macos") {
            "/bin/zsh"
        } else {
            "/bin/bash"
        };
        assert_eq!(unix_shell(Some(OsStr::new("fish")), &all), default);
        assert_eq!(unix_shell(Some(OsStr::new("")), &all), default);
        assert_eq!(unix_shell(None, &all), default);
        assert_eq!(unix_shell(Some(OsStr::new("/nope/zsh")), &none), "/bin/sh");
    }

    #[test]
    fn windows_shell_falls_back_from_powershell_to_comspec() {
        let only = |want: &'static str| move |p: &Path| p.to_string_lossy().ends_with(want);
        let ps = windows_shell(
            Some(OsStr::new("C:/Windows")),
            None,
            Some(OsStr::new("C:/Windows/system32/cmd.exe")),
            &only("powershell.exe"),
        );
        assert_eq!(display_name(&ps), "powershell");
        let from_path = windows_shell(None, Some(OsStr::new("C:/a;C:/ps")), None, &|p: &Path| {
            p.starts_with("C:/ps")
        });
        assert_eq!(
            PathBuf::from(from_path),
            PathBuf::from("C:/ps").join("powershell.exe")
        );
        let cmd = windows_shell(
            Some(OsStr::new("C:/Windows")),
            None,
            Some(OsStr::new("C:/Windows/system32/cmd.exe")),
            &|_: &Path| false,
        );
        assert_eq!(cmd, "C:/Windows/system32/cmd.exe");
        assert_eq!(
            windows_shell(None, None, None, &|_: &Path| false),
            "cmd.exe"
        );
    }

    #[test]
    fn cwd_falls_back_to_home_when_the_project_folder_is_unusable() {
        let dir = std::env::temp_dir();
        assert_eq!(resolve_cwd(Some(&dir)), dir);
        let home = resolve_cwd(None);
        assert!(home.is_absolute() && home.is_dir());
        assert_eq!(resolve_cwd(Some(Path::new("/definitely/not/here"))), home);
        assert_eq!(resolve_cwd(Some(Path::new("relative/dir"))), home);
    }
}
