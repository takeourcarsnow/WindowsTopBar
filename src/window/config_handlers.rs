//! Configuration management handlers for the TopBar application
//!
//! Contains functions for reloading, resetting, and toggling configuration options.

use log::{info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::Graphics::Gdi::InvalidateRect;

use crate::config::Config;

use super::state::get_window_state;

/// Toggle a boolean config value
pub fn toggle_config_bool<F>(hwnd: HWND, getter: F)
where
    F: FnOnce(&mut crate::config::Config) -> &mut bool,
{
    if let Some(state) = get_window_state() {
        let config = state.read().config.clone();
        let mut new_config = (*config).clone();

        let value = getter(&mut new_config);
        *value = !*value;

        if let Err(e) = new_config.save() {
            warn!("Failed to save config: {}", e);
        }

        state.write().config = std::sync::Arc::new(new_config);
        unsafe {
            let _ = InvalidateRect(hwnd, None, true);
        }
    }
}

/// Toggle a module on/off
pub fn toggle_module(hwnd: HWND, module_id: &str) {
    if let Some(state) = get_window_state() {
        let config = state.read().config.clone();
        let mut new_config = (*config).clone();

        // Special handling for clock when it's centered
        if module_id == "clock"
            && new_config
                .modules
                .center_modules
                .iter()
                .any(|m| m == "clock")
        {
            // Clock is centered, remove it from center and disable
            new_config.modules.center_modules.retain(|m| m != "clock");
            new_config.modules.clock.center = false;
            info!("Disabled centered clock: {}", module_id);
        }
        // Check if module exists in right_modules
        else if let Some(pos) = new_config
            .modules
            .right_modules
            .iter()
            .position(|m| m == module_id)
        {
            // Remove it
            new_config.modules.right_modules.remove(pos);
            info!("Disabled module: {}", module_id);
        } else {
            // Add it back at the appropriate position
            let insert_pos = find_module_insert_position(&new_config.modules.right_modules, module_id);
            new_config.modules.right_modules.insert(insert_pos, module_id.to_string());
            info!("Enabled module: {}", module_id);
        }

        // Save config
        if let Err(e) = new_config.save() {
            warn!("Failed to save config: {}", e);
        }

        // Update the state with new config
        state.write().config = std::sync::Arc::new(new_config);

        // Force a redraw so changes take effect immediately
        unsafe {
            let _ = InvalidateRect(hwnd, None, true);
        }
    }
}

/// Open config file in default editor
pub fn open_config_file() {
    use crate::config::Config;
    let path = Config::config_path();

    // Create config if it doesn't exist
    if !path.exists() {
        if let Ok(config) = Config::load_or_default() {
            let _ = config.save();
        }
    }

    unsafe {
        let path_wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path_wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
    info!("Opening config file: {:?}", path);
}

/// Reload configuration
pub fn reload_config(hwnd: HWND) {
    use crate::config::Config;

    match Config::load_or_default() {
        Ok(config) => {
            if let Some(state) = get_window_state() {
                state.write().config = std::sync::Arc::new(config);
                info!("Configuration reloaded");
                unsafe {
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
        }
        Err(e) => {
            warn!("Failed to reload config: {}", e);
        }
    }
}

/// Reset configuration to defaults (with confirmation)
pub fn reset_config(hwnd: HWND) {
    use crate::config::Config;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_YESNO, IDYES};

    unsafe {
        let title: Vec<u16> = "Reset Settings".encode_utf16().chain(std::iter::once(0)).collect();
        let msg: Vec<u16> = "Reset all settings to defaults? This will overwrite your config file.".encode_utf16().chain(std::iter::once(0)).collect();

        let resp = MessageBoxW(None, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_YESNO | MB_ICONWARNING);
        if resp.0 == IDYES.0 {
            let cfg = Config::default();
            match cfg.save() {
                Ok(_) => {
                    if let Some(state) = get_window_state() {
                        state.write().config = std::sync::Arc::new(cfg);
                        info!("Configuration reset to defaults");
                        let _ = InvalidateRect(hwnd, None, true);
                    }
                }
                Err(e) => {
                    warn!("Failed to save default config: {}", e);
                }
            }
        } else {
            info!("Reset to defaults cancelled by user");
        }
    }
}

/// Install bundled macOS-style cursors by running the INF 'Install.inf' in the resources folder.
/// This will invoke the system installer (may prompt for UAC) and display a confirmation on error/success.
pub fn install_mac_cursors(hwnd: HWND) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK, MB_ICONERROR, MB_OKCANCEL, IDOK};

    // Try to locate the INF file in a few candidate locations relative to the executable and current directory.
    let mut candidates = Vec::new();
    if let Ok(mut exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            candidates.push(dir.join("resources").join("macoscursors").join("Install.inf"));
            candidates.push(dir.join("..").join("resources").join("macoscursors").join("Install.inf"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("resources").join("macoscursors").join("Install.inf"));
    }

    let inf = candidates.into_iter().find(|p| p.exists());
    if inf.is_none() {
        unsafe {
            let msg = "Could not find 'Install.inf' in the expected resources path.";
            let title = "Install Cursors";
            let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(None, PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()), MB_OK | MB_ICONERROR);
        }
        return;
    }
    let inf = inf.unwrap();

    // Ask the user to continue
    unsafe {
        let title = "Install macOS Cursors";
        let msg = format!("Installer INF found at:\n{}\n\nClick OK to run the installer (you may be prompted for administrator permission).", inf.display());
        let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let resp = MessageBoxW(None, PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()), MB_OKCANCEL | MB_ICONWARNING);
        if resp.0 != IDOK.0 {
            return;
        }
    }

    // Launch installer via ShellExecute 'install' verb
    let path_wide: Vec<u16> = inf.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let h = ShellExecuteW(None, w!("install"), PCWSTR(path_wide.as_ptr()), None, None, SW_SHOWNORMAL);
        // According to ShellExecute docs, return value > 32 indicates success
        if (h.0 as isize) <= 32 {
            let title = "Install Cursors";
            let msg = format!("Failed to launch installer (error code {}).", h.0 as isize);
            let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(None, PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()), MB_OK | MB_ICONERROR);
        } else {
            let title = "Install Cursors";
            let msg = "Installer launched. Follow the system dialogs to install the cursors.";
            let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(None, PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()), MB_OK | MB_ICONWARNING);
        }
    }

}

/// Default order of right-side modules for insertion position calculation
const DEFAULT_RIGHT_MODULE_ORDER: &[&str] = &[ 
    "weather",
    "media",
    "clipboard",
    "keyboard_layout",
    "gpu",
    "system_info",
    "disk",
    "network",
    "bluetooth",
    "night_light",
    "volume",
    "battery",
    "uptime",
    "clock",
];

/// Find the appropriate insert position for a module based on default order
fn find_module_insert_position(existing_modules: &[String], module_id: &str) -> usize {
    DEFAULT_RIGHT_MODULE_ORDER
        .iter()
        .position(|&m| m == module_id)
        .map(|target_idx| {
            existing_modules
                .iter()
                .position(|m| {
                    DEFAULT_RIGHT_MODULE_ORDER
                        .iter()
                        .position(|&dm| dm == m.as_str())
                        .map(|existing_idx| existing_idx > target_idx)
                        .unwrap_or(false)
                })
                .unwrap_or(existing_modules.len())
        })
        .unwrap_or(existing_modules.len())
}