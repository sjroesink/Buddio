use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub action_type: String,
    pub action_value: String,
    pub category: String,
    pub tags: String,
    pub frequency: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewItem {
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub action_type: String,
    pub action_value: String,
    pub category: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateItem {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub action_type: Option<String>,
    pub action_value: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    pub id: String,
    pub item_id: Option<String>,
    pub command_text: String,
    pub action_type: String,
    pub executed_at: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCommandHistory {
    pub item_id: Option<String>,
    pub command_text: String,
    pub action_type: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub key: String,
    pub value: String,
    pub context: Option<String>,
    pub memory_type: String,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMemory {
    pub key: String,
    pub value: String,
    pub context: Option<String>,
    pub memory_type: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSuggestion {
    pub suggested_command: String,
    pub reason: String,
    pub confidence: f64,
    pub source_item_id: Option<String>,
}

// --- Conversations ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewConversation {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewConversationMessage {
    pub conversation_id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationWithPreview {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub last_message_preview: Option<String>,
}

// --- Slash Commands ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    pub id: String,
    pub name: String,
    pub description: String,
    pub script_path: String,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSlashCommand {
    pub name: String,
    pub description: String,
    pub script_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandParam {
    pub id: String,
    pub command_id: String,
    pub name: String,
    pub description: String,
    pub position: i64,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSlashCommandParam {
    pub name: String,
    pub description: String,
    pub position: i64,
    pub required: bool,
}
