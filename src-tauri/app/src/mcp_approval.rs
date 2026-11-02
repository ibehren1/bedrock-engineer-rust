//! The app's answer to "may this stdio MCP server start?" (see `mcp::launch_approval`): the
//! launch's fingerprint must have been approved by the user in a native dialog
//! ([`crate::approvals`]), saved in `<userData>/mcp-approved-servers.json`.

use std::future::Future;
use std::pin::Pin;

use mcp::{LaunchApprover, LaunchRequest};
use tauri::AppHandle;

use crate::approvals::{ApprovalStore, Question};

const FILE_NAME: &str = "mcp-approved-servers.json";

pub struct McpLaunchApprover(ApprovalStore);

impl McpLaunchApprover {
    pub fn new(app: AppHandle, user_data: &std::path::Path) -> Self {
        Self(ApprovalStore::open(app, user_data, FILE_NAME))
    }
}

impl LaunchApprover for McpLaunchApprover {
    fn approve<'a>(
        &'a self,
        request: &'a LaunchRequest,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let allowed = self
                .0
                .check(&request.fingerprint, true, || Question {
                    title: "Start an MCP server?".into(),
                    text: dialog_text(request),
                })
                .await;
            tracing::info!(
                server = %request.server_name,
                command = %request.command,
                allowed,
                "MCP server launch decision"
            );
            allowed
        })
    }
}

/// What the dialog says: the exact program and arguments, the names of the variables it gets,
/// and a warning when the app's AWS keys go with it.
fn dialog_text(request: &LaunchRequest) -> String {
    let mut text = format!(
        "An agent wants to start the MCP server \"{}\". It runs on this computer as you, with \
         access to your files and network.\n\nOnly allow it if you trust where this agent came \
         from.\n\nProgram: {}",
        request.server_name, request.command
    );
    if !request.args.is_empty() {
        text.push_str(&format!("\nArguments: {}", request.args.join(" ")));
    }
    if !request.env_names.is_empty() {
        text.push_str(&format!(
            "\nEnvironment variables: {}",
            request.env_names.join(", ")
        ));
    }
    if request.receives_aws_keys {
        text.push_str(
            "\n\nThis server will also receive your AWS access key and secret key from the app's \
             settings.",
        );
    }
    text.push_str(
        "\n\nThe app asks again if the program, its arguments or its environment change.",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_shows_the_launch_and_warns_about_aws_keys() {
        let mut request = LaunchRequest {
            server_name: "files".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@x/server".into()],
            env_names: vec!["TOKEN".into()],
            receives_aws_keys: false,
            fingerprint: "f".into(),
        };
        let text = dialog_text(&request);
        assert!(text.contains("\"files\""), "{text}");
        assert!(text.contains("Program: npx"), "{text}");
        assert!(text.contains("Arguments: -y @x/server"), "{text}");
        assert!(text.contains("Environment variables: TOKEN"), "{text}");
        assert!(!text.contains("AWS access key"), "{text}");
        request.receives_aws_keys = true;
        assert!(dialog_text(&request).contains("AWS access key"));
    }
}
