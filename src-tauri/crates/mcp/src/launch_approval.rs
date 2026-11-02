//! Approval before a stdio MCP server is started.
//!
//! A stdio server is a program the app runs on the user's machine, with the user's privileges,
//! and its command, arguments and `env` come from the agent's config. That config is often
//! written by someone else: a cloned repository's `.bedrock-engineer/agents/`, an imported
//! agent file, an organization's agent bucket. So no stdio server starts until the user has
//! approved that exact launch. [`McpManager`](crate::McpManager) asks its [`LaunchApprover`]
//! before every spawn; the app answers from its store of approved fingerprints or asks the
//! user. URL and built-in servers start no process and are not gated here.

use common::agent::McpServerConfig;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::pin::Pin;

/// What is about to be started, as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    pub server_name: String,
    pub command: String,
    pub args: Vec<String>,
    /// Names of the variables set from the server's `env` (values are not shown: they can be
    /// secrets the user typed in).
    pub env_names: Vec<String>,
    /// The app's static AWS access keys will be put into the server's environment
    /// (`{{awsProfile}}` with static credentials, see [`crate::env_placeholders`]).
    pub receives_aws_keys: bool,
    /// [`fingerprint`] of the server config as written.
    pub fingerprint: String,
}

/// Decides whether a stdio MCP server may start.
pub trait LaunchApprover: Send + Sync {
    fn approve<'a>(
        &'a self,
        request: &'a LaunchRequest,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

/// A stable hash of everything that decides what a stdio server runs: the command, its
/// arguments and its `env` as written in the config (placeholders unexpanded, so rotating AWS
/// credentials doesn't ask again, while adding a `{{awsProfile}}` entry does). Changing any of
/// them asks again. The server's name is not part of it.
pub fn fingerprint(server: &McpServerConfig) -> String {
    let mut env: Vec<(&String, &String)> = server.env.iter().flatten().collect();
    env.sort();
    let canonical = serde_json::json!({
        "command": server.command.as_deref().unwrap_or_default(),
        "args": server.args.clone().unwrap_or_default(),
        "env": env,
    });
    let digest = Sha256::digest(canonical.to_string().as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(value: serde_json::Value) -> McpServerConfig {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn fingerprint_covers_command_args_and_env_but_not_name() {
        let base = serde_json::json!({
            "name": "a", "description": "", "command": "npx", "args": ["-y", "pkg"],
            "env": { "B": "2", "A": "1" }
        });
        let fp = fingerprint(&server(base.clone()));
        let mut renamed = base.clone();
        renamed["name"] = "b".into();
        assert_eq!(fingerprint(&server(renamed)), fp);
        // env order doesn't matter
        let mut reordered = base.clone();
        reordered["env"] = serde_json::json!({ "A": "1", "B": "2" });
        assert_eq!(fingerprint(&server(reordered)), fp);
        for (key, value) in [
            ("command", serde_json::json!("uvx")),
            ("args", serde_json::json!(["-y", "other"])),
            (
                "env",
                serde_json::json!({ "A": "1", "B": "2", "C": "{{awsProfile}}" }),
            ),
        ] {
            let mut changed = base.clone();
            changed[key] = value;
            assert_ne!(fingerprint(&server(changed)), fp, "{key}");
        }
    }
}
