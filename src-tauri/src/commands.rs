use chrono::Timelike;
use golaunch_core::{
    CommandHistory, CommandSuggestion, Conversation, ConversationMessage, ConversationWithPreview,
    Database, Item, Memory, NewCommandHistory, NewConversation, NewConversationMessage, NewItem,
    NewMemory, NewSlashCommand, SlashCommand, SlashCommandParam,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;

use crate::agent::manager::AgentManager;
use crate::agent::registry::{check_agents_installed, fetch_registry};
use crate::agent::types::{AgentConfig, AgentStatus, RegistryAgent, SessionConfigOptionInfo};
use crate::context::LaunchContext;
use crate::hotkey;
use crate::{HotkeyState, LaunchContextState};

pub struct AgentState(pub Arc<Mutex<AgentManager>>);

// --- Existing item commands ---

#[tauri::command]
pub fn search_items(query: String) -> Result<Vec<Item>, String> {
    let db = Database::new()?;
    if query.is_empty() {
        db.list_items(None, false)
    } else {
        db.search_items(&query)
    }
}

#[tauri::command]
pub fn get_all_items() -> Result<Vec<Item>, String> {
    let db = Database::new()?;
    db.list_items(None, false)
}

#[tauri::command]
pub fn execute_item(
    id: String,
    context_state: tauri::State<'_, LaunchContextState>,
) -> Result<Option<String>, String> {
    let db = Database::new()?;
    let mut item = db.get_item(&id)?;
    db.increment_frequency(&id)?;

    // Replace {selection} placeholder with actual selected text
    let launch_context = context_state
        .0
        .lock()
        .map(|c| c.clone())
        .unwrap_or_default();

    if let Some(selected_text) = &launch_context.selected_text {
        // URL encode the selected text for URLs
        if item.action_type == "url" {
            let encoded = urlencoding::encode(selected_text);
            item.action_value = item.action_value.replace("{selection}", &encoded);
        } else {
            item.action_value = item.action_value.replace("{selection}", selected_text);
        }
    }

    // Record command history
    let _ = db.record_command(NewCommandHistory {
        item_id: Some(id.clone()),
        command_text: item.action_value.clone(),
        action_type: item.action_type.clone(),
        source: Some("launcher".to_string()),
    });

    // Auto-learn: record category preference
    let _ = db.add_memory(NewMemory {
        key: "last_used_category".to_string(),
        value: item.category.clone(),
        context: None,
        memory_type: Some("pattern".to_string()),
        confidence: Some(0.5),
    });

    // Auto-learn: record execution hour pattern
    let hour = chrono::Local::now().hour();
    let time_key = format!("active_hour_{}", hour);
    let _ = db.add_memory(NewMemory {
        key: time_key,
        value: item.title.clone(),
        context: Some(item.category.clone()),
        memory_type: Some("pattern".to_string()),
        confidence: Some(0.3),
    });

    match item.action_type.as_str() {
        "url" => {
            open::that(&item.action_value).map_err(|e| format!("Failed to open URL: {e}"))?;
            Ok(None)
        }
        "command" | "script" => {
            // Fire-and-forget: spawn the process without waiting so Buddio
            // never freezes on long-running or interactive commands.
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &item.action_value])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                    .map_err(|e| format!("Failed to execute command: {e}"))?;
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::process::Command::new("sh")
                    .args(["-c", &item.action_value])
                    .spawn()
                    .map_err(|e| format!("Failed to execute command: {e}"))?;
            }

            Ok(None)
        }
        other => Err(format!("Unknown action type: {other}")),
    }
}

#[tauri::command]
pub fn get_categories() -> Result<Vec<String>, String> {
    let db = Database::new()?;
    db.get_categories()
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("launcher-reset", ());
    if let Some(window) = app.get_webview_window("main") {
        let _: () = window
            .hide()
            .map_err(|e| format!("Failed to hide window: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_window_compact(
    app: AppHandle,
    compact: bool,
    anchor: Option<String>,
) -> Result<(), String> {
    const DEFAULT_WIDTH: f64 = 680.0;
    const COMPACT_HEIGHT: f64 = 64.0;
    const NORMAL_HEIGHT: f64 = 480.0;

    if let Some(window) = app.get_webview_window("main") {
        let height = if compact {
            COMPACT_HEIGHT
        } else {
            NORMAL_HEIGHT
        };

        let _: () = window
            .set_size(Size::Logical(LogicalSize::new(DEFAULT_WIDTH, height)))
            .map_err(|e| format!("Failed to resize window: {e}"))?;

        if let Some(monitor) = window
            .primary_monitor()
            .map_err(|e| format!("Failed to read primary monitor: {e}"))?
        {
            let scale = monitor.scale_factor();
            let monitor_pos = monitor.position();
            let monitor_size = monitor.size();

            let target_width_px = (DEFAULT_WIDTH * scale).round() as i32;
            let target_height_px = (height * scale).round() as i32;
            let normal_height_px = (NORMAL_HEIGHT * scale).round() as i32;

            let centered_x = monitor_pos.x + (monitor_size.width as i32 - target_width_px) / 2;
            let centered_normal_top =
                monitor_pos.y + (monitor_size.height as i32 - normal_height_px) / 2;

            let y = if anchor.as_deref() == Some("bottom") {
                centered_normal_top + normal_height_px - target_height_px
            } else {
                centered_normal_top
            };

            let _: () = window
                .set_position(Position::Physical(PhysicalPosition::new(centered_x, y)))
                .map_err(|e| format!("Failed to position window: {e}"))?;
        } else {
            let _: () = window
                .center()
                .map_err(|e| format!("Failed to center window: {e}"))?;
        }
    }

    Ok(())
}

// --- Settings commands ---

#[tauri::command]
pub fn get_setting(key: String) -> Result<Option<String>, String> {
    let db = Database::new()?;
    db.get_setting(&key)
}

#[tauri::command]
pub fn set_setting(key: String, value: String) -> Result<(), String> {
    let db = Database::new()?;
    db.set_setting(&key, &value)
}

#[tauri::command]
pub fn get_agent_config() -> Result<AgentConfig, String> {
    let db = Database::new()?;
    load_agent_config(&db)
}

#[tauri::command]
pub fn save_agent_config(config: AgentConfig) -> Result<(), String> {
    let db = Database::new()?;
    db.set_setting("acp.source", &config.source)?;
    db.set_setting("acp.agent_id", &config.agent_id)?;
    db.set_setting("acp.binary_path", &config.binary_path)?;
    db.set_setting("acp.args", &config.args)?;
    db.set_setting("acp.env", &config.env)?;
    db.set_setting(
        "acp.auto_fallback",
        if config.auto_fallback {
            "true"
        } else {
            "false"
        },
    )?;
    // New provider fields
    db.set_setting("agent.provider", &config.provider)?;
    db.set_setting("agent.claude.api_key", &config.api_key)?;
    db.set_setting("agent.claude.model", &config.model)?;
    db.set_setting("agent.claude.auth_method", &config.auth_method)?;
    Ok(())
}

fn load_agent_config(db: &Database) -> Result<AgentConfig, String> {
    Ok(AgentConfig {
        provider: db
            .get_setting("agent.provider")?
            .unwrap_or_else(|| "acp".to_string()),
        source: db.get_setting("acp.source")?.unwrap_or_default(),
        agent_id: db.get_setting("acp.agent_id")?.unwrap_or_default(),
        binary_path: db.get_setting("acp.binary_path")?.unwrap_or_default(),
        args: db.get_setting("acp.args")?.unwrap_or_default(),
        env: db.get_setting("acp.env")?.unwrap_or_default(),
        auto_fallback: db
            .get_setting("acp.auto_fallback")?
            .map(|v| v == "true")
            .unwrap_or(false),
        api_key: db
            .get_setting("agent.claude.api_key")?
            .unwrap_or_default(),
        model: db
            .get_setting("agent.claude.model")?
            .unwrap_or_default(),
        auth_method: db
            .get_setting("agent.claude.auth_method")?
            .unwrap_or_else(|| "oauth".to_string()),
    })
}

// --- Command History commands ---

#[tauri::command]
pub fn record_command(
    item_id: Option<String>,
    command_text: String,
    action_type: String,
) -> Result<CommandHistory, String> {
    let db = Database::new()?;
    db.record_command(NewCommandHistory {
        item_id,
        command_text,
        action_type,
        source: Some("launcher".to_string()),
    })
}

#[tauri::command]
pub fn get_command_suggestions(query: String) -> Result<Vec<CommandSuggestion>, String> {
    let db = Database::new()?;
    db.suggest_commands(&query)
}

#[tauri::command]
pub fn add_item_from_suggestion(
    title: String,
    action_value: String,
    action_type: String,
    category: Option<String>,
) -> Result<Item, String> {
    let db = Database::new()?;
    db.add_item(NewItem {
        title,
        subtitle: Some("Added from suggestion".to_string()),
        icon: None,
        action_type,
        action_value,
        category,
        tags: None,
    })
}

// --- Memory commands ---

#[tauri::command]
pub fn get_memories(query: Option<String>) -> Result<Vec<Memory>, String> {
    let db = Database::new()?;
    match query {
        Some(q) if !q.is_empty() => db.search_memories(&q),
        _ => db.list_memories(None),
    }
}

#[tauri::command]
pub fn add_memory_cmd(
    key: String,
    value: String,
    context: Option<String>,
    memory_type: Option<String>,
    confidence: Option<f64>,
) -> Result<Memory, String> {
    let db = Database::new()?;
    db.add_memory(NewMemory {
        key,
        value,
        context,
        memory_type,
        confidence,
    })
}

#[tauri::command]
pub fn remove_memory(id: String) -> Result<bool, String> {
    let db = Database::new()?;
    db.remove_memory(&id)
}

#[tauri::command]
pub fn get_memory_by_key(key: String, context: Option<String>) -> Result<Memory, String> {
    let db = Database::new()?;
    db.get_memory_by_key(&key, context.as_deref())
}

#[tauri::command]
pub fn get_relevant_memories(context: Option<String>) -> Result<Vec<Memory>, String> {
    let db = Database::new()?;
    db.get_relevant_memories(context.as_deref())
}

// --- Conversation commands ---

#[tauri::command]
pub fn create_conversation(title: String) -> Result<Conversation, String> {
    let db = Database::new()?;
    db.create_conversation(NewConversation { title })
}

#[tauri::command]
pub fn list_conversations(limit: Option<usize>) -> Result<Vec<ConversationWithPreview>, String> {
    let db = Database::new()?;
    db.list_conversations(limit.unwrap_or(50))
}

#[tauri::command]
pub fn get_conversation_messages(
    conversation_id: String,
) -> Result<Vec<ConversationMessage>, String> {
    let db = Database::new()?;
    db.get_conversation_messages(&conversation_id)
}

#[tauri::command]
pub fn add_conversation_message(
    conversation_id: String,
    role: String,
    content: String,
) -> Result<ConversationMessage, String> {
    let db = Database::new()?;
    db.add_conversation_message(NewConversationMessage {
        conversation_id,
        role,
        content,
    })
}

#[tauri::command]
pub fn search_conversations(query: String) -> Result<Vec<ConversationWithPreview>, String> {
    let db = Database::new()?;
    db.search_conversations(&query)
}

#[tauri::command]
pub fn delete_conversation(id: String) -> Result<bool, String> {
    let db = Database::new()?;
    db.delete_conversation(&id)
}

// --- Agent lifecycle commands ---

#[tauri::command]
pub async fn agent_connect(
    app: AppHandle,
    state: tauri::State<'_, AgentState>,
    config: AgentConfig,
) -> Result<(), String> {
    let mut manager = state.inner().0.lock().await;
    manager.connect(app.clone(), config).await
}

#[tauri::command]
pub async fn agent_disconnect(state: tauri::State<'_, AgentState>) -> Result<(), String> {
    let mut manager = state.inner().0.lock().await;
    manager.disconnect().await
}

#[tauri::command]
pub async fn agent_get_status(state: tauri::State<'_, AgentState>) -> Result<AgentStatus, String> {
    let manager = state.inner().0.lock().await;
    Ok(manager.status())
}

// --- Agent prompting commands ---

#[tauri::command]
pub async fn agent_prompt(
    state: tauri::State<'_, AgentState>,
    context_state: tauri::State<'_, LaunchContextState>,
    query: String,
    context_items: Vec<Item>,
) -> Result<(), String> {
    let db = Database::new()?;

    // Read the current launch context
    let launch_context = context_state
        .0
        .lock()
        .map(|c| c.clone())
        .unwrap_or_default();

    // Fetch baseline relevant memories, then enrich with query-term matches (including facts).
    let mut memories = db.get_relevant_memories(None).unwrap_or_default();
    let mut seen_memory_ids: HashSet<String> = memories.iter().map(|m| m.id.clone()).collect();

    let mut memory_terms = vec![query.clone()];
    let mut seen_terms: HashSet<String> = HashSet::from([query.to_lowercase()]);
    for token in query.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
        let normalized = token.trim().to_lowercase();
        if normalized.len() < 3 || !seen_terms.insert(normalized.clone()) {
            continue;
        }
        memory_terms.push(normalized);
    }

    for term in memory_terms {
        if let Ok(matches) = db.search_memories(&term) {
            for memory in matches {
                if seen_memory_ids.insert(memory.id.clone()) {
                    memories.push(memory);
                }
            }
        }
    }

    // Fetch command suggestions for this query
    let suggestions = db.suggest_commands(&query).unwrap_or_default();

    // Fetch recent command history for behavioral context
    let recent_history = db.get_recent_commands(10).unwrap_or_default();

    // Fetch recent conversation context
    let recent_conversations = db.get_recent_conversation_context(3).unwrap_or_default();

    // Fetch registered slash commands
    let slash_commands = db.list_slash_commands().unwrap_or_default();

    let mut manager = state.inner().0.lock().await;
    manager
        .prompt(
            &query,
            &context_items,
            &memories,
            &suggestions,
            &recent_history,
            &recent_conversations,
            &launch_context,
            &slash_commands,
        )
        .await
}

#[tauri::command]
pub async fn agent_prompt_slash_command(
    state: tauri::State<'_, AgentState>,
    query: String,
) -> Result<(), String> {
    let db = Database::new()?;

    let memories = db.get_relevant_memories(None).unwrap_or_default();
    let slash_commands = db.list_slash_commands().unwrap_or_default();

    let mut manager = state.inner().0.lock().await;
    manager
        .prompt_slash_command(&query, &memories, &slash_commands)
        .await
}

#[tauri::command]
pub async fn agent_cancel(state: tauri::State<'_, AgentState>) -> Result<(), String> {
    let mut manager = state.inner().0.lock().await;
    manager.cancel().await
}

// --- Agent permission commands ---

#[tauri::command]
pub async fn agent_resolve_permission(
    state: tauri::State<'_, AgentState>,
    request_id: String,
    option_id: String,
) -> Result<(), String> {
    let mut manager = state.inner().0.lock().await;
    manager.resolve_permission(&request_id, &option_id).await
}

#[tauri::command]
pub async fn agent_resolve_question(
    state: tauri::State<'_, AgentState>,
    request_id: String,
    answers: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let manager = state.inner().0.lock().await;
    manager.resolve_question(&request_id, answers)
}

// --- Agent config option commands ---

#[tauri::command]
pub async fn agent_get_config_options(
    state: tauri::State<'_, AgentState>,
) -> Result<Vec<SessionConfigOptionInfo>, String> {
    let manager = state.inner().0.lock().await;
    Ok(manager.get_config_options())
}

#[tauri::command]
pub async fn agent_set_config_option(
    state: tauri::State<'_, AgentState>,
    config_id: String,
    value: String,
) -> Result<Vec<SessionConfigOptionInfo>, String> {
    let mut manager = state.inner().0.lock().await;
    manager.set_config_option(&config_id, &value).await
}

// --- ACP registry commands (keep acp_ prefix as these are ACP-specific) ---

#[tauri::command]
pub async fn acp_fetch_registry() -> Result<Vec<RegistryAgent>, String> {
    fetch_registry().await
}

#[tauri::command]
pub async fn acp_check_agents_installed(
    agents: Vec<RegistryAgent>,
) -> Result<HashMap<String, bool>, String> {
    tokio::task::spawn_blocking(move || Ok(check_agents_installed(&agents)))
        .await
        .map_err(|e| format!("Failed to check installations: {e}"))?
}

// --- Agent install command ---

#[tauri::command]
pub async fn acp_install_agent(agent: RegistryAgent) -> Result<String, String> {
    match agent.distribution_type.as_str() {
        "npx" if !agent.distribution_detail.is_empty() => {
            let package = agent.distribution_detail.clone();
            tokio::task::spawn_blocking(move || {
                #[cfg(target_os = "windows")]
                let output = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &format!("npm install -g {}", package)])
                    .output()
                    .map_err(|e| format!("Failed to run npm install: {e}"))?;

                #[cfg(not(target_os = "windows"))]
                let output = std::process::Command::new("npm")
                    .args(["install", "-g", &package])
                    .output()
                    .map_err(|e| format!("Failed to run npm install: {e}"))?;

                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    Ok(format!("Successfully installed {package}\n{stdout}"))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    Err(format!("npm install failed:\n{stderr}"))
                }
            })
            .await
            .map_err(|e| format!("Install task failed: {e}"))?
        }
        "binary" if !agent.archive_url.is_empty() => {
            let archive_url = agent.archive_url.clone();
            let agent_id = agent.id.clone();
            let agent_name = agent.name.clone();

            // Download the archive and extract to a well-known directory
            let install_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("Buddio")
                .join("agents")
                .join(&agent_id);

            let install_dir_clone = install_dir.clone();
            tokio::task::spawn_blocking(move || {
                // Create the install directory
                std::fs::create_dir_all(&install_dir_clone)
                    .map_err(|e| format!("Failed to create install dir: {e}"))?;

                // Download the archive
                let response = reqwest::blocking::get(&archive_url)
                    .map_err(|e| format!("Failed to download {agent_name}: {e}"))?;
                let bytes = response
                    .bytes()
                    .map_err(|e| format!("Failed to read download: {e}"))?;

                // Extract based on file extension
                if archive_url.ends_with(".tar.gz") || archive_url.ends_with(".tgz") {
                    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(&bytes));
                    let mut archive = tar::Archive::new(decoder);
                    archive
                        .unpack(&install_dir_clone)
                        .map_err(|e| format!("Failed to extract tar.gz: {e}"))?;
                } else if archive_url.ends_with(".zip") {
                    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&*bytes))
                        .map_err(|e| format!("Failed to open zip: {e}"))?;
                    archive
                        .extract(&install_dir_clone)
                        .map_err(|e| format!("Failed to extract zip: {e}"))?;
                } else {
                    return Err(format!("Unsupported archive format: {archive_url}"));
                }

                Ok(format!(
                    "Successfully installed {agent_name} to {}",
                    install_dir_clone.display()
                ))
            })
            .await
            .map_err(|e| format!("Install task failed: {e}"))?
        }
        "binary" => Err(format!(
            "No download URL available for {}. Please install it manually.",
            agent.name
        )),
        _ => Err(format!(
            "Unknown distribution type: {}",
            agent.distribution_type
        )),
    }
}

// --- Launch context commands ---

#[tauri::command]
pub fn get_launch_context(
    state: tauri::State<'_, LaunchContextState>,
) -> Result<LaunchContext, String> {
    let lock = state
        .0
        .lock()
        .map_err(|e| format!("Failed to read launch context: {e}"))?;
    Ok(lock.clone())
}

#[tauri::command]
pub async fn type_text_to_app(app: AppHandle, text: String) -> Result<(), String> {
    // Hide the launcher first so the source app regains focus
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Small delay to let the OS switch focus back to the source app
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    tokio::task::spawn_blocking(move || crate::context::type_text(&text))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn replace_selection_text(app: AppHandle, text: String) -> Result<(), String> {
    // Hide the launcher first so the source app regains focus
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Small delay to let the OS switch focus back to the source app
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    tokio::task::spawn_blocking(move || crate::context::replace_selection(&text))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

// --- Rewrite history commands ---

#[tauri::command]
pub fn record_rewrite(prompt: String) -> Result<(), String> {
    let db = Database::new()?;
    db.record_command(NewCommandHistory {
        item_id: None,
        command_text: prompt,
        action_type: "rewrite".to_string(),
        source: Some("selection_rewrite".to_string()),
    })?;
    Ok(())
}

#[tauri::command]
pub fn get_rewrite_suggestions() -> Result<Vec<CommandSuggestion>, String> {
    let db = Database::new()?;
    db.get_recent_rewrites(10)
}

// --- Slash command commands ---

#[tauri::command]
pub fn list_slash_commands() -> Result<Vec<SlashCommand>, String> {
    let db = Database::new()?;
    db.list_slash_commands()
}

#[tauri::command]
pub fn search_slash_commands(query: String) -> Result<Vec<SlashCommand>, String> {
    let db = Database::new()?;
    db.search_slash_commands(&query)
}

#[tauri::command]
pub fn get_slash_command_by_name(name: String) -> Result<SlashCommand, String> {
    let db = Database::new()?;
    db.get_slash_command_by_name(&name)
}

#[tauri::command]
pub fn add_slash_command(
    name: String,
    description: String,
    script_path: String,
) -> Result<SlashCommand, String> {
    let db = Database::new()?;
    db.add_slash_command(NewSlashCommand {
        name,
        description,
        script_path,
    })
}

#[tauri::command]
pub fn remove_slash_command(name: String) -> Result<bool, String> {
    let db = Database::new()?;
    db.remove_slash_command_by_name(&name)
}

#[tauri::command]
pub fn execute_slash_command(name: String, args: String) -> Result<String, String> {
    let db = Database::new()?;
    let cmd = db.get_slash_command_by_name(&name)?;
    db.increment_slash_command_usage(&cmd.id)?;

    let _ = db.record_command(NewCommandHistory {
        item_id: None,
        command_text: format!("/{} {}", name, args),
        action_type: "slash_command".to_string(),
        source: Some("launcher".to_string()),
    });

    #[cfg(target_os = "windows")]
    let output = {
        std::process::Command::new("powershell")
            .args(["-ExecutionPolicy", "Bypass", "-File", &cmd.script_path])
            .args(args.split_whitespace().collect::<Vec<&str>>())
            .output()
            .map_err(|e| format!("Failed to execute script: {e}"))?
    };

    #[cfg(not(target_os = "windows"))]
    let output = {
        std::process::Command::new("sh")
            .arg(&cmd.script_path)
            .args(args.split_whitespace().collect::<Vec<&str>>())
            .output()
            .map_err(|e| format!("Failed to execute script: {e}"))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!("Script failed:\n{stderr}\n{stdout}"))
    }
}

#[tauri::command]
pub fn get_slash_command_params(name: String) -> Result<Vec<SlashCommandParam>, String> {
    let db = Database::new()?;
    db.get_params_by_command_name(&name)
}

// --- Shortcut mode commands ---

#[tauri::command]
pub fn get_shortcut_mode(state: tauri::State<'_, HotkeyState>) -> Result<String, String> {
    let mgr = state.0.lock().map_err(|e| format!("{e}"))?;
    Ok(mgr.shortcut_str().to_string())
}

#[tauri::command]
pub fn set_shortcut_mode(
    app: AppHandle,
    state: tauri::State<'_, HotkeyState>,
    mode: String,
) -> Result<(), String> {
    let mut mgr = state.0.lock().map_err(|e| format!("{e}"))?;
    mgr.switch_shortcut(&app, &mode)?;
    let db = Database::new()?;
    db.set_setting(hotkey::SETTING_KEY, &mode)?;
    Ok(())
}

// --- Update commands ---

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Updater not available: {e}"))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            body: update.body.clone(),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Update check failed: {e}")),
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Updater not available: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {e}"))?
        .ok_or_else(|| "No update available".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Failed to install update: {e}"))?;
    app.restart();
}

// --- Per-agent env var commands ---

#[tauri::command]
pub fn get_agent_env(agent_id: String) -> Result<Vec<(String, String)>, String> {
    let db = Database::new()?;
    let prefix = format!("acp.env.{}.", agent_id);
    let all_settings = db.get_all_settings()?;
    let env_vars: Vec<(String, String)> = all_settings
        .into_iter()
        .filter(|s| s.key.starts_with(&prefix))
        .map(|s| {
            let var_name = s.key.strip_prefix(&prefix).unwrap_or(&s.key).to_string();
            (var_name, s.value)
        })
        .collect();
    Ok(env_vars)
}

#[tauri::command]
pub fn set_agent_env(agent_id: String, env_name: String, value: String) -> Result<(), String> {
    let db = Database::new()?;
    let key = format!("acp.env.{}.{}", agent_id, env_name);
    db.set_setting(&key, &value)
}
