mod acp;
mod commands;
mod context;

use commands::*;
use context::LaunchContext;
use std::env;
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tokio::sync::Mutex;

/// Returns true when GOLAUNCH_TEST=1 is set (used by E2E test harness).
fn is_test_mode() -> bool {
    env::var("GOLAUNCH_TEST").map_or(false, |v| v == "1")
}

use acp::manager::AcpManager;

/// Shared state holding the most recent launch context.
pub struct LaunchContextState(pub StdMutex<LaunchContext>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            search_items,
            get_all_items,
            execute_item,
            get_categories,
            hide_window,
            set_window_compact,
            get_setting,
            set_setting,
            get_agent_config,
            save_agent_config,
            acp_connect,
            acp_disconnect,
            acp_get_status,
            acp_prompt,
            acp_prompt_slash_command,
            acp_cancel,
            acp_resolve_permission,
            acp_get_config_options,
            acp_set_config_option,
            acp_fetch_registry,
            acp_check_agents_installed,
            acp_install_agent,
            get_agent_env,
            set_agent_env,
            create_conversation,
            list_conversations,
            get_conversation_messages,
            add_conversation_message,
            search_conversations,
            delete_conversation,
            record_command,
            get_command_suggestions,
            add_item_from_suggestion,
            get_memories,
            add_memory_cmd,
            remove_memory,
            get_memory_by_key,
            get_relevant_memories,
            get_launch_context,
            type_text_to_app,
            replace_selection_text,
            record_rewrite,
            get_rewrite_suggestions,
            list_slash_commands,
            search_slash_commands,
            get_slash_command_by_name,
            add_slash_command,
            remove_slash_command,
            execute_slash_command,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize ACP manager state
            app.manage(AcpState(Arc::new(Mutex::new(AcpManager::new()))));

            // Initialize launch context state
            app.manage(LaunchContextState(StdMutex::new(LaunchContext::default())));

            // In test mode, skip global shortcut registration and keep window visible
            if is_test_mode() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                return Ok(());
            }

            // Register global shortcut: Option+Space on macOS, Ctrl+Space on Windows/Linux
            let shortcut = if cfg!(target_os = "macos") {
                Shortcut::new(Some(Modifiers::ALT), Code::Space)
            } else {
                Shortcut::new(Some(Modifiers::CONTROL), Code::Space)
            };
            app.handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())
                .unwrap_or(());

            let handle_clone = handle.clone();

            if let Some(window) = app.get_webview_window("main") {
                let handle_on_close = handle.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = handle_on_close.emit("launcher-reset", ());
                        if let Some(main) = handle_on_close.get_webview_window("main") {
                            let _ = main.hide();
                        }
                    }
                });
            }

            app.global_shortcut()
                .on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    if let Some(window) = handle_clone.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = handle_clone.emit("launcher-reset", ());
                            let _ = window.hide();
                        } else {
                            // Capture context BEFORE showing the launcher (while source app has focus)
                            let ctx = context::capture_launch_context();
                            if let Some(state) = handle_clone.try_state::<LaunchContextState>() {
                                if let Ok(mut lock) = state.0.lock() {
                                    *lock = ctx.clone();
                                }
                            }
                            // Emit the context to the frontend
                            let _ = handle_clone.emit("launch-context", &ctx);

                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.center();
                        }
                    }
                })?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running GoLaunch");
}
