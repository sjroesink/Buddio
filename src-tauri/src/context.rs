use serde::{Deserialize, Serialize};

/// Context captured at the moment the launcher is invoked.
/// Includes clipboard text, selected text, and info about the previously focused window.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaunchContext {
    /// Text currently on the clipboard (if any).
    pub clipboard_text: Option<String>,
    /// Text that was selected in the source application (if any).
    pub selected_text: Option<String>,
    /// Title of the window that had focus before the launcher appeared.
    pub source_window_title: Option<String>,
    /// Process name of the source application (e.g. "Code.exe", "chrome.exe").
    pub source_process_name: Option<String>,
    /// Full executable path of the source process (e.g. "C:\\Program Files\\...\\Code.exe").
    pub source_process_path: Option<String>,
}

/// Capture the full launch context from the OS.
/// Must be called *before* the launcher window takes focus.
pub fn capture_launch_context() -> LaunchContext {
    let source_window_title = get_foreground_window_title();
    let (source_process_name, source_process_path) = get_foreground_process_info();
    let clipboard_text = get_clipboard_text();
    let selected_text = capture_selected_text(&clipboard_text);

    LaunchContext {
        clipboard_text,
        selected_text,
        source_window_title,
        source_process_name,
        source_process_path,
    }
}

// ── Cross-platform clipboard via arboard ──

/// Read text from the system clipboard.
fn get_clipboard_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok().filter(|s| !s.is_empty())
}

/// Write text to the system clipboard.
fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to init clipboard: {e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("Failed to set clipboard: {e}"))
}

// ── Cross-platform selected text capture ──

/// Capture the currently selected text by simulating Ctrl+C (or Cmd+C on macOS)
/// and reading clipboard. Compares against the previous clipboard contents to
/// detect whether something was copied.
fn capture_selected_text(previous_clipboard: &Option<String>) -> Option<String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default()).ok()?;

    // Use Cmd on macOS, Ctrl on Windows/Linux
    let modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };

    enigo.key(modifier, Direction::Press).ok()?;
    enigo.key(Key::Unicode('c'), Direction::Click).ok()?;
    enigo.key(modifier, Direction::Release).ok()?;

    // Give the target app time to update the clipboard
    std::thread::sleep(std::time::Duration::from_millis(80));

    let new_clipboard = get_clipboard_text();

    match (&new_clipboard, previous_clipboard) {
        (Some(new), Some(old)) if new != old => Some(new.clone()),
        (Some(new), None) => Some(new.clone()),
        _ => None,
    }
}

// ── Foreground window info (platform-specific) ──

/// Get the title of the foreground window.
fn get_foreground_window_title() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        win32::get_foreground_window_title()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_foreground_window_title()
    }
    #[cfg(target_os = "linux")]
    {
        None
    }
}

/// Get process metadata (name + full executable path) for the foreground window.
fn get_foreground_process_info() -> (Option<String>, Option<String>) {
    #[cfg(target_os = "windows")]
    {
        win32::get_foreground_process_info()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_foreground_process_info()
    }
    #[cfg(target_os = "linux")]
    {
        (None, None)
    }
}

// ── Cross-platform text input ──

/// Simulate typing text into the currently focused application.
pub fn type_text(text: &str) -> Result<(), String> {
    use enigo::{Enigo, Keyboard, Settings};

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to init enigo: {e}"))?;

    enigo
        .text(text)
        .map_err(|e| format!("Failed to type text: {e}"))?;

    Ok(())
}

/// Simulate replacing the current selection: put text on clipboard, then paste.
/// Uses Cmd+V on macOS, Ctrl+V on Windows/Linux.
pub fn replace_selection(text: &str) -> Result<(), String> {
    set_clipboard_text(text)?;

    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to init enigo: {e}"))?;

    let modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| format!("Failed to press modifier: {e}"))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Failed to press V: {e}"))?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| format!("Failed to release modifier: {e}"))?;

    Ok(())
}

// ── Windows-specific implementations ──

#[cfg(target_os = "windows")]
mod win32 {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    pub fn get_foreground_window_title() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == HWND::default() {
                return None;
            }

            let mut title = vec![0u16; 512];
            let len = GetWindowTextW(hwnd, &mut title);

            if len > 0 {
                let text = String::from_utf16_lossy(&title[..len as usize]);
                Some(text)
            } else {
                None
            }
        }
    }

    pub fn get_foreground_process_info() -> (Option<String>, Option<String>) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == HWND::default() {
                return (None, None);
            }

            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            if process_id == 0 {
                return (None, None);
            }

            use windows::Win32::System::Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
                PROCESS_QUERY_LIMITED_INFORMATION,
            };

            let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
                Ok(p) => p,
                Err(_) => return (None, None),
            };

            let mut name_buf = vec![0u16; 1024];
            let mut size = name_buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut size,
            );

            let _ = windows::Win32::Foundation::CloseHandle(process);

            if ok.is_ok() && size > 0 {
                let full_path = String::from_utf16_lossy(&name_buf[..size as usize]);
                let name = std::path::Path::new(&full_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| full_path.clone());
                (Some(name), Some(full_path))
            } else {
                (None, None)
            }
        }
    }
}

// ── macOS-specific implementations ──

#[cfg(target_os = "macos")]
mod macos {
    use std::process::Command;

    pub fn get_foreground_window_title() -> Option<String> {
        let output = Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events"
                    set frontApp to first application process whose frontmost is true
                    try
                        set winTitle to name of front window of frontApp
                        return winTitle
                    on error
                        return name of frontApp
                    end try
                end tell"#,
            ])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn get_foreground_process_name() -> Option<String> {
        let output = Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events"
                    set frontApp to first application process whose frontmost is true
                    return name of frontApp
                end tell"#,
            ])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn get_foreground_process_path() -> Option<String> {
        let output = Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events"
                    set frontApp to first application process whose frontmost is true
                    try
                        return POSIX path of (file of frontApp as alias)
                    on error
                        return ""
                    end try
                end tell"#,
            ])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn get_foreground_process_info() -> (Option<String>, Option<String>) {
        (get_foreground_process_name(), get_foreground_process_path())
    }
}
