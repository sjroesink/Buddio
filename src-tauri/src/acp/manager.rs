use agent_client_protocol::{
    Agent, ClientCapabilities, ClientSideConnection, ContentBlock, Implementation,
    InitializeRequest, McpServer, McpServerStdio, NewSessionRequest, PermissionOptionId,
    ProtocolVersion, RequestPermissionOutcome, SelectedPermissionOutcome, SessionConfigId,
    SessionConfigKind, SessionConfigOption, SessionConfigSelectOptions, SessionConfigValueId,
    SessionId, SetSessionConfigOptionRequest, TextContent,
};
use golaunch_core::{
    CommandHistory, CommandSuggestion, Conversation, ConversationMessage, Item, Memory,
    SlashCommand,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::client::BuddioClient;
use super::types::{
    AgentConfig, AgentStatus, AgentUpdate, PermissionRequest, SessionConfigOptionInfo,
    SessionConfigSelectGroupInfo, SessionConfigSelectOptionInfo, SessionConfigSelectOptionsInfo,
};

pub struct AcpManager {
    status: AgentStatus,
    session_id: Option<SessionId>,
    prompt_tx: Option<mpsc::UnboundedSender<PromptCommand>>,
    cancel_tx: Option<mpsc::UnboundedSender<()>>,
    permission_resolve_tx: Option<mpsc::UnboundedSender<(String, String)>>,
    config_option_tx: Option<mpsc::UnboundedSender<ConfigOptionCommand>>,
    shutdown_tx: Option<mpsc::UnboundedSender<()>>,
    config_options: Vec<SessionConfigOptionInfo>,
}

enum PromptCommand {
    Prompt {
        session_id: SessionId,
        content: Vec<ContentBlock>,
    },
}

struct ConfigOptionCommand {
    session_id: SessionId,
    config_id: String,
    value: String,
    reply: oneshot::Sender<Result<Vec<SessionConfigOptionInfo>, String>>,
}

impl AcpManager {
    pub fn new() -> Self {
        Self {
            status: AgentStatus::Disconnected,
            session_id: None,
            prompt_tx: None,
            cancel_tx: None,
            permission_resolve_tx: None,
            config_option_tx: None,
            shutdown_tx: None,
            config_options: Vec::new(),
        }
    }

    pub fn status(&self) -> AgentStatus {
        self.status
    }

    pub async fn connect(&mut self, app: AppHandle, config: AgentConfig) -> Result<(), String> {
        if self.status == AgentStatus::Connected {
            return Ok(());
        }

        self.status = AgentStatus::Connecting;
        let _ = app.emit(
            "acp-update",
            AgentUpdate::StatusChange {
                status: AgentStatus::Connecting,
            },
        );

        let binary = if config.binary_path.is_empty() {
            return Err("No binary path configured".to_string());
        } else {
            config.binary_path.clone()
        };

        let args: Vec<String> = if config.args.is_empty() {
            vec![]
        } else {
            config.args.split_whitespace().map(String::from).collect()
        };

        // Resolve the binary path: check if it's on PATH, otherwise look in
        // our install directory (AppData/Local/Buddio/agents/<agent_id>/)
        let resolved_binary = resolve_binary_path(&binary, &config.agent_id);

        // On Windows, commands like "npx" are actually .cmd batch scripts
        // that cannot be spawned directly. We need to run them through cmd.exe.
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = tokio::process::Command::new("cmd");
            let mut cmd_args = vec!["/C".to_string(), resolved_binary.clone()];
            cmd_args.extend(args.clone());
            c.args(&cmd_args);
            c
        };
        #[cfg(not(target_os = "windows"))]
        let mut cmd = {
            let mut c = tokio::process::Command::new(&resolved_binary);
            c.args(&args);
            c
        };

        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        // Parse env vars from "KEY=VALUE,KEY2=VALUE2"
        if !config.env.is_empty() {
            for pair in config.env.split(',') {
                if let Some((k, v)) = pair.split_once('=') {
                    cmd.env(k.trim(), v.trim());
                }
            }
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn agent process: {e}"))?;

        let child_stdin = child.stdin.take().ok_or("Failed to get agent stdin")?;
        let child_stdout = child.stdout.take().ok_or("Failed to get agent stdout")?;

        // Channels for bridging async ACP events to Tauri
        let (update_tx, mut update_rx) = mpsc::unbounded_channel::<AgentUpdate>();
        let (permission_tx, mut permission_rx) = mpsc::unbounded_channel::<PermissionRequest>();
        let (prompt_tx, prompt_rx) = mpsc::unbounded_channel::<PromptCommand>();
        let (cancel_tx, cancel_rx) = mpsc::unbounded_channel::<()>();
        let (perm_resolve_tx, perm_resolve_rx) = mpsc::unbounded_channel::<(String, String)>();
        let (config_option_tx, config_option_rx) = mpsc::unbounded_channel::<ConfigOptionCommand>();
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel::<()>();

        // Session initialization oneshot (now returns config options too)
        let (session_tx, session_rx) =
            oneshot::channel::<Result<(SessionId, Vec<SessionConfigOptionInfo>), String>>();

        // Spawn the ACP connection on a dedicated thread with LocalSet
        // (required because Client trait is !Send)
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async move {
                let acp_client = BuddioClient::new(update_tx.clone(), permission_tx);
                let pending_perms = acp_client.pending_permissions();

                let stdin_async =
                    tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(child_stdin);
                let stdout_async =
                    tokio_util::compat::TokioAsyncReadCompatExt::compat(child_stdout);

                let (connection, io_future) =
                    ClientSideConnection::new(acp_client, stdin_async, stdout_async, |fut| {
                        tokio::task::spawn_local(fut);
                    });

                // Spawn I/O handler
                tokio::task::spawn_local(async move {
                    if let Err(e) = io_future.await {
                        eprintln!("ACP I/O error: {e:?}");
                    }
                });

                // Initialize the connection
                let init_result = connection
                    .initialize(
                        InitializeRequest::new(ProtocolVersion::LATEST)
                            .client_info(Implementation::new("Buddio", "0.1.0"))
                            .client_capabilities(ClientCapabilities::new()),
                    )
                    .await;

                if let Err(e) = init_result {
                    let _ = session_tx.send(Err(format!("Initialize failed: {e:?}")));
                    return;
                }

                // Create a new session with the Buddio MCP server
                let cwd = std::env::current_dir().unwrap_or_else(|_| "/".into());
                let mcp_binary = resolve_mcp_binary_path();
                let mcp_server = McpServer::Stdio(McpServerStdio::new("buddio", &mcp_binary));
                let session_result = connection
                    .new_session(NewSessionRequest::new(cwd).mcp_servers(vec![mcp_server]))
                    .await;

                let (session_id, initial_config_options) = match session_result {
                    Ok(resp) => {
                        let config_infos = resp
                            .config_options
                            .as_ref()
                            .map(|opts| opts.iter().map(convert_config_option).collect())
                            .unwrap_or_default();
                        (resp.session_id, config_infos)
                    }
                    Err(e) => {
                        let _ = session_tx.send(Err(format!("New session failed: {e:?}")));
                        return;
                    }
                };

                let _ = session_tx.send(Ok((session_id.clone(), initial_config_options)));

                // Handle permission resolves from the Tauri thread
                let pending_perms_clone = pending_perms.clone();
                let mut perm_resolve_rx = perm_resolve_rx;
                tokio::task::spawn_local(async move {
                    while let Some((request_id, option_id)) = perm_resolve_rx.recv().await {
                        if let Some(responder) =
                            pending_perms_clone.borrow_mut().remove(&request_id)
                        {
                            let outcome = RequestPermissionOutcome::Selected(
                                SelectedPermissionOutcome::new(PermissionOptionId::new(option_id)),
                            );
                            let _ = responder.send(outcome);
                        }
                    }
                });

                // Wrap connection in Rc for sharing between prompt and config handlers
                let connection = std::rc::Rc::new(connection);

                // Handle prompts from the Tauri thread
                let conn_for_prompts = connection.clone();
                let mut prompt_rx = prompt_rx;
                tokio::task::spawn_local(async move {
                    while let Some(cmd) = prompt_rx.recv().await {
                        match cmd {
                            PromptCommand::Prompt {
                                session_id,
                                content,
                            } => {
                                let result = conn_for_prompts
                                    .prompt(agent_client_protocol::PromptRequest::new(
                                        session_id, content,
                                    ))
                                    .await;

                                // Yield several times to let any pending
                                // spawn_local notification tasks (message
                                // chunks) flush before we emit TurnComplete.
                                // The ACP RPC layer spawns each notification
                                // handler as a separate local task, so the
                                // prompt response can resolve before the last
                                // message-chunk tasks have run.
                                for _ in 0..3 {
                                    tokio::task::yield_now().await;
                                }

                                match result {
                                    Ok(resp) => {
                                        let _ = update_tx.send(AgentUpdate::TurnComplete {
                                            stop_reason: format!("{:?}", resp.stop_reason),
                                        });
                                    }
                                    Err(e) => {
                                        let _ = update_tx.send(AgentUpdate::TurnComplete {
                                            stop_reason: format!("Error: {e:?}"),
                                        });
                                    }
                                }
                            }
                        }
                    }
                });

                // Handle config option changes from the Tauri thread
                let conn_for_config = connection.clone();
                let mut config_option_rx = config_option_rx;
                tokio::task::spawn_local(async move {
                    while let Some(cmd) = config_option_rx.recv().await {
                        let result = conn_for_config
                            .set_session_config_option(SetSessionConfigOptionRequest::new(
                                cmd.session_id,
                                SessionConfigId::new(cmd.config_id),
                                SessionConfigValueId::new(cmd.value),
                            ))
                            .await;

                        let reply_result = match result {
                            Ok(resp) => {
                                let infos: Vec<SessionConfigOptionInfo> = resp
                                    .config_options
                                    .iter()
                                    .map(convert_config_option)
                                    .collect();
                                Ok(infos)
                            }
                            Err(e) => Err(format!("Failed to set config option: {e:?}")),
                        };
                        let _ = cmd.reply.send(reply_result);
                    }
                });

                // Handle cancels
                let pending_perms_cancel = pending_perms;
                let mut cancel_rx = cancel_rx;
                tokio::task::spawn_local(async move {
                    while let Some(()) = cancel_rx.recv().await {
                        // Cancel all pending permissions
                        let mut perms = pending_perms_cancel.borrow_mut();
                        for (_id, responder) in perms.drain() {
                            let _ = responder.send(RequestPermissionOutcome::Cancelled);
                        }
                    }
                });

                // Wait for shutdown signal
                let mut shutdown_rx = shutdown_rx;
                shutdown_rx.recv().await;

                // Kill the child process
                let _ = child.kill().await;
            });
        });

        // Wait for session initialization
        let (session_id, initial_config_options) = session_rx
            .await
            .map_err(|_| "Connection thread died".to_string())??;

        self.session_id = Some(session_id);
        self.prompt_tx = Some(prompt_tx);
        self.cancel_tx = Some(cancel_tx);
        self.permission_resolve_tx = Some(perm_resolve_tx);
        self.config_option_tx = Some(config_option_tx);
        self.shutdown_tx = Some(shutdown_tx);
        self.config_options = initial_config_options;
        self.status = AgentStatus::Connected;

        let _ = app.emit(
            "acp-update",
            AgentUpdate::StatusChange {
                status: AgentStatus::Connected,
            },
        );

        // Emit initial config options if any
        if !self.config_options.is_empty() {
            let _ = app.emit("acp-config-options", &self.config_options);
        }

        // Spawn background tasks to forward updates and permissions to Tauri events
        let app_for_updates = app.clone();
        tokio::spawn(async move {
            while let Some(update) = update_rx.recv().await {
                let _ = app_for_updates.emit("acp-update", &update);
            }
        });

        let app_for_perms = app;
        tokio::spawn(async move {
            while let Some(perm) = permission_rx.recv().await {
                let _ = app_for_perms.emit("acp-permission-request", &perm);
            }
        });

        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), String> {
        // Signal the connection thread to shut down
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        self.prompt_tx.take();
        self.cancel_tx.take();
        self.permission_resolve_tx.take();
        self.config_option_tx.take();
        self.session_id.take();
        self.config_options.clear();

        self.status = AgentStatus::Disconnected;
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
        let session_id = self.session_id.clone().ok_or("Not connected to agent")?;
        let prompt_tx = self.prompt_tx.as_ref().ok_or("Not connected to agent")?;

        let content = vec![ContentBlock::Text(TextContent::new(prompt_text))];

        prompt_tx
            .send(PromptCommand::Prompt {
                session_id,
                content,
            })
            .map_err(|_| "Failed to send prompt to agent".to_string())
    }

    pub async fn cancel(&mut self) -> Result<(), String> {
        let cancel_tx = self.cancel_tx.as_ref().ok_or("Not connected to agent")?;

        cancel_tx
            .send(())
            .map_err(|_| "Failed to send cancel to agent".to_string())
    }

    pub async fn resolve_permission(
        &mut self,
        request_id: &str,
        option_id: &str,
    ) -> Result<(), String> {
        let tx = self
            .permission_resolve_tx
            .as_ref()
            .ok_or("Not connected to agent")?;

        tx.send((request_id.to_string(), option_id.to_string()))
            .map_err(|_| "Failed to resolve permission".to_string())
    }

    pub fn get_config_options(&self) -> Vec<SessionConfigOptionInfo> {
        self.config_options.clone()
    }

    pub async fn set_config_option(
        &mut self,
        config_id: &str,
        value: &str,
    ) -> Result<Vec<SessionConfigOptionInfo>, String> {
        let session_id = self.session_id.clone().ok_or("Not connected to agent")?;
        let tx = self
            .config_option_tx
            .as_ref()
            .ok_or("Not connected to agent")?;

        let (reply_tx, reply_rx) = oneshot::channel();

        tx.send(ConfigOptionCommand {
            session_id,
            config_id: config_id.to_string(),
            value: value.to_string(),
            reply: reply_tx,
        })
        .map_err(|_| "Failed to send config option request".to_string())?;

        let updated = reply_rx
            .await
            .map_err(|_| "Config option response channel closed".to_string())??;

        self.config_options = updated.clone();
        Ok(updated)
    }
}

/// Convert an ACP SessionConfigOption to our serializable info type.
fn convert_config_option(opt: &SessionConfigOption) -> SessionConfigOptionInfo {
    let category = opt.category.as_ref().map(|c| format!("{:?}", c));

    let (current_value, select_options) = match &opt.kind {
        SessionConfigKind::Select(select) => {
            let current = select.current_value.0.to_string();
            let opts = match &select.options {
                SessionConfigSelectOptions::Ungrouped(options) => {
                    SessionConfigSelectOptionsInfo::Ungrouped {
                        options: options
                            .iter()
                            .map(|o| SessionConfigSelectOptionInfo {
                                value: o.value.0.to_string(),
                                name: o.name.clone(),
                                description: o.description.clone(),
                            })
                            .collect(),
                    }
                }
                SessionConfigSelectOptions::Grouped(groups) => {
                    SessionConfigSelectOptionsInfo::Grouped {
                        groups: groups
                            .iter()
                            .map(|g| SessionConfigSelectGroupInfo {
                                group: g.group.0.to_string(),
                                name: g.name.clone(),
                                options: g
                                    .options
                                    .iter()
                                    .map(|o| SessionConfigSelectOptionInfo {
                                        value: o.value.0.to_string(),
                                        name: o.name.clone(),
                                        description: o.description.clone(),
                                    })
                                    .collect(),
                            })
                            .collect(),
                    }
                }
                _ => SessionConfigSelectOptionsInfo::Ungrouped { options: vec![] },
            };
            (current, opts)
        }
        _ => {
            return SessionConfigOptionInfo {
                id: opt.id.0.to_string(),
                name: opt.name.clone(),
                description: opt.description.clone(),
                category,
                current_value: String::new(),
                select_options: SessionConfigSelectOptionsInfo::Ungrouped { options: vec![] },
            }
        }
    };

    SessionConfigOptionInfo {
        id: opt.id.0.to_string(),
        name: opt.name.clone(),
        description: opt.description.clone(),
        category,
        current_value,
        select_options,
    }
}

/// Resolve a binary path for agent spawning.
/// If the binary is on PATH (or is "npx"), use it directly.
/// Otherwise check Buddio's agents install directory, with a legacy GoLaunch fallback.
fn resolve_binary_path(binary: &str, agent_id: &str) -> String {
    // If it looks like an absolute path or "npx", use as-is
    if binary == "npx" || std::path::Path::new(binary).is_absolute() {
        return binary.to_string();
    }

    // Check if on system PATH
    if super::registry::check_command_available(binary) {
        return binary.to_string();
    }

    // On Windows, npm-installed binaries are .cmd wrappers — try without .exe
    let bin_no_ext = binary.strip_suffix(".exe").unwrap_or(binary);
    if bin_no_ext != binary && super::registry::check_command_available(bin_no_ext) {
        return bin_no_ext.to_string();
    }

    // Check our install directory: AppData/Local/Buddio/agents/<agent_id>/<binary>
    if let Some(data_dir) = dirs::data_local_dir() {
        let buddio_candidate = data_dir
            .join("Buddio")
            .join("agents")
            .join(agent_id)
            .join(binary);
        if buddio_candidate.exists() {
            return buddio_candidate.to_string_lossy().to_string();
        }

        // Legacy fallback: AppData/Local/GoLaunch/agents/<agent_id>/<binary>
        let legacy_candidate = data_dir
            .join("GoLaunch")
            .join("agents")
            .join(agent_id)
            .join(binary);
        if legacy_candidate.exists() {
            return legacy_candidate.to_string_lossy().to_string();
        }
    }

    // Fallback: return as-is and let the OS resolve it
    binary.to_string()
}

/// Resolve the path to the `buddio-mcp` binary.
///
/// During development both `buddio-app` and `buddio-mcp` are compiled to the
/// same `target/{debug,release}` directory, so we look next to the current exe first.
fn resolve_mcp_binary_path() -> std::path::PathBuf {
    let bin_name = if cfg!(target_os = "windows") {
        "buddio-mcp.exe"
    } else {
        "buddio-mcp"
    };
    let legacy_bin_name = if cfg!(target_os = "windows") {
        "golaunch-mcp.exe"
    } else {
        "golaunch-mcp"
    };

    // Check next to the current executable (works for both dev and bundled)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let buddio_candidate = dir.join(bin_name);
            if buddio_candidate.exists() {
                return buddio_candidate;
            }

            let legacy_candidate = dir.join(legacy_bin_name);
            if legacy_candidate.exists() {
                return legacy_candidate;
            }
        }
    }

    // Fallback: assume it's on PATH.
    if super::registry::check_command_available(legacy_bin_name) {
        return std::path::PathBuf::from(legacy_bin_name);
    }

    std::path::PathBuf::from(bin_name)
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

/// Build a structured prompt for the ACP agent that includes system instructions,
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
              summarize, or transform selected text. ONLY then respond with just the rewritten text \
              (no explanation, no commentary). The launcher will offer a \"Replace selection\" button.\n\
           B) ACTION requests — when the user asks to add a command, open something, go somewhere, \
              or perform any action. Even if there is selected text, this is NOT a rewrite. \
              Add the item using the appropriate MCP tool and confirm what you did. The launcher will offer a \"Run\" button.\n\
         - CRITICAL for rewrites: Preserve the exact same format as the selected text. If the input is plain text, \
           return plain text. If it's code, return code without wrapping it in markdown code fences. \
           If it's HTML, return HTML. Never add markdown formatting (like ```), headers, or bullet points \
           unless the original selected text already uses that format. Your output will be pasted directly \
           in place of the selection, so it must be in the same format.\n\
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
         Commands (action_type `command` or `script`) are executed via `powershell -NoProfile -Command \"<action_value>\"` on Windows.\n\
         This means `action_value` MUST be valid PowerShell syntax. Important rules:\n\
         - Do NOT use `start powershell -NoExit ...` — `start` is an alias for `Start-Process` in PowerShell \
           and `Start-Process` does NOT have a `-NoExit` parameter.\n\
         - To run a command in a new visible PowerShell window that stays open, use:\n\
           `Start-Process powershell -ArgumentList '-NoExit', '-Command', '\"your commands here\"'`\n\
         - For fire-and-forget background commands (no window needed), just write the command directly:\n\
           `Set-Location \"path\"; docker compose up -d`\n\
         - Use single quotes for literal strings containing paths. Use Set-Location instead of cd when needed.\n\
         - Commands are spawned asynchronously (fire-and-forget) — the launcher does not wait for them to finish.\n\n\
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
        || launch_context.source_window_title.is_some();

    if has_context {
        p.push_str("## Current Context\n");
        if let Some(ref title) = launch_context.source_window_title {
            let process = launch_context
                .source_process_name
                .as_deref()
                .unwrap_or("unknown");
            p.push_str(&format!("Source application: {} ({})\n", title, process));
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
