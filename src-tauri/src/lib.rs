mod agent;
mod commands;
mod context;
pub mod hotkey;

use commands::*;
use context::LaunchContext;
use std::env;
use std::sync::{Arc, Mutex as StdMutex};
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};
use tokio::sync::Mutex;

/// Returns true when BUDDIO_TEST=1 is set (used by E2E test harness).
/// Accepts legacy GOLAUNCH_TEST for backward compatibility.
fn is_test_mode() -> bool {
    env::var("BUDDIO_TEST")
        .or_else(|_| env::var("GOLAUNCH_TEST"))
        .is_ok_and(|v| v == "1")
}

use agent::manager::AgentManager;

/// Shared state holding the most recent launch context.
pub struct LaunchContextState(pub StdMutex<LaunchContext>);

/// Shared state for the hotkey manager.
pub struct HotkeyState(pub StdMutex<hotkey::HotkeyManager>);

const TRAY_OPEN_LAUNCHER_ID: &str = "tray_open_launcher";
const TRAY_OPEN_SETTINGS_ID: &str = "tray_open_settings";

fn show_launcher_from_tray(app: &tauri::AppHandle, open_settings: bool) {
    hotkey::show_launcher(app);
    if open_settings {
        let _ = app.emit("open-settings", ());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            agent_connect,
            agent_disconnect,
            agent_get_status,
            agent_prompt,
            agent_prompt_slash_command,
            agent_cancel,
            agent_resolve_permission,
            agent_get_config_options,
            agent_set_config_option,
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
            get_slash_command_params,
            get_shortcut_mode,
            set_shortcut_mode,
            check_for_update,
            install_update,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize agent manager state
            app.manage(AgentState(Arc::new(Mutex::new(AgentManager::new()))));

            // Initialize launch context state
            app.manage(LaunchContextState(StdMutex::new(LaunchContext::default())));

            // Initialize hotkey manager state
            let mut hotkey_mgr = hotkey::HotkeyManager::new();

            let tray_menu = MenuBuilder::new(app)
                .text(TRAY_OPEN_LAUNCHER_ID, "Launcher openen")
                .text(TRAY_OPEN_SETTINGS_ID, "Settings openen")
                .build()?;

            let mut tray_builder = TrayIconBuilder::with_id("buddio-tray")
                .menu(&tray_menu)
                .tooltip("Buddio")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id() == TRAY_OPEN_LAUNCHER_ID {
                        show_launcher_from_tray(app, false);
                    } else if event.id() == TRAY_OPEN_SETTINGS_ID {
                        show_launcher_from_tray(app, true);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_launcher_from_tray(tray.app_handle(), false);
                    }
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder.build(app)?;

            // In test mode, skip global shortcut registration and keep window visible
            if is_test_mode() {
                app.manage(HotkeyState(StdMutex::new(hotkey_mgr)));
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                return Ok(());
            }

            if let Some(window) = app.get_webview_window("main") {
                let handle_on_close = handle.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = handle_on_close.emit("launcher-reset", ());
                        if let Some(main) = handle_on_close.get_webview_window("main") {
                            let _ = main.hide();
                        }
                    }
                    _ => {}
                });
            }

            // Load saved shortcut preference and activate
            let saved_shortcut = hotkey::HotkeyManager::load_saved_shortcut();
            hotkey_mgr
                .activate(&handle, saved_shortcut)
                .unwrap_or_else(|e| eprintln!("Failed to activate shortcut: {e}"));

            app.manage(HotkeyState(StdMutex::new(hotkey_mgr)));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Buddio");
}
