use golaunch_core::{
    CommandHistory, CommandSuggestion, Conversation, ConversationMessage, Item, Memory,
    SlashCommand,
};
use tauri::AppHandle;

use super::acp::AcpProvider;
use super::provider::{AgentProvider, ProviderKind};
use super::sidecar::SidecarProvider;
use super::types::{AgentConfig, AgentStatus, SessionConfigOptionInfo};

pub struct AgentManager {
    provider: Option<Box<dyn AgentProvider>>,
    provider_kind: ProviderKind,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            provider: None,
            provider_kind: ProviderKind::Acp,
        }
    }

    pub fn status(&self) -> AgentStatus {
        self.provider
            .as_ref()
            .map(|p| p.status())
            .unwrap_or(AgentStatus::Disconnected)
    }

    pub async fn connect(&mut self, app: AppHandle, config: AgentConfig) -> Result<(), String> {
        // Determine provider kind from config
        let kind = ProviderKind::from_str(&config.provider);
        self.provider_kind = kind;

        // Create the appropriate provider
        let mut provider: Box<dyn AgentProvider> = match kind {
            ProviderKind::Acp => Box::new(AcpProvider::new()),
            ProviderKind::Claude => Box::new(SidecarProvider::new("Claude")),
            ProviderKind::Copilot => Box::new(SidecarProvider::new("Copilot")),
        };

        provider.connect(&config, app).await?;
        self.provider = Some(provider);
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), String> {
        if let Some(mut provider) = self.provider.take() {
            provider.disconnect().await?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn prompt(
        &mut self,
        query: &str,
        context_items: &[Item],
        memories: &[Memory],
        suggestions: &[CommandSuggestion],
        recent_history: &[CommandHistory],
        recent_conversations: &[(Conversation, Vec<ConversationMessage>)],
        launch_context: &crate::context::LaunchContext,
        slash_commands: &[SlashCommand],
    ) -> Result<(), String> {
        let prompt_text = build_agent_prompt(
            query,
            context_items,
            memories,
            suggestions,
            recent_history,
            recent_conversations,
            launch_context,
            slash_commands,
        );
        self.send_prompt(prompt_text)
    }

    pub async fn prompt_slash_command(
        &mut self,
        query: &str,
        memories: &[Memory],
        slash_commands: &[SlashCommand],
    ) -> Result<(), String> {
        let prompt_text = build_slash_command_prompt(query, memories, slash_commands);
        self.send_prompt(prompt_text)
    }

    fn send_prompt(&self, prompt_text: String) -> Result<(), String> {
        let provider = self.provider.as_ref().ok_or("No agent provider active")?;
        provider.send_prompt(prompt_text)
    }

    pub async fn cancel(&mut self) -> Result<(), String> {
        let provider = self.provider.as_mut().ok_or("No agent provider active")?;
        provider.cancel().await
    }

    pub async fn resolve_permission(
        &mut self,
        request_id: &str,
        option_id: &str,
    ) -> Result<(), String> {
        let provider = self.provider.as_ref().ok_or("No agent provider active")?;
        provider.resolve_permission(request_id, option_id)
    }

    pub fn resolve_question(
        &self,
        request_id: &str,
        answers: std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let provider = self.provider.as_ref().ok_or("No agent provider active")?;
        provider.resolve_question(request_id, answers)
    }

    pub fn get_config_options(&self) -> Vec<SessionConfigOptionInfo> {
        self.provider
            .as_ref()
            .map(|p| p.config_options())
            .unwrap_or_default()
    }

    pub async fn set_config_option(
        &mut self,
        config_id: &str,
        value: &str,
    ) -> Result<Vec<SessionConfigOptionInfo>, String> {
        let provider = self.provider.as_mut().ok_or("No agent provider active")?;
        provider.set_config_option(config_id, value).await
    }
}

/// Build a focused prompt for the slash command creation agent.
/// This agent's sole purpose is to collaborate with the user to create a script,
/// register it as a slash command, and execute it.
fn build_slash_command_prompt(
    query: &str,
    memories: &[Memory],
    slash_commands: &[SlashCommand],
) -> String {
    let slash_dir = golaunch_core::Database::slash_commands_dir()
        .unwrap_or_else(|_| "slash-commands".into())
        .to_string_lossy()
        .to_string();

    let mut p = String::with_capacity(2048);

    // ── System instructions ──
    p.push_str(
        "You are Buddio Slash Command Builder — a dedicated agent for creating and executing \
         user-defined slash commands. The user typed a slash command that doesn't exist yet.\n\n\
         Your ONLY job:\n\
         1. Figure out what the script should do based on the command name and arguments.\n\
            For example: `/kill 4924` → a script that kills the process running on port 4924.\n\
         2. Show a PREVIEW of what you'll create using this exact format:\n\n\
            **Command:** `/commandname`\n\
            **Description:** What it does\n\
            **Parameters:**\n\
            - `param1` (required) — description of param1\n\
            - `param2` (optional) — description of param2\n\
            **Script preview:**\n\
            ```\n\
            (the full script content)\n\
            ```\n\n\
            Then ask: \"Shall I create this command?\"\n\
         3. WAIT for the user to confirm (e.g., \"yes\", \"ok\", \"go ahead\").\n\
            - If the user says no or asks for changes, revise and show the preview again.\n\
            - CRITICAL: Do NOT call `slash_commands_add` until the user explicitly confirms.\n\
         4. After confirmation, call `slash_commands_add` with the script and parameter definitions.\n\
         5. Briefly confirm the command was created. STOP — do NOT execute the command yourself.\n\
            The Buddio UI will provide an execute button to the user.\n\n\
         Guidelines:\n\
         - Be smart about inferring the purpose from the command name — the user expects you to understand.\n\
         - Be concise — the user is in a launcher and wants quick results.\n\
         - Make scripts robust: validate arguments, handle errors, produce clear output.\n\
         - If the purpose is ambiguous, ask ONE clarifying question, then proceed.\n\
         - Always define parameters with clear names and descriptions for every argument the script accepts.\n\
         - ONLY use the Buddio MCP tools listed below. Do NOT use Bash, Read, Glob, Grep, \
           Write, Edit, or any other filesystem tools. Do NOT explore the codebase or look for \
           configuration files. You already have everything you need.\n\n",
    );

    // ── Platform info ──
    if cfg!(target_os = "windows") {
        p.push_str(
            "## Platform\n\
             Windows — write PowerShell scripts (`.ps1`). Arguments are passed as positional \
             params (`$args[0]`, `$args[1]`, etc.).\n\n",
        );
    } else if cfg!(target_os = "macos") {
        p.push_str(
            "## Platform\n\
             macOS — write Bash scripts (`.sh`). Arguments are passed as `$1`, `$2`, etc. \
             Make sure to add `#!/bin/bash` as the first line.\n\n",
        );
    } else {
        p.push_str(
            "## Platform\n\
             Linux — write Bash scripts (`.sh`). Arguments are passed as `$1`, `$2`, etc. \
             Make sure to add `#!/bin/bash` as the first line.\n\n",
        );
    }

    // ── MCP tools reference (slash commands only) ──
    p.push_str(&format!(
        "## Buddio MCP Tools\n\
         Script storage directory: `{slash_dir}`\n\n\
         Available slash command tools:\n\
         - `slash_commands_add` — Create a new slash command (name, description, script_content, params). \
           The `params` field is an array of parameter definitions: [{{name, description, position, required}}]. \
           Writes the script file automatically.\n\
         - `slash_commands_get` — Get a slash command by name\n\
         - `slash_commands_list` — List all registered slash commands\n\
         - `slash_commands_search` — Search slash commands by name or description\n\
         - `slash_commands_remove` — Remove a slash command by name\n\
         - `slash_commands_get_params` — Get parameter definitions for a slash command by name\n\n\
         Workflow: show a preview and wait for user approval, then call `slash_commands_add` with the script contents and params. Do NOT execute the command — the UI handles that.\n\n"
    ));

    // ── User memory / preferences ──
    if !memories.is_empty() {
        p.push_str("## User Preferences\n");
        for mem in memories {
            let ctx = mem
                .context
                .as_deref()
                .map(|c| format!(" (context: {c})"))
                .unwrap_or_default();
            p.push_str(&format!(
                "- {}: {}{} [type: {}]\n",
                mem.key, mem.value, ctx, mem.memory_type
            ));
        }
        p.push('\n');
    }

    // ── Registered slash commands ──
    if !slash_commands.is_empty() {
        p.push_str("## Existing Slash Commands\n");
        p.push_str("These already exist — avoid name collisions:\n");
        for cmd in slash_commands {
            p.push_str(&format!(
                "- **/{name}**: {desc} (script: `{path}`, used {count} times)\n",
                name = cmd.name,
                desc = cmd.description,
                path = cmd.script_path,
                count = cmd.usage_count,
            ));
        }
        p.push('\n');
    }

    // ── User query ──
    p.push_str(&format!("## User Query\n{query}\n"));

    p
}

/// Build a structured prompt for the agent that includes system instructions,
/// MCP tools reference, user context, and the query.
#[allow(clippy::too_many_arguments)]
fn build_agent_prompt(
    query: &str,
    context_items: &[Item],
    memories: &[Memory],
    suggestions: &[CommandSuggestion],
    recent_history: &[CommandHistory],
    recent_conversations: &[(Conversation, Vec<ConversationMessage>)],
    launch_context: &crate::context::LaunchContext,
    slash_commands: &[SlashCommand],
) -> String {
    let mut p = String::with_capacity(4096);

    // ── System instructions ──
    p.push_str(
        "You are Buddio Assistant, an AI helper embedded in Buddio — a keyboard-driven \
         launcher application. The user typed a search query that didn't match any of their \
         predefined commands, so they're asking you for help.\n\n\
         Your capabilities:\n\
         1. Directly add, update, or remove launcher commands using the Buddio MCP tools\n\
         2. Query the launcher database to find commands, history, and memories\n\
         3. Manage the user's persistent memory (preferences, facts, patterns)\n\
         4. Help the user figure out what command they need\n\
         5. Answer questions about tools, CLIs, and workflows\n\n\
         IMPORTANT — Action-oriented behavior:\n\
         - When the user wants to add a command or item, ALWAYS show a preview first and ask for approval \
           before calling `items_add`. Show the item details (title, action, type, category) and ask: \
           \"Shall I add this?\" Wait for the user to confirm before creating it.\n\
         - After successfully adding an item, ask the user: \"Want me to run it now?\" \
           If the user confirms, call `items_run` to execute it.\n\
         - When the user asks about their setup, QUERY the database first, then answer.\n\
         - If the query is ambiguous, CHECK memory and existing commands first before asking clarifying questions.\n\
         - Treat memory facts as authoritative context (e.g., if a name maps to a project, use that meaning).\n\
         - Reading memory/list/history is a safe lookup step and should be done proactively without requesting permission.\n\
         - When you learn something about the user's preferences, SAVE it to memory.\n\
         - After making changes, briefly confirm what you did.\n\
         - Be concise — the user is in a launcher and wants quick results.\n\
         - If the query looks like a command (e.g. \"npm install\", \"docker compose up\"), \
           suggest adding it as a launcher item — but always show a preview and wait for approval first.\n\
         - You have access to the user's current context: selected text, clipboard, and source application.\n\
         - IMPORTANT: Distinguish between two types of requests:\n\
           A) REWRITE requests — when the user explicitly asks to rewrite, rephrase, translate, \
              summarize, or transform selected text: use the `replace_selection` tool to directly \
              replace their selection with the new text. Do NOT output the rewritten text as a regular \
              message — always use the tool. The tool will handle replacing the text and closing the launcher.\n\
           B) ACTION requests — when the user asks to add a command, open something, go somewhere, \
              or perform any action. Even if there is selected text, this is NOT a rewrite. \
              Add the item using the appropriate MCP tool and confirm what you did. The launcher will offer a \"Run\" button.\n\
         - CRITICAL for rewrites: When using `replace_selection`, preserve the exact same format as the \
           selected text. If the input is plain text, pass plain text. If it's code, pass code without \
           markdown fences. If it's HTML, pass HTML. Never add markdown formatting. The text argument \
           will be pasted directly in place of the selection, so it must be in the same format.\n\
         - When working with selected text, consider the source application for appropriate formatting.\n\n",
    );

    // ── MCP Tools Reference ──
    p.push_str(
        "## Buddio MCP Tools\n\
         You have Buddio MCP tools available for managing the launcher. Use these tools directly.\n\n\
         ### Items\n\
         - `items_add` — Add a new launcher item (title, action_value, action_type: command/url/script, category, subtitle, icon, tags)\n\
         - `items_get` — Get an item by ID\n\
         - `items_update` — Update item fields by ID (only provided fields are changed)\n\
         - `items_remove` — Remove an item by ID\n\
         - `items_search` — Search items by query (matches title, subtitle, tags, category)\n\
         - `items_list` — List all items, optionally filtered by category\n\
         - `items_run` — Execute an item by ID (opens URLs, runs commands/scripts)\n\
         - `items_get_categories` — List all distinct categories\n\
         - `items_import` — Import items from a JSON array\n\
         - `items_export` — Export all items as JSON\n\
         Action types: `command` (shell), `url` (browser), `script` (script file)\n\n\
         ### Command Execution Details\n\
         CRITICAL: Buddio ALWAYS uses PowerShell on Windows — NEVER cmd.exe. \
         Do NOT mention cmd, cmd.exe, or Command Prompt in your responses. \
         ALL commands are executed via `powershell -NoProfile -Command \"<action_value>\"`. \
         If a command fails, the issue is PowerShell syntax — never blame cmd.\n\n\
         This means `action_value` MUST be valid PowerShell syntax. Important rules:\n\
         - Do NOT use `start powershell -NoExit ...` — `start` is an alias for `Start-Process` in PowerShell \
           and `Start-Process` does NOT have a `-NoExit` parameter.\n\
         - To run a command in a new visible PowerShell window that stays open, use:\n\
           `Start-Process powershell -ArgumentList '-NoExit', '-Command', '\"your commands here\"'`\n\
         - For fire-and-forget background commands (no window needed), just write the command directly:\n\
           `Set-Location \"path\"; docker compose up -d`\n\
         - Use single quotes for literal strings containing paths. Use Set-Location instead of cd when needed.\n\
         - Commands are spawned asynchronously (fire-and-forget) — the launcher does not wait for them to finish.\n\
         - When debugging command failures, always analyze the error as a PowerShell error.\n\n\
         ### Memory\n\
         - `memory_add` — Add or update a memory (key, value, memory_type: preference/pattern/fact, context, confidence)\n\
         - `memory_get` — Get memory by ID\n\
         - `memory_get_by_key` — Get memory by key and optional context\n\
         - `memory_remove` — Remove memory by ID\n\
         - `memory_search` — Search memories by query\n\
         - `memory_list` — List all memories, optionally filtered by type\n\
         - `memory_touch` — Update last_accessed timestamp\n\
         - `memory_get_relevant` — Get relevant memories (preferences/patterns with confidence > 0.3)\n\
         Memory types: `preference` (user preference), `pattern` (learned behavior), `fact` (stored info)\n\n\
         ### Command History\n\
         - `history_record` — Record a command execution\n\
         - `history_search` — Search command history\n\
         - `history_recent` — Get recent history entries (default: 20)\n\
         - `history_suggest` — Get command suggestions based on query\n\
         - `history_recent_rewrites` — Get recent rewrite prompts\n\n\
         ### Conversations\n\
         - `conversations_create` — Create a new conversation\n\
         - `conversations_get` — Get conversation by ID\n\
         - `conversations_list` — List recent conversations with previews\n\
         - `conversations_search` — Search conversations by title/content\n\
         - `conversations_delete` — Delete a conversation and its messages\n\
         - `conversations_add_message` — Add a message (role: user/assistant, content)\n\
         - `conversations_get_messages` — Get all messages in a conversation\n\
         - `conversations_search_messages` — Search messages across conversations\n\
         - `conversations_recent_context` — Get recent conversations with last messages\n\
         Use conversation tools to recall earlier discussions with the user.\n\n\
         ### Slash Commands\n\
         Slash commands are user-defined scripts invoked with `/name args...` from the launcher.\n\
         Creation of new slash commands is handled by a dedicated agent — you do not need to create them.\n\
         You can reference existing slash commands listed below for context.\n\n\
         ### Settings\n\
         - `settings_get` — Get a setting by key\n\
         - `settings_set` — Set a setting value\n\
         - `settings_delete` — Delete a setting\n\
         - `settings_list` — List all settings\n\n\
         ### Utility\n\
         - `db_path` — Get the database file path\n\n",
    );

    // ── User memory / preferences ──
    if !memories.is_empty() {
        p.push_str("## User Memory Context\n");
        for mem in memories {
            let ctx = mem
                .context
                .as_deref()
                .map(|c| format!(" (context: {c})"))
                .unwrap_or_default();
            p.push_str(&format!(
                "- {}: {}{} [type: {}]\n",
                mem.key, mem.value, ctx, mem.memory_type
            ));
        }
        p.push('\n');
    }

    // ── Registered slash commands ──
    if !slash_commands.is_empty() {
        p.push_str("## Registered Slash Commands\n");
        for cmd in slash_commands {
            p.push_str(&format!(
                "- **/{name}**: {desc} (script: `{path}`, used {count} times)\n",
                name = cmd.name,
                desc = cmd.description,
                path = cmd.script_path,
                count = cmd.usage_count,
            ));
        }
        p.push('\n');
    }

    // ── Existing launcher items ──
    if !context_items.is_empty() {
        p.push_str("## User's Predefined Commands\n");
        for item in context_items {
            let subtitle = item
                .subtitle
                .as_deref()
                .map(|s| format!(" — {s}"))
                .unwrap_or_default();
            p.push_str(&format!(
                "- **{}**{} [{}]: `{}` (category: {}, id: {})\n",
                item.title, subtitle, item.action_type, item.action_value, item.category, item.id
            ));
        }
        p.push('\n');
    }

    // ── Command suggestions ──
    if !suggestions.is_empty() {
        p.push_str("## Possible Matches\n");
        p.push_str("Suggestions from command history and similar existing items:\n");
        for s in suggestions {
            let source = match s.reason.as_str() {
                "history_match" => "previously executed",
                "similar_item" => "similar to existing command",
                "query_parse" => "parsed from query",
                other => other,
            };
            p.push_str(&format!(
                "- `{}` ({}; confidence: {:.0}%)\n",
                s.suggested_command,
                source,
                s.confidence * 100.0
            ));
        }
        p.push('\n');
    }

    // ── Recent command history ──
    if !recent_history.is_empty() {
        p.push_str("## Recent Command History\n");
        for entry in recent_history {
            p.push_str(&format!(
                "- `{}` [{}] at {}\n",
                entry.command_text, entry.action_type, entry.executed_at
            ));
        }
        p.push('\n');
    }

    // ── Recent conversations ──
    if !recent_conversations.is_empty() {
        p.push_str("## Recent Conversation Context\n");
        p.push_str("Summary of recent conversations with this user (use `conversations_get_messages` for full details):\n\n");
        for (conv, messages) in recent_conversations {
            p.push_str(&format!(
                "**{}** (id: {}, updated: {})\n",
                conv.title, conv.id, conv.updated_at
            ));
            for msg in messages {
                let role = match msg.role.as_str() {
                    "user" => "User",
                    "assistant" => "Assistant",
                    other => other,
                };
                let content = if msg.content.len() > 200 {
                    format!("{}...", &msg.content[..197])
                } else {
                    msg.content.clone()
                };
                p.push_str(&format!("  {}: {}\n", role, content));
            }
            p.push('\n');
        }
    }

    // ── Launch context ──
    let has_context = launch_context.selected_text.is_some()
        || launch_context.clipboard_text.is_some()
        || launch_context.source_window_title.is_some()
        || launch_context.source_process_name.is_some()
        || launch_context.source_process_path.is_some();

    if has_context {
        p.push_str("## Current Context\n");
        if let Some(ref title) = launch_context.source_window_title {
            let process = launch_context
                .source_process_name
                .as_deref()
                .unwrap_or("unknown");
            p.push_str(&format!("Source application: {} ({})\n", title, process));
        } else if let Some(ref process) = launch_context.source_process_name {
            p.push_str(&format!("Source process: {}\n", process));
        }
        if let Some(ref process_path) = launch_context.source_process_path {
            p.push_str(&format!("Source process path: {}\n", process_path));
        }
        if let Some(ref text) = launch_context.selected_text {
            let truncated = if text.len() > 2000 {
                format!("{}... [truncated]", &text[..2000])
            } else {
                text.clone()
            };
            p.push_str(&format!("Selected text:\n```\n{}\n```\n", truncated));
        }
        if let Some(ref text) = launch_context.clipboard_text {
            let truncated = if text.len() > 1000 {
                format!("{}... [truncated]", &text[..1000])
            } else {
                text.clone()
            };
            p.push_str(&format!("Clipboard contents:\n```\n{}\n```\n", truncated));
        }
        p.push('\n');
    }

    // ── User query ──
    p.push_str(&format!("## User Query\n{query}\n"));

    p
}
