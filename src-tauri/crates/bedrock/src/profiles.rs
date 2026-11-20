//! The named AWS profiles on this machine, for the profile picker in Settings → AWS Authentication.
//!
//! Reads the shared config file (`$AWS_CONFIG_FILE`, else `~/.aws/config`) and the shared
//! credentials file (`$AWS_SHARED_CREDENTIALS_FILE`, else `~/.aws/credentials`) and returns the
//! profile names only — never anything else from the files. In the config file a profile is
//! `[default]` or `[profile name]`; other sections (`[sso-session …]`, `[services …]`, …) are
//! skipped. In the credentials file every section is a profile. A missing or unreadable file
//! contributes nothing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DEFAULT: &str = "default";

/// The profile names in the shared config and credentials files: deduplicated, sorted, with
/// `default` first.
pub fn list_profiles() -> Vec<String> {
    let home = home_dir();
    let config = shared_file_path("AWS_CONFIG_FILE", &["config"], home.as_deref());
    let credentials = shared_file_path(
        "AWS_SHARED_CREDENTIALS_FILE",
        &["credentials"],
        home.as_deref(),
    );
    let read = |p: Option<PathBuf>| p.and_then(|p| std::fs::read_to_string(p).ok());
    profiles_from(read(config).as_deref(), read(credentials).as_deref())
}

/// [`list_profiles`] over file contents (`None` for a file that couldn't be read).
pub fn profiles_from(config: Option<&str>, credentials: Option<&str>) -> Vec<String> {
    let mut names = BTreeSet::new();
    for section in config.map(sections).into_iter().flatten() {
        if let Some(name) = config_profile_name(section) {
            names.insert(name.to_string());
        }
    }
    for section in credentials.map(sections).into_iter().flatten() {
        names.insert(section.to_string());
    }
    let has_default = names.remove(DEFAULT);
    has_default
        .then(|| DEFAULT.to_string())
        .into_iter()
        .chain(names)
        .collect()
}

/// The trimmed, non-empty `[section]` names of an INI-style file, in order.
fn sections(contents: &str) -> impl Iterator<Item = &str> {
    contents.lines().filter_map(|line| {
        let rest = line.trim().strip_prefix('[')?;
        let name = rest[..rest.find(']')?].trim();
        (!name.is_empty()).then_some(name)
    })
}

/// A config-file section's profile name: `default`, or `x` for `profile x`.
fn config_profile_name(section: &str) -> Option<&str> {
    if section == DEFAULT {
        return Some(DEFAULT);
    }
    let rest = section.strip_prefix("profile")?;
    // `profile` must be followed by whitespace (`[profilex]` isn't a profile).
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let name = rest.trim();
    (!name.is_empty() && !name.contains(char::is_whitespace)).then_some(name)
}

/// `$var` (with a leading `~` expanded) when set, else `~/.aws/<parts>`.
fn shared_file_path(var: &str, parts: &[&str], home: Option<&Path>) -> Option<PathBuf> {
    if let Some(value) = std::env::var_os(var).filter(|v| !v.is_empty()) {
        return Some(expand_home(PathBuf::from(value), home));
    }
    let mut path = home?.join(".aws");
    path.extend(parts);
    Some(path)
}

fn expand_home(path: PathBuf, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix("~"), home) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path,
    }
}

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "\
# comment [not-a-section]
[default]
region = us-east-1

[profile work]
sso_session = corp
[ profile  spaced ]
[profile default]
[sso-session corp]
sso_start_url = https://example.awsapps.com/start
[services local]
bedrock-runtime =
  endpoint_url = http://localhost
[profilex]
[profile]
[profile two words]
[bare]
[profile dev] ; trailing comment
";

    const CREDENTIALS: &str = "\
[default]
aws_access_key_id = AKIDEXAMPLE
aws_secret_access_key = secret
[ci]
aws_access_key_id = AKID2
[work]
[]
";

    #[test]
    fn collects_profiles_from_both_files() {
        assert_eq!(
            profiles_from(Some(CONFIG), Some(CREDENTIALS)),
            vec!["default", "ci", "dev", "spaced", "work"]
        );
    }

    #[test]
    fn missing_files_contribute_nothing() {
        assert!(profiles_from(None, None).is_empty());
        assert_eq!(profiles_from(None, Some("[b]\n[a]\n")), vec!["a", "b"]);
        assert_eq!(
            profiles_from(Some("[profile z]\n[default]\n"), None),
            vec!["default", "z"]
        );
        assert!(profiles_from(Some(""), Some("no sections here")).is_empty());
    }

    #[test]
    fn never_returns_file_contents() {
        let out = profiles_from(Some(CONFIG), Some(CREDENTIALS)).join(",");
        assert!(!out.contains("AKID") && !out.contains("secret") && !out.contains("http"));
    }

    #[test]
    fn reads_files_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, "[profile disk]\n").unwrap();
        let read = |p: &Path| std::fs::read_to_string(p).ok();
        assert_eq!(
            profiles_from(
                read(&config).as_deref(),
                read(&dir.path().join("missing")).as_deref()
            ),
            vec!["disk"]
        );
    }

    #[test]
    fn default_paths_live_under_home_dot_aws() {
        let home = Path::new("/home/u");
        // An env var that is surely unset falls back to ~/.aws.
        assert_eq!(
            shared_file_path("BEDROCK_ENGINEER_TEST_UNSET_VAR", &["config"], Some(home)),
            Some(home.join(".aws").join("config"))
        );
        assert_eq!(
            shared_file_path("BEDROCK_ENGINEER_TEST_UNSET_VAR", &["config"], None),
            None
        );
        assert_eq!(
            expand_home(PathBuf::from("~/custom/config"), Some(home)),
            home.join("custom/config")
        );
        assert_eq!(
            expand_home(PathBuf::from("/etc/aws"), Some(home)),
            PathBuf::from("/etc/aws")
        );
    }
}
