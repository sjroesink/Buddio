use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredEnvVar {
    pub name: String,
    pub description: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAgent {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub icon: Option<String>,
    pub distribution_type: String,
    /// For npx: the npm package name. For binary: the command (e.g. "./opencode.exe").
    pub distribution_detail: String,
    /// Extra arguments from the registry (e.g. ["acp", "--flag"]).
    pub distribution_args: Vec<String>,
    /// Archive download URL for binary distributions.
    pub archive_url: String,
    pub required_env: Vec<RequiredEnvVar>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentUpdate {
    MessageChunk {
        text: String,
    },
    ThoughtChunk {
        text: String,
    },
    ToolCall {
        id: String,
        title: String,
        kind: String,
        content: Option<String>,
    },
    ToolCallUpdate {
        id: String,
        title: Option<String>,
        status: Option<String>,
    },
    Plan {
        entries: Vec<PlanEntry>,
    },
    TurnComplete {
        stop_reason: String,
    },
    StatusChange {
        status: AgentStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub request_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub tool_description: Option<String>,
    pub command_preview: Option<String>,
    pub options: Vec<PermissionOptionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionOptionInfo {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

// --- User Question types (AskUserQuestion from Claude) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionRequest {
    pub request_id: String,
    pub tool_use_id: String,
    pub questions: Vec<UserQuestionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionItem {
    pub question: String,
    pub header: String,
    pub options: Vec<UserQuestionOption>,
    pub multi_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    #[serde(default)]
    pub provider: String,
    pub source: String,
    pub agent_id: String,
    pub binary_path: String,
    pub args: String,
    pub env: String,
    pub auto_fallback: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_auth_method")]
    pub auth_method: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default)]
    pub ollama_model: String,
}

fn default_auth_method() -> String {
    "oauth".to_string()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

// --- Session Config Option types (serializable mirror of ACP protocol types) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfigSelectOptionInfo {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfigSelectGroupInfo {
    pub group: String,
    pub name: String,
    pub options: Vec<SessionConfigSelectOptionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionConfigSelectOptionsInfo {
    Ungrouped {
        options: Vec<SessionConfigSelectOptionInfo>,
    },
    Grouped {
        groups: Vec<SessionConfigSelectGroupInfo>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfigOptionInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub current_value: String,
    pub select_options: SessionConfigSelectOptionsInfo,
}
