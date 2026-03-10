use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use super::provider::AgentProvider;
use super::types::{
    AgentConfig, AgentStatus, AgentUpdate, PermissionOptionInfo, PermissionRequest,
    SessionConfigOptionInfo,
};

// --- JSON lines protocol types (Node → Rust) ---

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SidecarEvent {
    StatusChange {
        status: String,
    },
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
    PermissionRequest {
        request_id: String,
        session_id: String,
        tool_name: String,
        options: Vec<SidecarPermOption>,
    },
    UserQuestion {
        request_id: String,
        tool_use_id: String,
        questions: Vec<SidecarQuestionItem>,
    },
    TurnComplete {
        stop_reason: String,
    },
    ReplaceSelectionRequest {
        request_id: String,
        text: String,
    },
    AuthStatus {
        is_authenticating: bool,
        auth_url: Option<String>,
        error: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize)]
struct SidecarPermOption {
    option_id: String,
    name: String,
    kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SidecarQuestionItem {
    question: String,
    header: String,
    options: Vec<SidecarQuestionOption>,
    #[serde(rename = "multiSelect")]
    multi_select: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SidecarQuestionOption {
    label: String,
    description: String,
}

// --- JSON lines protocol types (Rust → Node) ---

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SidecarCommand {
    Init {
        provider: String,
        config: SidecarConfig,
        mcp_binary: String,
    },
    Prompt {
        text: String,
    },
    Cancel,
    ResolvePermission {
        request_id: String,
        option_id: String,
    },
    ResolveQuestion {
        request_id: String,
        answers: std::collections::HashMap<String, String>,
    },
    ResolveReplaceSelection {
        request_id: String,
        success: bool,
    },
    Shutdown,
}

#[derive(Debug, Serialize)]
struct SidecarConfig {
    api_key: String,
    model: String,
    auth_method: String,
}

/// Provider for Claude and Copilot SDKs via a Node.js sidecar process.
pub struct SidecarProvider {
    provider_name: String,
    status: AgentStatus,
    stdin_tx: Option<mpsc::UnboundedSender<SidecarCommand>>,
    shutdown_tx: Option<mpsc::UnboundedSender<()>>,
}

impl SidecarProvider {
    pub fn new(provider_name: &str) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            status: AgentStatus::Disconnected,
            stdin_tx: None,
            shutdown_tx: None,
        }
    }

    fn send_command(&self, cmd: SidecarCommand) -> Result<(), String> {
        let tx = self
            .stdin_tx
            .as_ref()
            .ok_or("Sidecar not connected")?;
        tx.send(cmd)
            .map_err(|_| "Failed to send command to sidecar".to_string())
    }
}

#[async_trait]
impl AgentProvider for SidecarProvider {
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

        // Find Node.js
        if !super::registry::check_command_available("node") {
            self.status = AgentStatus::Error;
            return Err(
                "Node.js is required for Claude/Copilot providers but was not found on PATH. \
                 Please install Node.js (https://nodejs.org/) and try again."
                    .to_string(),
            );
        }

        // Resolve sidecar script path — look next to the executable first,
        // then fall back to a development path relative to the project root.
        let sidecar_script = resolve_sidecar_path()?;

        // Resolve buddio-mcp binary
        let mcp_binary = super::acp::resolve_mcp_binary_path()
            .to_string_lossy()
            .to_string();

        // Spawn Node.js sidecar process
        let mut cmd = tokio::process::Command::new("node");
        cmd.arg(&sidecar_script);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn sidecar: {e}"))?;

        let child_stdin = child.stdin.take().ok_or("Failed to get sidecar stdin")?;
        let child_stdout = child.stdout.take().ok_or("Failed to get sidecar stdout")?;

        // Channel for sending commands to the stdin writer task
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<SidecarCommand>();
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel::<()>();

        // Oneshot for init completion
        let (init_tx, init_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

        let provider_name = self.provider_name.to_lowercase();
        let api_key = config.api_key.clone();
        let model = config.model.clone();
        let auth_method = config.auth_method.clone();
        let app_clone = app.clone();

        // Spawn stdin writer task
        tokio::spawn(async move {
            let mut stdin = child_stdin;
            while let Some(cmd) = stdin_rx.recv().await {
                let json = match serde_json::to_string(&cmd) {
                    Ok(j) => j,
                    Err(e) => {
                        eprintln!("Failed to serialize sidecar command: {e}");
                        continue;
                    }
                };
                if stdin
                    .write_all(format!("{json}\n").as_bytes())
                    .await
                    .is_err()
                {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        // Spawn stdout reader task
        let stdin_tx_for_init = stdin_tx.clone();
        let stdin_tx_for_replace = stdin_tx.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};

            let reader = BufReader::new(child_stdout);
            let mut lines = reader.lines();
            let mut init_tx = Some(init_tx);

            // Send init command
            let _ = stdin_tx_for_init.send(SidecarCommand::Init {
                provider: provider_name,
                config: SidecarConfig {
                    api_key,
                    model,
                    auth_method,
                },
                mcp_binary,
            });

            while let Ok(Some(line)) = lines.next_line().await {
                let event: SidecarEvent = match serde_json::from_str(&line) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Failed to parse sidecar event: {e}: {line}");
                        continue;
                    }
                };

                match event {
                    SidecarEvent::StatusChange { status } => {
                        match status.as_str() {
                            "connected" => {
                                let _ = app_clone.emit(
                                    "agent-update",
                                    AgentUpdate::StatusChange {
                                        status: AgentStatus::Connected,
                                    },
                                );
                                if let Some(tx) = init_tx.take() {
                                    let _ = tx.send(Ok(()));
                                }
                            }
                            "error" => {
                                let _ = app_clone.emit(
                                    "agent-update",
                                    AgentUpdate::StatusChange {
                                        status: AgentStatus::Error,
                                    },
                                );
                                if let Some(tx) = init_tx.take() {
                                    let _ = tx.send(Err("Sidecar reported error".to_string()));
                                }
                            }
                            _ => {}
                        }
                    }
                    SidecarEvent::MessageChunk { text } => {
                        let _ = app_clone.emit(
                            "agent-update",
                            AgentUpdate::MessageChunk { text },
                        );
                    }
                    SidecarEvent::ThoughtChunk { text } => {
                        let _ = app_clone.emit(
                            "agent-update",
                            AgentUpdate::ThoughtChunk { text },
                        );
                    }
                    SidecarEvent::ToolCall {
                        id,
                        title,
                        kind,
                        content,
                    } => {
                        let _ = app_clone.emit(
                            "agent-update",
                            AgentUpdate::ToolCall {
                                id,
                                title,
                                kind,
                                content,
                            },
                        );
                    }
                    SidecarEvent::ToolCallUpdate { id, title, status } => {
                        let _ = app_clone.emit(
                            "agent-update",
                            AgentUpdate::ToolCallUpdate { id, title, status },
                        );
                    }
                    SidecarEvent::PermissionRequest {
                        request_id,
                        session_id,
                        tool_name,
                        options,
                    } => {
                        let _ = app_clone.emit(
                            "agent-permission-request",
                            PermissionRequest {
                                request_id,
                                session_id,
                                tool_name,
                                tool_description: None,
                                command_preview: None,
                                options: options
                                    .into_iter()
                                    .map(|o| PermissionOptionInfo {
                                        option_id: o.option_id,
                                        name: o.name,
                                        kind: o.kind,
                                    })
                                    .collect(),
                            },
                        );
                    }
                    SidecarEvent::UserQuestion {
                        request_id,
                        tool_use_id,
                        questions,
                    } => {
                        let _ = app_clone.emit(
                            "agent-user-question",
                            crate::agent::types::UserQuestionRequest {
                                request_id,
                                tool_use_id,
                                questions: questions
                                    .into_iter()
                                    .map(|q| crate::agent::types::UserQuestionItem {
                                        question: q.question,
                                        header: q.header,
                                        options: q
                                            .options
                                            .into_iter()
                                            .map(|o| {
                                                crate::agent::types::UserQuestionOption {
                                                    label: o.label,
                                                    description: o.description,
                                                }
                                            })
                                            .collect(),
                                        multi_select: q.multi_select,
                                    })
                                    .collect(),
                            },
                        );
                    }
                    SidecarEvent::TurnComplete { stop_reason } => {
                        let _ = app_clone.emit(
                            "agent-update",
                            AgentUpdate::TurnComplete { stop_reason },
                        );
                    }
                    SidecarEvent::ReplaceSelectionRequest { request_id, text } => {
                        let stdin_tx = stdin_tx_for_replace.clone();
                        let app_for_replace = app_clone.clone();
                        tokio::spawn(async move {
                            // Hide the window first so focus returns to source app
                            if let Some(window) = app_for_replace.get_webview_window("main") {
                                let _ = window.hide();
                            }

                            // Small delay for window focus to return
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

                            // Perform the replacement
                            let success = tokio::task::spawn_blocking(move || {
                                crate::context::replace_selection(&text).is_ok()
                            })
                            .await
                            .unwrap_or(false);

                            let _ = stdin_tx.send(SidecarCommand::ResolveReplaceSelection {
                                request_id,
                                success,
                            });
                        });
                    }
                    SidecarEvent::AuthStatus {
                        auth_url,
                        error,
                        ..
                    } => {
                        if let Some(url) = &auth_url {
                            if let Err(e) = open::that(url) {
                                eprintln!("Failed to open auth URL: {e}");
                            }
                        }
                        if let Some(err) = &error {
                            eprintln!("Auth error: {err}");
                        }
                    }
                    SidecarEvent::Error { message } => {
                        eprintln!("Sidecar error: {message}");
                        if let Some(tx) = init_tx.take() {
                            // Init phase — report via oneshot
                            let _ = tx.send(Err(message));
                        } else {
                            // Runtime phase — forward error as a message chunk
                            // so the user can see what went wrong
                            let _ = app_clone.emit(
                                "agent-update",
                                AgentUpdate::MessageChunk {
                                    text: format!("\n\n**Error:** {message}"),
                                },
                            );
                        }
                    }
                }
            }

            // Stdout closed — sidecar exited
            let _ = child.wait().await;
        });

        // Spawn shutdown handler
        let stdin_tx_for_shutdown = stdin_tx.clone();
        tokio::spawn(async move {
            shutdown_rx.recv().await;
            let _ = stdin_tx_for_shutdown.send(SidecarCommand::Shutdown);
        });

        // Wait for init to complete
        let init_result = init_rx
            .await
            .map_err(|_| "Sidecar init channel closed".to_string())?;

        match init_result {
            Ok(()) => {
                self.status = AgentStatus::Connected;
                self.stdin_tx = Some(stdin_tx);
                self.shutdown_tx = Some(shutdown_tx);
                Ok(())
            }
            Err(e) => {
                self.status = AgentStatus::Error;
                Err(e)
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), String> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.stdin_tx.take();
        self.status = AgentStatus::Disconnected;
        Ok(())
    }

    fn status(&self) -> AgentStatus {
        self.status
    }

    fn send_prompt(&self, content: String) -> Result<(), String> {
        self.send_command(SidecarCommand::Prompt { text: content })
    }

    async fn cancel(&mut self) -> Result<(), String> {
        self.send_command(SidecarCommand::Cancel)
    }

    fn resolve_permission(&self, request_id: &str, option_id: &str) -> Result<(), String> {
        self.send_command(SidecarCommand::ResolvePermission {
            request_id: request_id.to_string(),
            option_id: option_id.to_string(),
        })
    }

    fn resolve_question(
        &self,
        request_id: &str,
        answers: std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        self.send_command(SidecarCommand::ResolveQuestion {
            request_id: request_id.to_string(),
            answers,
        })
    }

    fn config_options(&self) -> Vec<SessionConfigOptionInfo> {
        Vec::new()
    }

    async fn set_config_option(
        &mut self,
        _config_id: &str,
        _value: &str,
    ) -> Result<Vec<SessionConfigOptionInfo>, String> {
        Ok(Vec::new())
    }
}

/// Resolve the path to the sidecar `dist/index.js` script.
///
/// Checks in order:
/// 1. Next to the current executable (bundled app: `<exe_dir>/sidecar/index.js`)
/// 2. Development path relative to project root (`sidecar/dist/index.js`)
fn resolve_sidecar_path() -> Result<String, String> {
    // 1. Bundled: next to the current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("sidecar").join("index.js");
            if bundled.exists() {
                return Ok(bundled.to_string_lossy().to_string());
            }
        }
    }

    // 2. Development: relative to Cargo manifest dir → ../../sidecar/dist/index.js
    //    or from the working directory
    let cwd_candidate = std::path::PathBuf::from("sidecar/dist/index.js");
    if cwd_candidate.exists() {
        return Ok(cwd_candidate.to_string_lossy().to_string());
    }

    // 3. Try relative to the exe's grandparent (common in dev builds)
    if let Ok(exe) = std::env::current_exe() {
        // exe is typically in target/debug/buddio-app.exe
        // sidecar is at project_root/sidecar/dist/index.js
        if let Some(target_dir) = exe.parent() {
            // target/debug -> target -> project_root
            if let Some(project_root) = target_dir.parent().and_then(|p| p.parent()) {
                let dev_candidate = project_root.join("sidecar").join("dist").join("index.js");
                if dev_candidate.exists() {
                    return Ok(dev_candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    Err(
        "Sidecar script not found. Please build the sidecar first: cd sidecar && npm run build"
            .to_string(),
    )
}
