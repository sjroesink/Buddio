use golaunch_core::{
    Database, NewCommandHistory, NewConversation, NewConversationMessage, NewItem, NewMemory,
    NewSlashCommand, UpdateItem,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Deserialize;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Parameter structs
// ---------------------------------------------------------------------------

// --- Items ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ItemsAddParams {
    /// Item title displayed in the launcher
    pub title: String,
    /// Action type: "command", "url", or "script" (default: "command")
    #[serde(default = "default_command")]
    pub action_type: String,
    /// The command, URL, or script path to execute
    pub action_value: String,
    /// Subtitle / description
    pub subtitle: Option<String>,
    /// Icon (emoji or icon name)
    pub icon: Option<String>,
    /// Category for grouping
    pub category: Option<String>,
    /// Comma-separated tags
    pub tags: Option<String>,
}
fn default_command() -> String {
    "command".to_string()
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdParams {
    /// The ID of the item/resource
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ItemsUpdateParams {
    /// The item ID to update
    pub id: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub action_type: Option<String>,
    pub action_value: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ItemsListParams {
    /// Filter by category
    pub category: Option<String>,
    /// Include disabled items (default: false)
    #[serde(default)]
    pub include_disabled: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// Search query string
    pub query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ItemsImportParams {
    /// JSON array of items to import
    pub items: Vec<NewItemParam>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NewItemParam {
    pub title: String,
    #[serde(default = "default_command")]
    pub action_type: String,
    pub action_value: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
}

// --- Memory ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryAddParams {
    /// Memory key (e.g., "preferred_editor")
    pub key: String,
    /// Memory value (e.g., "vscode")
    pub value: String,
    /// Optional context (e.g., category)
    pub context: Option<String>,
    /// Memory type: "preference", "pattern", or "fact" (default: "fact")
    pub memory_type: Option<String>,
    /// Confidence score 0.0–1.0 (default: 1.0)
    pub confidence: Option<f64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryGetByKeyParams {
    /// Memory key
    pub key: String,
    /// Optional context
    pub context: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemoryListParams {
    /// Filter by memory type: "preference", "pattern", or "fact"
    pub memory_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct OptionalContextParams {
    /// Optional context filter
    pub context: Option<String>,
}

// --- History ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HistoryRecordParams {
    /// The command text that was executed
    pub command_text: String,
    /// Action type (e.g., "command", "url", "script", "slash_command")
    pub action_type: String,
    /// Optional associated item ID
    pub item_id: Option<String>,
    /// Source identifier (default: "mcp")
    pub source: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LimitParams {
    /// Maximum number of results (default: 20)
    pub limit: Option<usize>,
}

// --- Conversations ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConversationCreateParams {
    /// Conversation title
    pub title: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConversationAddMessageParams {
    /// The conversation ID to add a message to
    pub conversation_id: String,
    /// Message role: "user" or "assistant"
    pub role: String,
    /// Message content
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConversationGetMessagesParams {
    /// The conversation ID
    pub conversation_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConversationSearchMessagesParams {
    /// Search query
    pub query: String,
    /// Maximum number of results (default: 20)
    pub limit: Option<usize>,
}

// --- Slash Commands ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SlashCommandAddParams {
    /// Command name (without leading /)
    pub name: String,
    /// Description of what the command does
    #[serde(default)]
    pub description: String,
    /// Path to the script file
    pub script_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SlashCommandNameParams {
    /// Command name (without leading /)
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SlashCommandRunParams {
    /// Command name (without leading /)
    pub name: String,
    /// Arguments to pass to the script
    #[serde(default)]
    pub args: String,
}

// --- Settings ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SettingsKeyParams {
    /// Setting key
    pub key: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SettingsSetParams {
    /// Setting key
    pub key: String,
    /// Setting value
    pub value: String,
}

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct GoLaunchMcp {
    db_path: Option<PathBuf>,
    tool_router: ToolRouter<Self>,
}

impl GoLaunchMcp {
    pub fn new(db_path: Option<PathBuf>) -> Self {
        Self {
            db_path,
            tool_router: Self::tool_router(),
        }
    }

    fn get_db(&self) -> Result<Database, McpError> {
        match &self.db_path {
            Some(path) => Database::with_path(path).map_err(|e| McpError::internal_error(e, None)),
            None => Database::new().map_err(|e| McpError::internal_error(e, None)),
        }
    }
}

fn db_err(e: String) -> McpError {
    McpError::internal_error(e, None)
}

fn json_text<T: serde::Serialize>(val: &T) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(val).unwrap(),
    )])
}

// ---------------------------------------------------------------------------
// All MCP tools
// ---------------------------------------------------------------------------

#[tool_router]
impl GoLaunchMcp {
    // ── Items (10) ──────────────────────────────────────────────────────

    #[tool(
        description = "Add a new launcher item with a title, action type (command/url/script), and action value"
    )]
    fn items_add(
        &self,
        Parameters(p): Parameters<ItemsAddParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let item = db
            .add_item(NewItem {
                title: p.title,
                subtitle: p.subtitle,
                icon: p.icon,
                action_type: p.action_type,
                action_value: p.action_value,
                category: p.category,
                tags: p.tags,
            })
            .map_err(db_err)?;
        Ok(json_text(&item))
    }

    #[tool(description = "Get a single launcher item by its ID")]
    fn items_get(&self, Parameters(p): Parameters<IdParams>) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let item = db.get_item(&p.id).map_err(db_err)?;
        Ok(json_text(&item))
    }

    #[tool(
        description = "Update fields of an existing launcher item by ID. Only provided fields are changed."
    )]
    fn items_update(
        &self,
        Parameters(p): Parameters<ItemsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let item = db
            .update_item(
                &p.id,
                UpdateItem {
                    title: p.title,
                    subtitle: p.subtitle,
                    icon: p.icon,
                    action_type: p.action_type,
                    action_value: p.action_value,
                    category: p.category,
                    tags: p.tags,
                    enabled: p.enabled,
                },
            )
            .map_err(db_err)?;
        Ok(json_text(&item))
    }

    #[tool(description = "Remove a launcher item by its ID")]
    fn items_remove(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let removed = db.remove_item(&p.id).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "removed": removed })))
    }

    #[tool(
        description = "Search launcher items by query string (matches title, subtitle, tags, category)"
    )]
    fn items_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let items = db.search_items(&p.query).map_err(db_err)?;
        Ok(json_text(&items))
    }

    #[tool(
        description = "List all launcher items, optionally filtered by category. Set include_disabled to true to include disabled items."
    )]
    fn items_list(
        &self,
        Parameters(p): Parameters<ItemsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let items = db
            .list_items(p.category.as_deref(), p.include_disabled)
            .map_err(db_err)?;
        Ok(json_text(&items))
    }

    #[tool(
        description = "Execute a launcher item by ID: opens URLs in the browser, runs commands/scripts in the shell. Records execution in history."
    )]
    fn items_run(&self, Parameters(p): Parameters<IdParams>) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let item = db.get_item(&p.id).map_err(db_err)?;
        db.increment_frequency(&p.id).map_err(db_err)?;
        let _ = db.record_command(NewCommandHistory {
            item_id: Some(p.id.clone()),
            command_text: item.action_value.clone(),
            action_type: item.action_type.clone(),
            source: Some("mcp".to_string()),
        });

        match item.action_type.as_str() {
            "url" => {
                open::that(&item.action_value).map_err(|e| {
                    McpError::internal_error(format!("Failed to open URL: {e}"), None)
                })?;
                Ok(json_text(&serde_json::json!({
                    "executed": true,
                    "title": item.title,
                    "action_type": "url",
                    "action_value": item.action_value
                })))
            }
            "command" | "script" => {
                #[cfg(target_os = "windows")]
                let output = std::process::Command::new("cmd")
                    .args(["/C", &item.action_value])
                    .output()
                    .map_err(|e| {
                        McpError::internal_error(format!("Failed to execute: {e}"), None)
                    })?;

                #[cfg(not(target_os = "windows"))]
                let output = std::process::Command::new("sh")
                    .args(["-c", &item.action_value])
                    .output()
                    .map_err(|e| {
                        McpError::internal_error(format!("Failed to execute: {e}"), None)
                    })?;

                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Ok(json_text(&serde_json::json!({
                    "executed": true,
                    "title": item.title,
                    "action_type": item.action_type,
                    "success": output.status.success(),
                    "stdout": stdout,
                    "stderr": stderr
                })))
            }
            other => Err(McpError::internal_error(
                format!("Unknown action type: {other}"),
                None,
            )),
        }
    }

    #[tool(description = "List all distinct item categories")]
    fn items_get_categories(&self) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let categories = db.get_categories().map_err(db_err)?;
        Ok(json_text(&categories))
    }

    #[tool(description = "Import launcher items from a JSON array of item objects")]
    fn items_import(
        &self,
        Parameters(p): Parameters<ItemsImportParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let new_items: Vec<NewItem> = p
            .items
            .into_iter()
            .map(|i| NewItem {
                title: i.title,
                subtitle: i.subtitle,
                icon: i.icon,
                action_type: i.action_type,
                action_value: i.action_value,
                category: i.category,
                tags: i.tags,
            })
            .collect();
        let imported = db.import_items(new_items).map_err(db_err)?;
        Ok(json_text(&imported))
    }

    #[tool(description = "Export all launcher items as a JSON array")]
    fn items_export(&self) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let items = db.export_items().map_err(db_err)?;
        Ok(json_text(&items))
    }

    // ── Memory (8) ──────────────────────────────────────────────────────

    #[tool(
        description = "Add or update a memory entry. If a memory with the same key+context already exists, it will be updated."
    )]
    fn memory_add(
        &self,
        Parameters(p): Parameters<MemoryAddParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let mem = db
            .add_memory(NewMemory {
                key: p.key,
                value: p.value,
                context: p.context,
                memory_type: p.memory_type,
                confidence: p.confidence,
            })
            .map_err(db_err)?;
        Ok(json_text(&mem))
    }

    #[tool(description = "Get a memory entry by its ID")]
    fn memory_get(&self, Parameters(p): Parameters<IdParams>) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let mem = db.get_memory(&p.id).map_err(db_err)?;
        Ok(json_text(&mem))
    }

    #[tool(description = "Get a memory entry by its key and optional context")]
    fn memory_get_by_key(
        &self,
        Parameters(p): Parameters<MemoryGetByKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let mem = db
            .get_memory_by_key(&p.key, p.context.as_deref())
            .map_err(db_err)?;
        Ok(json_text(&mem))
    }

    #[tool(description = "Remove a memory entry by its ID")]
    fn memory_remove(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let removed = db.remove_memory(&p.id).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "removed": removed })))
    }

    #[tool(description = "Search memories by query (matches key, value, context)")]
    fn memory_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let memories = db.search_memories(&p.query).map_err(db_err)?;
        Ok(json_text(&memories))
    }

    #[tool(
        description = "List all memories, optionally filtered by type (preference/pattern/fact)"
    )]
    fn memory_list(
        &self,
        Parameters(p): Parameters<MemoryListParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let memories = db.list_memories(p.memory_type.as_deref()).map_err(db_err)?;
        Ok(json_text(&memories))
    }

    #[tool(description = "Update the last_accessed timestamp of a memory entry")]
    fn memory_touch(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        db.touch_memory(&p.id).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "success": true })))
    }

    #[tool(
        description = "Get relevant memories (preferences and patterns with confidence > 0.3), optionally filtered by context"
    )]
    fn memory_get_relevant(
        &self,
        Parameters(p): Parameters<OptionalContextParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let memories = db
            .get_relevant_memories(p.context.as_deref())
            .map_err(db_err)?;
        Ok(json_text(&memories))
    }

    // ── Command History (5) ─────────────────────────────────────────────

    #[tool(description = "Record a command execution in history")]
    fn history_record(
        &self,
        Parameters(p): Parameters<HistoryRecordParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let entry = db
            .record_command(NewCommandHistory {
                item_id: p.item_id,
                command_text: p.command_text,
                action_type: p.action_type,
                source: Some(p.source.unwrap_or_else(|| "mcp".to_string())),
            })
            .map_err(db_err)?;
        Ok(json_text(&entry))
    }

    #[tool(description = "Search command history by query")]
    fn history_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let entries = db.search_command_history(&p.query).map_err(db_err)?;
        Ok(json_text(&entries))
    }

    #[tool(description = "Get recent command history entries")]
    fn history_recent(
        &self,
        Parameters(p): Parameters<LimitParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let entries = db
            .get_recent_commands(p.limit.unwrap_or(20))
            .map_err(db_err)?;
        Ok(json_text(&entries))
    }

    #[tool(
        description = "Get AI-like command suggestions based on a query (checks history, items, and fallback)"
    )]
    fn history_suggest(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let suggestions = db.suggest_commands(&p.query).map_err(db_err)?;
        Ok(json_text(&suggestions))
    }

    #[tool(description = "Get recent rewrite prompts from history (for selection-rewrite feature)")]
    fn history_recent_rewrites(
        &self,
        Parameters(p): Parameters<LimitParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let suggestions = db
            .get_recent_rewrites(p.limit.unwrap_or(10))
            .map_err(db_err)?;
        Ok(json_text(&suggestions))
    }

    // ── Conversations (9) ───────────────────────────────────────────────

    #[tool(description = "Create a new conversation with a title")]
    fn conversations_create(
        &self,
        Parameters(p): Parameters<ConversationCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let conv = db
            .create_conversation(NewConversation { title: p.title })
            .map_err(db_err)?;
        Ok(json_text(&conv))
    }

    #[tool(description = "Get a conversation by its ID")]
    fn conversations_get(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let conv = db.get_conversation(&p.id).map_err(db_err)?;
        Ok(json_text(&conv))
    }

    #[tool(description = "List recent conversations with message count and last message preview")]
    fn conversations_list(
        &self,
        Parameters(p): Parameters<LimitParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let convs = db
            .list_conversations(p.limit.unwrap_or(20))
            .map_err(db_err)?;
        Ok(json_text(&convs))
    }

    #[tool(description = "Search conversations by title or message content")]
    fn conversations_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let convs = db.search_conversations(&p.query).map_err(db_err)?;
        Ok(json_text(&convs))
    }

    #[tool(description = "Delete a conversation and all its messages")]
    fn conversations_delete(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let deleted = db.delete_conversation(&p.id).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "deleted": deleted })))
    }

    #[tool(description = "Add a message to a conversation")]
    fn conversations_add_message(
        &self,
        Parameters(p): Parameters<ConversationAddMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let msg = db
            .add_conversation_message(NewConversationMessage {
                conversation_id: p.conversation_id,
                role: p.role,
                content: p.content,
            })
            .map_err(db_err)?;
        Ok(json_text(&msg))
    }

    #[tool(description = "Get all messages in a conversation, ordered chronologically")]
    fn conversations_get_messages(
        &self,
        Parameters(p): Parameters<ConversationGetMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let messages = db
            .get_conversation_messages(&p.conversation_id)
            .map_err(db_err)?;
        Ok(json_text(&messages))
    }

    #[tool(description = "Search messages across all conversations")]
    fn conversations_search_messages(
        &self,
        Parameters(p): Parameters<ConversationSearchMessagesParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let messages = db
            .search_conversation_messages(&p.query, p.limit.unwrap_or(20))
            .map_err(db_err)?;
        Ok(json_text(&messages))
    }

    #[tool(
        description = "Get recent conversation context: last N conversations with their last 5 messages each (useful for agent context)"
    )]
    fn conversations_recent_context(
        &self,
        Parameters(p): Parameters<LimitParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let context = db
            .get_recent_conversation_context(p.limit.unwrap_or(5))
            .map_err(db_err)?;
        let output: Vec<serde_json::Value> = context
            .iter()
            .map(|(conv, msgs)| {
                serde_json::json!({
                    "conversation": conv,
                    "messages": msgs,
                })
            })
            .collect();
        Ok(json_text(&output))
    }

    // ── Slash Commands (7) ──────────────────────────────────────────────

    #[tool(description = "Register a new slash command with a name, description, and script path")]
    fn slash_commands_add(
        &self,
        Parameters(p): Parameters<SlashCommandAddParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let cmd = db
            .add_slash_command(NewSlashCommand {
                name: p.name,
                description: p.description,
                script_path: p.script_path,
            })
            .map_err(db_err)?;
        Ok(json_text(&cmd))
    }

    #[tool(description = "Get a slash command by its name")]
    fn slash_commands_get(
        &self,
        Parameters(p): Parameters<SlashCommandNameParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let cmd = db.get_slash_command_by_name(&p.name).map_err(db_err)?;
        Ok(json_text(&cmd))
    }

    #[tool(description = "List all registered slash commands")]
    fn slash_commands_list(&self) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let commands = db.list_slash_commands().map_err(db_err)?;
        Ok(json_text(&commands))
    }

    #[tool(description = "Search slash commands by name or description")]
    fn slash_commands_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let commands = db.search_slash_commands(&p.query).map_err(db_err)?;
        Ok(json_text(&commands))
    }

    #[tool(description = "Remove a slash command by its name")]
    fn slash_commands_remove(
        &self,
        Parameters(p): Parameters<SlashCommandNameParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let removed = db.remove_slash_command_by_name(&p.name).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "removed": removed })))
    }

    #[tool(
        description = "Execute a slash command by name with optional arguments. Returns stdout/stderr."
    )]
    fn slash_commands_run(
        &self,
        Parameters(p): Parameters<SlashCommandRunParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let cmd = db.get_slash_command_by_name(&p.name).map_err(db_err)?;
        db.increment_slash_command_usage(&cmd.id).map_err(db_err)?;

        let _ = db.record_command(NewCommandHistory {
            item_id: None,
            command_text: format!("/{} {}", p.name, p.args),
            action_type: "slash_command".to_string(),
            source: Some("mcp".to_string()),
        });

        #[cfg(target_os = "windows")]
        let output = std::process::Command::new("powershell")
            .args(["-ExecutionPolicy", "Bypass", "-File", &cmd.script_path])
            .args(p.args.split_whitespace())
            .output()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to execute script: {e}"), None)
            })?;

        #[cfg(not(target_os = "windows"))]
        let output = std::process::Command::new("sh")
            .arg(&cmd.script_path)
            .args(p.args.split_whitespace())
            .output()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to execute script: {e}"), None)
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(json_text(&serde_json::json!({
            "success": output.status.success(),
            "stdout": stdout,
            "stderr": stderr
        })))
    }

    #[tool(description = "Increment the usage counter for a slash command by its ID")]
    fn slash_commands_increment_usage(
        &self,
        Parameters(p): Parameters<IdParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        db.increment_slash_command_usage(&p.id).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "success": true })))
    }

    // ── Settings (4) ────────────────────────────────────────────────────

    #[tool(description = "Get a setting value by key")]
    fn settings_get(
        &self,
        Parameters(p): Parameters<SettingsKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let value = db.get_setting(&p.key).map_err(db_err)?;
        Ok(json_text(
            &serde_json::json!({ "key": p.key, "value": value }),
        ))
    }

    #[tool(description = "Set a setting value (creates or updates)")]
    fn settings_set(
        &self,
        Parameters(p): Parameters<SettingsSetParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        db.set_setting(&p.key, &p.value).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "success": true })))
    }

    #[tool(description = "Delete a setting by key")]
    fn settings_delete(
        &self,
        Parameters(p): Parameters<SettingsKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let deleted = db.delete_setting(&p.key).map_err(db_err)?;
        Ok(json_text(&serde_json::json!({ "deleted": deleted })))
    }

    #[tool(description = "List all settings")]
    fn settings_list(&self) -> Result<CallToolResult, McpError> {
        let db = self.get_db()?;
        let settings = db.get_all_settings().map_err(db_err)?;
        Ok(json_text(&settings))
    }

    // ── Utility (1) ─────────────────────────────────────────────────────

    #[tool(description = "Get the path to the GoLaunch database file")]
    fn db_path(&self) -> Result<CallToolResult, McpError> {
        let path = match &self.db_path {
            Some(p) => p.display().to_string(),
            None => Database::db_path().map_err(db_err)?.display().to_string(),
        };
        Ok(json_text(&serde_json::json!({ "path": path })))
    }
}

// ---------------------------------------------------------------------------
// ServerHandler implementation
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for GoLaunchMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "GoLaunch MCP server. Manages launcher items, memories, command history, \
                 conversations, slash commands, and settings for the GoLaunch keyboard launcher."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
