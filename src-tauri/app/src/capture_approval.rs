//! The app's answer to "may this agent capture the screen / the camera?" (see
//! `tools::CaptureApprover`). Asked once per (agent, kind) in a native dialog
//! ([`crate::approvals`]) and saved in `<userData>/capture-approvals.json`. The OS-level screen
//! recording or camera permission is the app's, granted once for every agent, so this is what
//! keeps an agent's tools from looking at the screen without the user knowing.

use std::future::Future;
use std::pin::Pin;

use tauri::AppHandle;
use tools::{CaptureApprover, CaptureKind, CaptureRequest};

use crate::approvals::{approval_key, ApprovalStore, Question};

const FILE_NAME: &str = "capture-approvals.json";

pub struct CaptureApprovals(ApprovalStore);

impl CaptureApprovals {
    pub fn new(app: AppHandle, user_data: &std::path::Path) -> Self {
        Self(ApprovalStore::open(app, user_data, FILE_NAME))
    }
}

fn kind_key(kind: CaptureKind) -> &'static str {
    match kind {
        CaptureKind::Screen => "screen",
        CaptureKind::Camera => "camera",
    }
}

impl CaptureApprover for CaptureApprovals {
    fn approve<'a>(
        &'a self,
        request: &'a CaptureRequest,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let key = approval_key(&["capture", kind_key(request.kind), &request.agent_id]);
            let allowed = self
                .0
                .check(&key, true, || Question {
                    title: match request.kind {
                        CaptureKind::Screen => "Let the agent see your screen?".into(),
                        CaptureKind::Camera => "Let the agent use your camera?".into(),
                    },
                    text: dialog_text(request),
                })
                .await;
            tracing::info!(agent = %request.agent_id, kind = kind_key(request.kind), allowed, "Capture decision");
            allowed
        })
    }
}

fn dialog_text(request: &CaptureRequest) -> String {
    let agent = request
        .agent_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .unwrap_or(&request.agent_id);
    let what = match request.kind {
        CaptureKind::Screen => "take screenshots of your screen or of a window",
        CaptureKind::Camera => "take pictures with your camera",
    };
    format!(
        "The agent \"{agent}\" wants to {what}. The pictures are sent to the model and may be \
         described in the chat, so anything visible (passwords, other chats, messages) can end up \
         there.\n\nIf you allow it, this agent may do this from now on without asking. Background \
         agents and sub-agents can never capture."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_names_the_agent_and_what_it_captures() {
        let screen = CaptureRequest {
            agent_id: "a1".into(),
            agent_name: Some("Helper".into()),
            kind: CaptureKind::Screen,
        };
        let text = dialog_text(&screen);
        assert!(
            text.contains("\"Helper\"") && text.contains("screenshots"),
            "{text}"
        );
        let camera = CaptureRequest {
            kind: CaptureKind::Camera,
            agent_name: None,
            ..screen
        };
        let text = dialog_text(&camera);
        assert!(text.contains("\"a1\"") && text.contains("camera"), "{text}");
    }
}
