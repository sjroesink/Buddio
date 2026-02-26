use golaunch_core::Database;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::context;
use crate::LaunchContextState;

/// The setting key used to persist the shortcut.
pub const SETTING_KEY: &str = "shortcut.mode";

/// Default shortcut when none is configured.
const DEFAULT_SHORTCUT: &str = "Ctrl+Space";

// ---------------------------------------------------------------------------
// Toggle launcher (shared between all activation methods)
// ---------------------------------------------------------------------------

pub fn toggle_launcher(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = handle.emit("launcher-reset", ());
            let _ = window.hide();
        } else {
            show_launcher(handle);
        }
    }
}

/// Shows the launcher window and refreshes launch context.
pub fn show_launcher(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let ctx = context::capture_launch_context();
        if let Some(state) = handle.try_state::<LaunchContextState>() {
            if let Ok(mut lock) = state.0.lock() {
                *lock = ctx.clone();
            }
        }
        let _ = handle.emit("launch-context", &ctx);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.center();
    }
}

// ---------------------------------------------------------------------------
// Shortcut string parsing
// ---------------------------------------------------------------------------

fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return Err("Empty shortcut string".to_string());
    }

    let mut modifiers = Modifiers::empty();
    let key_str = *parts.last().unwrap();

    for &part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "meta" | "super" | "win" => modifiers |= Modifiers::META,
            _ => return Err(format!("Unknown modifier: {}", part)),
        }
    }

    let code = str_to_code(key_str)?;
    let mods = if modifiers.is_empty() {
        None
    } else {
        Some(modifiers)
    };
    Ok(Shortcut::new(mods, code))
}

fn str_to_code(s: &str) -> Result<Code, String> {
    // Try single letter / digit first
    if s.len() == 1 {
        let ch = s.chars().next().unwrap().to_ascii_uppercase();
        match ch {
            'A' => return Ok(Code::KeyA),
            'B' => return Ok(Code::KeyB),
            'C' => return Ok(Code::KeyC),
            'D' => return Ok(Code::KeyD),
            'E' => return Ok(Code::KeyE),
            'F' => return Ok(Code::KeyF),
            'G' => return Ok(Code::KeyG),
            'H' => return Ok(Code::KeyH),
            'I' => return Ok(Code::KeyI),
            'J' => return Ok(Code::KeyJ),
            'K' => return Ok(Code::KeyK),
            'L' => return Ok(Code::KeyL),
            'M' => return Ok(Code::KeyM),
            'N' => return Ok(Code::KeyN),
            'O' => return Ok(Code::KeyO),
            'P' => return Ok(Code::KeyP),
            'Q' => return Ok(Code::KeyQ),
            'R' => return Ok(Code::KeyR),
            'S' => return Ok(Code::KeyS),
            'T' => return Ok(Code::KeyT),
            'U' => return Ok(Code::KeyU),
            'V' => return Ok(Code::KeyV),
            'W' => return Ok(Code::KeyW),
            'X' => return Ok(Code::KeyX),
            'Y' => return Ok(Code::KeyY),
            'Z' => return Ok(Code::KeyZ),
            '0' => return Ok(Code::Digit0),
            '1' => return Ok(Code::Digit1),
            '2' => return Ok(Code::Digit2),
            '3' => return Ok(Code::Digit3),
            '4' => return Ok(Code::Digit4),
            '5' => return Ok(Code::Digit5),
            '6' => return Ok(Code::Digit6),
            '7' => return Ok(Code::Digit7),
            '8' => return Ok(Code::Digit8),
            '9' => return Ok(Code::Digit9),
            // For punctuation like ";", "/", ".", etc. fall through to the
            // general match below which handles them by name or symbol.
            _ => {}
        }
    }

    // General match — handles multi-char names and single-char punctuation
    match s.to_lowercase().as_str() {
        "space" => Ok(Code::Space),
        "enter" | "return" => Ok(Code::Enter),
        "tab" => Ok(Code::Tab),
        "escape" | "esc" => Ok(Code::Escape),
        "backspace" => Ok(Code::Backspace),
        "delete" | "del" => Ok(Code::Delete),
        "insert" => Ok(Code::Insert),
        "home" => Ok(Code::Home),
        "end" => Ok(Code::End),
        "pageup" => Ok(Code::PageUp),
        "pagedown" => Ok(Code::PageDown),
        "arrowup" | "up" => Ok(Code::ArrowUp),
        "arrowdown" | "down" => Ok(Code::ArrowDown),
        "arrowleft" | "left" => Ok(Code::ArrowLeft),
        "arrowright" | "right" => Ok(Code::ArrowRight),
        "f1" => Ok(Code::F1),
        "f2" => Ok(Code::F2),
        "f3" => Ok(Code::F3),
        "f4" => Ok(Code::F4),
        "f5" => Ok(Code::F5),
        "f6" => Ok(Code::F6),
        "f7" => Ok(Code::F7),
        "f8" => Ok(Code::F8),
        "f9" => Ok(Code::F9),
        "f10" => Ok(Code::F10),
        "f11" => Ok(Code::F11),
        "f12" => Ok(Code::F12),
        "minus" | "-" => Ok(Code::Minus),
        "equal" | "=" => Ok(Code::Equal),
        "bracketleft" | "[" => Ok(Code::BracketLeft),
        "bracketright" | "]" => Ok(Code::BracketRight),
        "backslash" | "\\" => Ok(Code::Backslash),
        "semicolon" | ";" => Ok(Code::Semicolon),
        "quote" | "'" => Ok(Code::Quote),
        "backquote" | "`" => Ok(Code::Backquote),
        "comma" | "," => Ok(Code::Comma),
        "period" | "." => Ok(Code::Period),
        "slash" | "/" => Ok(Code::Slash),
        _ => Err(format!("Unknown key: {}", s)),
    }
}

// ---------------------------------------------------------------------------
// HotkeyManager – orchestrates mode switching
// ---------------------------------------------------------------------------

pub struct HotkeyManager {
    shortcut_str: String,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            shortcut_str: DEFAULT_SHORTCUT.to_string(),
        }
    }

    pub fn shortcut_str(&self) -> &str {
        &self.shortcut_str
    }

    /// Load the persisted shortcut from the database (falls back to default).
    pub fn load_saved_shortcut() -> String {
        let saved = Database::new()
            .ok()
            .and_then(|db| db.get_setting(SETTING_KEY).ok().flatten())
            .unwrap_or_default();

        if saved.is_empty() || saved == "double-caps-lock" {
            DEFAULT_SHORTCUT.to_string()
        } else {
            saved
        }
    }

    /// Activate the given shortcut. Deactivates the previous one first.
    pub fn activate(&mut self, app: &tauri::AppHandle, shortcut_str: String) -> Result<(), String> {
        let shortcut = parse_shortcut(&shortcut_str)?;

        // Deactivate current
        self.deactivate(app);

        // Register the new shortcut
        let handle = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                toggle_launcher(&handle);
            })
            .map_err(|e| format!("Failed to register shortcut: {e}"))?;

        self.shortcut_str = shortcut_str;
        Ok(())
    }

    /// Switch to a new shortcut at runtime.
    pub fn switch_shortcut(
        &mut self,
        app: &tauri::AppHandle,
        shortcut_str: &str,
    ) -> Result<(), String> {
        self.activate(app, shortcut_str.to_string())
    }

    /// Deactivate the current shortcut.
    fn deactivate(&self, app: &tauri::AppHandle) {
        if let Ok(shortcut) = parse_shortcut(&self.shortcut_str) {
            let _ = app.global_shortcut().unregister(shortcut);
        }
    }
}
