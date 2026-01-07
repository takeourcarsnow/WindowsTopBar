//! Hotkey system for TopBar
//!
//! Handles global hotkey registration and processing.

#![allow(dead_code)]

use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashMap;
use windows::Win32::Foundation::{HWND, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
};

/// Hotkey action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    ToggleBar,
    OpenMenu,
    QuickSearch,
    ToggleTheme,
    NextModule,
    PreviousModule,
    Refresh,
    Settings,
    Quit,
}

/// Parsed hotkey
#[derive(Debug, Clone)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
    pub action: HotkeyAction,
}

impl Hotkey {
    /// Parse a hotkey string like "Alt+T" or "Ctrl+Shift+S"
    pub fn parse(s: &str, action: HotkeyAction) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = 0u32;
        let mut key = 0u32;

        for (i, part) in parts.iter().enumerate() {
            let part_upper = part.to_uppercase();

            if i == parts.len() - 1 {
                // Last part is the key
                key = Self::parse_key(&part_upper)?;
            } else {
                // Modifier
                match part_upper.as_str() {
                    "ALT" => modifiers |= MOD_ALT.0,
                    "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL.0,
                    "SHIFT" => modifiers |= MOD_SHIFT.0,
                    "WIN" | "WINDOWS" | "SUPER" => modifiers |= MOD_WIN.0,
                    _ => return None,
                }
            }
        }

        Some(Self {
            modifiers,
            key,
            action,
        })
    }

    /// Parse a key name to virtual key code
    fn parse_key(s: &str) -> Option<u32> {
        // Single character keys
        if s.len() == 1 {
            let c = s.chars().next()?;
            if c.is_ascii_alphanumeric() {
                return Some(c.to_ascii_uppercase() as u32);
            }
        }

        // Special keys
        match s {
            "SPACE" => Some(0x20),
            "ENTER" | "RETURN" => Some(0x0D),
            "TAB" => Some(0x09),
            "ESCAPE" | "ESC" => Some(0x1B),
            "BACKSPACE" => Some(0x08),
            "DELETE" | "DEL" => Some(0x2E),
            "INSERT" | "INS" => Some(0x2D),
            "HOME" => Some(0x24),
            "END" => Some(0x23),
            "PAGEUP" | "PGUP" => Some(0x21),
            "PAGEDOWN" | "PGDN" => Some(0x22),
            "UP" => Some(0x26),
            "DOWN" => Some(0x28),
            "LEFT" => Some(0x25),
            "RIGHT" => Some(0x27),
            "F1" => Some(0x70),
            "F2" => Some(0x71),
            "F3" => Some(0x72),
            "F4" => Some(0x73),
            "F5" => Some(0x74),
            "F6" => Some(0x75),
            "F7" => Some(0x76),
            "F8" => Some(0x77),
            "F9" => Some(0x78),
            "F10" => Some(0x79),
            "F11" => Some(0x7A),
            "F12" => Some(0x7B),
            _ => None,
        }
    }
}

/// Hotkey manager
pub struct HotkeyManager {
    hwnd: HWND,
    hotkeys: HashMap<i32, Hotkey>,
    next_id: i32,
}

impl HotkeyManager {
    /// Create a new hotkey manager
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            hotkeys: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a hotkey
    pub fn register(&mut self, hotkey: Hotkey) -> Result<i32> {
        let id = self.next_id;
        self.next_id += 1;

        unsafe {
            let result = RegisterHotKey(
                self.hwnd,
                id,
                HOT_KEY_MODIFIERS(hotkey.modifiers),
                hotkey.key,
            );

            if result.is_ok() {
                info!("Registered hotkey {} for {:?}", id, hotkey.action);
                self.hotkeys.insert(id, hotkey);
                Ok(id)
            } else {
                warn!("Failed to register hotkey for {:?}", hotkey.action);
                Err(anyhow::anyhow!("Failed to register hotkey"))
            }
        }
    }

    /// Register a hotkey from string
    pub fn register_from_string(&mut self, s: &str, action: HotkeyAction) -> Result<i32> {
        let hotkey = Hotkey::parse(s, action)
            .ok_or_else(|| anyhow::anyhow!("Invalid hotkey string: {}", s))?;
        self.register(hotkey)
    }

    /// Unregister a hotkey by ID
    pub fn unregister(&mut self, id: i32) -> Result<()> {
        unsafe {
            let result = UnregisterHotKey(self.hwnd, id);
            if result.is_ok() {
                self.hotkeys.remove(&id);
                debug!("Unregistered hotkey {}", id);
                Ok(())
            } else {
                Err(anyhow::anyhow!("Failed to unregister hotkey"))
            }
        }
    }

    /// Unregister all hotkeys
    pub fn unregister_all(&mut self) {
        let ids: Vec<i32> = self.hotkeys.keys().copied().collect();
        for id in ids {
            let _ = self.unregister(id);
        }
    }

    /// Handle WM_HOTKEY message
    pub fn handle_hotkey(&self, wparam: WPARAM) -> Option<HotkeyAction> {
        let id = wparam.0 as i32;
        self.hotkeys.get(&id).map(|h| h.action)
    }

    /// Get registered hotkeys
    pub fn hotkeys(&self) -> &HashMap<i32, Hotkey> {
        &self.hotkeys
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

// Global hotkey mapping (id -> action) so WM_HOTKEY handler can dispatch
use once_cell::sync::OnceCell;
use parking_lot::Mutex as PLMutex;

static GLOBAL_HOTKEY_MAP: OnceCell<PLMutex<HashMap<i32, HotkeyAction>>> = OnceCell::new();

/// Set the global mapping of hotkey ids to actions (only first set wins)
pub fn set_global_hotkey_map(map: HashMap<i32, HotkeyAction>) {
    let _ = GLOBAL_HOTKEY_MAP.set(PLMutex::new(map));
}

/// Get the global hotkey map (if set)
pub fn global_hotkey_map() -> Option<&'static PLMutex<HashMap<i32, HotkeyAction>>> {
    GLOBAL_HOTKEY_MAP.get()
}

/// Register default hotkeys from config
pub fn register_default_hotkeys(manager: &mut HotkeyManager, config: &crate::config::HotkeyConfig) {
    if let Some(ref key) = config.toggle_bar {
        if let Err(e) = manager.register_from_string(key, HotkeyAction::ToggleBar) {
            warn!("Failed to register toggle_bar hotkey: {}", e);
        }
    }

    if let Some(ref key) = config.open_menu {
        if let Err(e) = manager.register_from_string(key, HotkeyAction::OpenMenu) {
            warn!("Failed to register open_menu hotkey: {}", e);
        }
    }

    if let Some(ref key) = config.quick_search {
        if let Err(e) = manager.register_from_string(key, HotkeyAction::QuickSearch) {
            warn!("Failed to register quick_search hotkey: {}", e);
        }
    }

    if let Some(ref key) = config.toggle_theme {
        if let Err(e) = manager.register_from_string(key, HotkeyAction::ToggleTheme) {
            warn!("Failed to register toggle_theme hotkey: {}", e);
        }
    }
}
