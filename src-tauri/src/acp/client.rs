use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use agent_client_protocol::{
    Client, ContentBlock, PermissionOptionId, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
};
use tokio::sync::mpsc;

use super::types::{AgentUpdate, PermissionOptionInfo, PermissionRequest, PlanEntry};

type PermissionResponder = tokio::sync::oneshot::Sender<RequestPermissionOutcome>;

fn normalize_command_preview(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn should_auto_allow_read_lookup(command_preview: Option<&str>) -> bool {
    let Some(preview) = command_preview else {
        return false;
    };

    let normalized = normalize_command_preview(preview);
    let padded = format!(" {normalized} ");

    // Only auto-allow golaunch-cli commands
    if !padded.contains("golaunch-cli") {
        return false;
    }

    // Block known write/mutating operations — everything else is auto-allowed
    let is_write_operation = padded.contains(" add ")
        || padded.contains(" remove ")
        || padded.contains(" delete ")
        || padded.contains(" update ")
        || padded.contains(" import ")
        || padded.contains(" run ");

    !is_write_operation
}

fn pick_auto_allow_option_id(options: &[PermissionOptionInfo]) -> Option<String> {
    options
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case("Allow"))
        .or_else(|| {
            options
                .iter()
                .find(|o| o.name.eq_ignore_ascii_case("Always Allow"))
        })
        .or_else(|| {
            options.iter().find(|o| {
                let kind = o.kind.to_lowercase();
                kind.contains("allow") && !kind.contains("reject") && !kind.contains("deny")
            })
        })
        .map(|o| o.option_id.clone())
}

/// GoLaunch's ACP client handler.
///
/// Receives session notifications and permission requests from the agent subprocess
/// and forwards them as serializable types over channels to the Tauri event system.
pub struct GoLaunchClient {
    update_tx: mpsc::UnboundedSender<AgentUpdate>,
    permission_tx: mpsc::UnboundedSender<PermissionRequest>,
    pending_permissions: Rc<RefCell<HashMap<String, PermissionResponder>>>,
}

impl GoLaunchClient {
    pub fn new(
        update_tx: mpsc::UnboundedSender<AgentUpdate>,
        permission_tx: mpsc::UnboundedSender<PermissionRequest>,
    ) -> Self {
        Self {
            update_tx,
            permission_tx,
            pending_permissions: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn pending_permissions(&self) -> Rc<RefCell<HashMap<String, PermissionResponder>>> {
        self.pending_permissions.clone()
    }
}

#[async_trait::async_trait(?Send)]
impl Client for GoLaunchClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> agent_client_protocol::Result<RequestPermissionResponse> {
        let request_id = args.tool_call.tool_call_id.to_string();

        let tool_name = args
            .tool_call
            .fields
            .title
            .clone()
            .unwrap_or_else(|| "Unknown tool".to_string());

        let command_preview = args.tool_call.fields.content.as_ref().and_then(|content| {
            content.iter().find_map(|c| {
                if let agent_client_protocol::ToolCallContent::Content(inner) = c {
                    if let ContentBlock::Text(t) = &inner.content {
                        return Some(t.text.clone());
                    }
                }
                None
            })
        });

        let options: Vec<PermissionOptionInfo> = args
            .options
            .iter()
            .map(|o| PermissionOptionInfo {
                option_id: o.option_id.to_string(),
                name: o.name.clone(),
                kind: format!("{:?}", o.kind),
            })
            .collect();

        if should_auto_allow_read_lookup(command_preview.as_deref()) {
            if let Some(option_id) = pick_auto_allow_option_id(&options) {
                return Ok(RequestPermissionResponse::new(
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                        PermissionOptionId::new(option_id),
                    )),
                ));
            }
        }

        let (tx, rx) = tokio::sync::oneshot::channel();

        self.pending_permissions
            .borrow_mut()
            .insert(request_id.clone(), tx);

        let _ = self.permission_tx.send(PermissionRequest {
            request_id: request_id.clone(),
            session_id: args.session_id.to_string(),
            tool_name,
            tool_description: None,
            command_preview,
            options,
        });

        match rx.await {
            Ok(outcome) => Ok(RequestPermissionResponse::new(outcome)),
            Err(_) => Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Cancelled,
            )),
        }
    }

    async fn session_notification(
        &self,
        args: SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        match &args.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(t) = &chunk.content {
                    let _ = self.update_tx.send(AgentUpdate::MessageChunk {
                        text: t.text.clone(),
                    });
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(t) = &chunk.content {
                    let _ = self.update_tx.send(AgentUpdate::ThoughtChunk {
                        text: t.text.clone(),
                    });
                }
            }
            SessionUpdate::ToolCall(tc) => {
                let _ = self.update_tx.send(AgentUpdate::ToolCall {
                    id: tc.tool_call_id.to_string(),
                    title: tc.title.clone(),
                    kind: format!("{:?}", tc.kind),
                });
            }
            SessionUpdate::ToolCallUpdate(tcu) => {
                let _ = self.update_tx.send(AgentUpdate::ToolCallUpdate {
                    id: tcu.tool_call_id.to_string(),
                    title: tcu.fields.title.clone(),
                    status: tcu.fields.status.map(|s| format!("{:?}", s)),
                });
            }
            SessionUpdate::Plan(plan) => {
                let entries = plan
                    .entries
                    .iter()
                    .map(|e| PlanEntry {
                        content: e.content.clone(),
                        priority: format!("{:?}", e.priority),
                        status: format!("{:?}", e.status),
                    })
                    .collect();
                let _ = self.update_tx.send(AgentUpdate::Plan { entries });
            }
            _ => {}
        }
        Ok(())
    }
}
