use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::types::{AgentConfig, AgentStatus, SessionConfigOptionInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Acp,
    Claude,
    Copilot,
    Codex,
}

impl ProviderKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "claude" => Self::Claude,
            "copilot" => Self::Copilot,
            "codex" => Self::Codex,
            _ => Self::Acp,
        }
    }
}

#[async_trait]
pub trait AgentProvider: Send {
    async fn connect(&mut self, config: &AgentConfig, app: AppHandle) -> Result<(), String>;
    async fn disconnect(&mut self) -> Result<(), String>;
    fn status(&self) -> AgentStatus;
    fn send_prompt(&self, content: String) -> Result<(), String>;
    async fn cancel(&mut self) -> Result<(), String>;
    fn resolve_permission(&self, request_id: &str, option_id: &str) -> Result<(), String>;
    fn resolve_question(
        &self,
        request_id: &str,
        answers: std::collections::HashMap<String, String>,
    ) -> Result<(), String>;
    fn config_options(&self) -> Vec<SessionConfigOptionInfo>;
    async fn set_config_option(
        &mut self,
        config_id: &str,
        value: &str,
    ) -> Result<Vec<SessionConfigOptionInfo>, String>;
}
