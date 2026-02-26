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

/// Buddio MCP tools that are read-only and safe to auto-allow.
const BUDDIO_READ_TOOLS: &[&str] = &[
    "items_get",
    "items_search",
    "items_list",
    "items_get_categories",
    "items_export",
    "memory_get",
    "memory_get_by_key",
    "memory_search",
    "memory_list",
    "memory_touch",
    "memory_get_relevant",
    "history_search",
    "history_recent",
    "history_suggest",
    "history_recent_rewrites",
    "conversations_get",
    "conversations_list",
    "conversations_search",
    "conversations_get_messages",
    "conversations_search_messages",
    "conversations_recent_context",
    "slash_commands_get",
    "slash_commands_list",
    "slash_commands_search",
    "settings_get",
    "settings_list",
    "db_path",
];

fn should_auto_allow(tool_name: &str, command_preview: Option<&str>) -> bool {
    // Auto-allow Buddio MCP read-only tools
    let tool_lower = tool_name.to_lowercase();
    if BUDDIO_READ_TOOLS.iter().any(|t| tool_lower.contains(t)) {
        return true;
    }

    // Auto-allow read-only CLI commands (Buddio + legacy GoLaunch)
    let Some(preview) = command_preview else {
        return false;
    };

    let normalized = normalize_command_preview(preview);
    let padded = format!(" {normalized} ");

    if !padded.contains("buddio-cli") && !padded.contains("golaunch-cli") {
        return false;
    }

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

/// Buddio's ACP client handler.
///
/// Receives session notifications and permission requests from the agent subprocess
/// and forwards them as serializable types over channels to the Tauri event system.
pub struct BuddioClient {
    update_tx: mpsc::UnboundedSender<AgentUpdate>,
    permission_tx: mpsc::UnboundedSender<PermissionRequest>,
    pending_permissions: Rc<RefCell<HashMap<String, PermissionResponder>>>,
}

impl BuddioClient {
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
impl Client for BuddioClient {
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

        if should_auto_allow(&tool_name, command_preview.as_deref()) {
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
                let content_text = tc.content.iter().find_map(|c| {
                    if let agent_client_protocol::ToolCallContent::Content(inner) = c {
                        if let ContentBlock::Text(t) = &inner.content {
                            return Some(t.text.clone());
                        }
                    }
                    None
                });
                let _ = self.update_tx.send(AgentUpdate::ToolCall {
                    id: tc.tool_call_id.to_string(),
                    title: tc.title.clone(),
                    kind: format!("{:?}", tc.kind),
                    content: content_text,
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
