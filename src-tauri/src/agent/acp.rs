use agent_client_protocol::{
    Agent, ClientCapabilities, ClientSideConnection, ContentBlock, Implementation,
    InitializeRequest, McpServer, McpServerStdio, NewSessionRequest, PermissionOptionId,
    ProtocolVersion, RequestPermissionOutcome, SelectedPermissionOutcome, SessionConfigId,
    SessionConfigKind, SessionConfigOption, SessionConfigSelectOptions, SessionConfigValueId,
    SessionId, SetSessionConfigOptionRequest, TextContent,
};
use async_trait::async_trait;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::acp_client::BuddioClient;
use super::provider::AgentProvider;
use super::types::{
    AgentConfig, AgentStatus, AgentUpdate, PermissionRequest, SessionConfigOptionInfo,
    SessionConfigSelectGroupInfo, SessionConfigSelectOptionInfo, SessionConfigSelectOptionsInfo,
};

pub struct AcpProvider {
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

impl AcpProvider {
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
}

#[async_trait]
impl AgentProvider for AcpProvider {
    async fn connect(&mut self, config: &AgentConfig, app: AppHandle) -> Result<(), String> {
        if self.status == AgentStatus::Connected {
            return Ok(());
        }

        self.status = AgentStatus::Connecting;
        let _ = app.emit(
            "agent-update",
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
        // that cannot be spawned directly. We run them through PowerShell.
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = tokio::process::Command::new("powershell");
            let invoke = std::iter::once(format!("& '{}'", resolved_binary))
                .chain(args.iter().map(|a| format!("'{}'", a.replace('\'', "''"))))
                .collect::<Vec<_>>()
                .join(" ");
            c.args(["-NoProfile", "-Command", &invoke]);
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
            "agent-update",
            AgentUpdate::StatusChange {
                status: AgentStatus::Connected,
            },
        );

        // Emit initial config options if any
        if !self.config_options.is_empty() {
            let _ = app.emit("agent-config-options", &self.config_options);
        }

        // Spawn background tasks to forward updates and permissions to Tauri events
        let app_for_updates = app.clone();
        tokio::spawn(async move {
            while let Some(update) = update_rx.recv().await {
                let _ = app_for_updates.emit("agent-update", &update);
            }
        });

        let app_for_perms = app;
        tokio::spawn(async move {
            while let Some(perm) = permission_rx.recv().await {
                let _ = app_for_perms.emit("agent-permission-request", &perm);
            }
        });

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), String> {
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

    fn status(&self) -> AgentStatus {
        self.status
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

    async fn cancel(&mut self) -> Result<(), String> {
        let cancel_tx = self.cancel_tx.as_ref().ok_or("Not connected to agent")?;

        cancel_tx
            .send(())
            .map_err(|_| "Failed to send cancel to agent".to_string())
    }

    fn resolve_permission(&self, request_id: &str, option_id: &str) -> Result<(), String> {
        let tx = self
            .permission_resolve_tx
            .as_ref()
            .ok_or("Not connected to agent")?;

        tx.send((request_id.to_string(), option_id.to_string()))
            .map_err(|_| "Failed to resolve permission".to_string())
    }

    fn config_options(&self) -> Vec<SessionConfigOptionInfo> {
        self.config_options.clone()
    }

    async fn set_config_option(
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
pub(crate) fn resolve_binary_path(binary: &str, agent_id: &str) -> String {
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
pub(crate) fn resolve_mcp_binary_path() -> std::path::PathBuf {
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
