//! The app's answer to "may this agent run commands matching this allowlist pattern on the
//! user's computer?" (see `tools::HostCommandApprover`). Asked once per (agent, pattern) in a
//! native dialog ([`crate::approvals`]) and saved in `<userData>/host-command-approvals.json`.
//! Unattended runs (background agents, sub-agents) use what was approved and never ask.

use std::future::Future;
use std::pin::Pin;

use tauri::AppHandle;
use tools::{HostCommandApprover, HostCommandRequest};

use crate::approvals::{approval_key, ApprovalStore, Question};

const FILE_NAME: &str = "host-command-approvals.json";

pub struct HostCommandApprovals(ApprovalStore);

impl HostCommandApprovals {
    pub fn new(app: AppHandle, user_data: &std::path::Path) -> Self {
        Self(ApprovalStore::open(app, user_data, FILE_NAME))
    }
}

impl HostCommandApprover for HostCommandApprovals {
    fn approve<'a>(
        &'a self,
        request: &'a HostCommandRequest,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let key = approval_key(&["host-command", &request.agent_id, &request.pattern]);
            let allowed = self
                .0
                .check(&key, request.may_ask, || Question {
                    title: "Run commands on your computer?".into(),
                    text: dialog_text(request),
                })
                .await;
            tracing::info!(
                agent = %request.agent_id,
                pattern = %request.pattern,
                allowed,
                asked = request.may_ask,
                "Host command pattern decision"
            );
            allowed
        })
    }
}

fn dialog_text(request: &HostCommandRequest) -> String {
    let agent = request
        .agent_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or(&request.agent_id);
    format!(
        "The agent \"{agent}\" wants to run this command on your computer, as you:\n\n{}\n\nin \
         {}\n\nIt is allowed by the agent's Allowed Commands pattern \"{}\". If you allow it, \
         this agent may run any command matching that pattern from now on without asking. Only \
         allow it if you added this pattern yourself, or you trust where this agent came from.",
        request.command, request.cwd, request.pattern
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_names_the_agent_command_and_pattern() {
        let request = HostCommandRequest {
            agent_id: "a1".into(),
            agent_name: Some("Reviewer".into()),
            pattern: "git *".into(),
            command: "git status".into(),
            cwd: "/p".into(),
            may_ask: true,
        };
        let text = dialog_text(&request);
        assert!(text.contains("\"Reviewer\""), "{text}");
        assert!(text.contains("\n\ngit status\n\n"), "{text}");
        assert!(text.contains("pattern \"git *\""), "{text}");
        let unnamed = HostCommandRequest {
            agent_name: None,
            ..request
        };
        assert!(dialog_text(&unnamed).contains("\"a1\""));
    }
}
