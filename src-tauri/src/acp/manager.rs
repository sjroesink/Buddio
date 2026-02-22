use agent_client_protocol::{
    Agent, ClientCapabilities, ClientSideConnection, ContentBlock, Implementation,
    InitializeRequest, NewSessionRequest, PermissionOptionId, ProtocolVersion,
    RequestPermissionOutcome, SelectedPermissionOutcome, SessionConfigId, SessionConfigKind,
    SessionConfigOption, SessionConfigSelectOptions, SessionConfigValueId, SessionId,
    SetSessionConfigOptionRequest, TextContent,
};
use golaunch_core::{
    CommandHistory, CommandSuggestion, Conversation, ConversationMessage, Item, Memory,
    SlashCommand,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::client::GoLaunchClient;
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
        // our install directory (AppData/Local/GoLaunch/agents/<agent_id>/)
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
                let acp_client = GoLaunchClient::new(update_tx.clone(), permission_tx);
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
                            .client_info(Implementation::new("GoLaunch", "0.1.0"))
                            .client_capabilities(ClientCapabilities::new()),
                    )
                    .await;

                if let Err(e) = init_result {
                    let _ = session_tx.send(Err(format!("Initialize failed: {e:?}")));
                    return;
                }

                // Create a new session
                let cwd = std::env::current_dir().unwrap_or_else(|_| "/".into());
                let session_result = connection.new_session(NewSessionRequest::new(cwd)).await;

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

/// Resolve the path to the golaunch-cli binary.
/// Looks next to the current executable first (production install),
/// then falls back to just the binary name (assumes PATH).
fn resolve_cli_path() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cli_name = if cfg!(target_os = "windows") {
                "golaunch-cli.exe"
            } else {
                "golaunch-cli"
            };
            let candidate = dir.join(cli_name);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    "golaunch-cli".to_string()
}

/// Resolve a binary path for agent spawning.
/// If the binary is on PATH (or is "npx"), use it directly.
/// Otherwise check the GoLaunch agents install directory.
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

    // Check our install directory: AppData/Local/GoLaunch/agents/<agent_id>/<binary>
    if let Some(data_dir) = dirs::data_local_dir() {
        let candidate = data_dir
            .join("GoLaunch")
            .join("agents")
            .join(agent_id)
            .join(binary);
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

    // Fallback: return as-is and let the OS resolve it
    binary.to_string()
}

/// Build a focused prompt for the slash command creation agent.
/// This agent's sole purpose is to collaborate with the user to create a script,
/// register it as a slash command, and execute it.
fn build_slash_command_prompt(
    query: &str,
    memories: &[Memory],
    slash_commands: &[SlashCommand],
) -> String {
    let cli = resolve_cli_path();
    let slash_dir = golaunch_core::Database::slash_commands_dir()
        .unwrap_or_else(|_| "slash-commands".into())
        .to_string_lossy()
        .to_string();

    let mut p = String::with_capacity(2048);

    // ── System instructions ──
    p.push_str(
        "You are GoLaunch Slash Command Builder — a dedicated agent for creating and executing \
         user-defined slash commands. The user typed a slash command that doesn't exist yet.\n\n\
         Your ONLY job:\n\
         1. Figure out what the script should do based on the command name and arguments.\n\
            For example: `/kill 4924` → a script that kills the process running on port 4924.\n\
         2. Briefly confirm with the user what the script will do.\n\
         3. Write the script file to the slash-commands directory.\n\
         4. Register it with the CLI using `slash-commands add`.\n\
         5. Execute it immediately using `slash-commands run`.\n\n\
         Guidelines:\n\
         - Be smart about inferring the purpose from the command name — the user expects you to understand.\n\
         - Be concise — the user is in a launcher and wants quick results.\n\
         - Make scripts robust: validate arguments, handle errors, produce clear output.\n\
         - If the purpose is ambiguous, ask ONE clarifying question, then proceed.\n\
         - After creating and executing, briefly confirm what happened.\n\n",
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

    // ── CLI reference (slash commands only) ──
    p.push_str(&format!(
        "## CLI Reference\n\
         Binary: `{cli}`\n\
         Script storage directory: `{slash_dir}`\n\n\
         ```bash\n\
         # List all slash commands\n\
         \"{cli}\" slash-commands list --json\n\n\
         # Register a new slash command\n\
         \"{cli}\" slash-commands add --name \"kill\" --description \"Kill process on port\" --script-path \"{slash_dir}/kill.ps1\"\n\n\
         # Run a slash command with arguments\n\
         \"{cli}\" slash-commands run --name \"kill\" --args \"4924\"\n\n\
         # Get a slash command by name\n\
         \"{cli}\" slash-commands get --name \"kill\"\n\n\
         # Remove a slash command\n\
         \"{cli}\" slash-commands remove --name \"kill\"\n\
         ```\n\n\
         Workflow: first write the script file to disk, then register with `slash-commands add`, \
         then execute with `slash-commands run`.\n\n"
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
/// CLI reference, user context, and the query.
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
    let cli = resolve_cli_path();
    let db_path = golaunch_core::Database::db_path()
        .unwrap_or_else(|_| "golaunch.db".into())
        .to_string_lossy()
        .to_string();

    let mut p = String::with_capacity(4096);

    // ── System instructions ──
    p.push_str(
        "You are GoLaunch Assistant, an AI helper embedded in GoLaunch — a keyboard-driven \
         launcher application. The user typed a search query that didn't match any of their \
         predefined commands, so they're asking you for help.\n\n\
         Your capabilities:\n\
         1. Directly add, update, or remove launcher commands using the GoLaunch CLI\n\
         2. Query the launcher database to find commands, history, and memories\n\
         3. Manage the user's persistent memory (preferences, facts, patterns)\n\
         4. Help the user figure out what command they need\n\
         5. Answer questions about tools, CLIs, and workflows\n\n\
         IMPORTANT — Action-oriented behavior:\n\
         - When the user wants to add a command, DO IT immediately with the CLI. Don't just suggest it.\n\
         - When the user asks about their setup, QUERY the database first, then answer.\n\
         - If the query is ambiguous, CHECK memory and existing commands first before asking clarifying questions.\n\
         - Treat memory facts as authoritative context (e.g., if a name maps to a project, use that meaning).\n\
         - Reading memory/list/history is a safe lookup step and should be done proactively without requesting permission.\n\
         - When you learn something about the user's preferences, SAVE it to memory.\n\
         - After making changes, briefly confirm what you did.\n\
         - Be concise — the user is in a launcher and wants quick results.\n\
         - If the query looks like a command (e.g. \"npm install\", \"docker compose up\"), \
           add it as a launcher item proactively.\n\
         - You have access to the user's current context: selected text, clipboard, and source application.\n\
         - IMPORTANT: Distinguish between two types of requests:\n\
           A) REWRITE requests — when the user explicitly asks to rewrite, rephrase, translate, \
              summarize, or transform selected text. ONLY then respond with just the rewritten text \
              (no explanation, no commentary). The launcher will offer a \"Replace selection\" button.\n\
           B) ACTION requests — when the user asks to add a command, open something, go somewhere, \
              or perform any action. Even if there is selected text, this is NOT a rewrite. \
              Add the item using the CLI and confirm what you did. The launcher will offer a \"Run\" button.\n\
         - CRITICAL for rewrites: Preserve the exact same format as the selected text. If the input is plain text, \
           return plain text. If it's code, return code without wrapping it in markdown code fences. \
           If it's HTML, return HTML. Never add markdown formatting (like ```), headers, or bullet points \
           unless the original selected text already uses that format. Your output will be pasted directly \
           in place of the selection, so it must be in the same format.\n\
         - When working with selected text, consider the source application for appropriate formatting.\n\n",
    );

    // ── CLI Reference ──
    p.push_str(&format!(
        "## GoLaunch CLI\n\
         Binary: `{cli}`\n\
         Database: `{db_path}`\n\n\
         ### Commands (Items)\n\
         ```bash\n\
         # Add a new launcher command\n\
         \"{cli}\" add --title \"Title\" --action-value \"the_command\" --action-type command --category \"Category\"\n\
         # Optional flags: --subtitle \"desc\" --icon \"emoji\" --tags \"t1,t2\"\n\n\
         # Add a URL shortcut\n\
         \"{cli}\" add --title \"Google\" --action-value \"https://google.com\" --action-type url --category \"Web\"\n\n\
         # List all commands (use --json for structured output)\n\
         \"{cli}\" list\n\
         \"{cli}\" list --category \"Development\" --json\n\n\
         # Search commands\n\
         \"{cli}\" search \"query\" --json\n\n\
         # Update a command by ID\n\
         \"{cli}\" update <id> --title \"New Title\" --action-value \"new_cmd\" --category \"Cat\"\n\n\
         # Remove a command by ID\n\
         \"{cli}\" remove <id>\n\
         ```\n\
         Action types: `command` (shell), `url` (browser), `script` (script file)\n\n\
         ### Memory\n\
         ```bash\n\
         # Store a preference or fact\n\
         \"{cli}\" memory add --key \"preferred_editor\" --value \"vscode\" --type preference\n\
         \"{cli}\" memory add --key \"project_dir\" --value \"D:\\\\Projects\" --type fact --context \"work\"\n\n\
         # Query memories\n\
         \"{cli}\" memory list --json\n\
         \"{cli}\" memory list --type preference\n\
         \"{cli}\" memory search \"editor\"\n\
         \"{cli}\" memory get \"preferred_editor\"\n\n\
         # Remove a memory\n\
         \"{cli}\" memory remove <id>\n\
         ```\n\
         Memory types: `preference` (user preference), `pattern` (learned behavior), `fact` (stored info)\n\n\
         ### History\n\
         ```bash\n\
         \"{cli}\" history              # last 20 commands\n\
         \"{cli}\" history --limit 50\n\
         \"{cli}\" history --search \"docker\" --json\n\
         ```\n\n\
         ### Import / Export\n\
         ```bash\n\
         \"{cli}\" export --output commands.json\n\
         \"{cli}\" import commands.json\n\
         ```\n\n\
         ### Conversations\n\
         ```bash\n\
         # List recent conversations\n\
         \"{cli}\" conversations list --limit 20 --json\n\n\
         # Search conversations by content\n\
         \"{cli}\" conversations search \"query\" --json\n\n\
         # Show a conversation with all messages\n\
         \"{cli}\" conversations show <id> --json\n\n\
         # Get recent conversation context (formatted summary)\n\
         \"{cli}\" conversations context --limit 5\n\
         ```\n\
         Use conversation commands to recall earlier discussions with the user.\n\n\
         ### Slash Commands\n\
         Slash commands are user-defined scripts invoked with `/name args...` from the launcher.\n\
         Creation of new slash commands is handled by a dedicated agent — you do not need to create them.\n\
         You can reference existing slash commands listed below for context.\n\n"
    ));

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
        p.push_str("Summary of recent conversations with this user (use `conversations show <id>` for full details):\n\n");
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
