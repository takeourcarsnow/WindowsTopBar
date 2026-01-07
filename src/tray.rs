//! System tray icon support for TopBar

use anyhow::Result;
use log::{debug, info};
use std::sync::Arc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResource, DestroyIcon, LoadImageW, HICON, IMAGE_ICON,
    LR_DEFAULTSIZE, LR_SHARED,
};

use crate::config::Config;
use crate::window::WM_TOPBAR_TRAY;
use crate::utils::to_wide_string;

/// Tray icon identifier
const TRAY_ICON_ID: u32 = 1;

/// System tray manager
pub struct TrayIcon {
    hwnd: HWND,
    icon: HICON,
    is_added: bool,
}

impl TrayIcon {
    /// Create a new tray icon
    pub fn new(hwnd: HWND) -> Result<Self> {
        let icon = Self::load_default_icon()?;
        
        let mut tray = Self {
            hwnd,
            icon,
            is_added: false,
        };
        
        tray.add()?;
        
        Ok(tray)
    }

    /// Load the default icon
    fn load_default_icon() -> Result<HICON> {
        unsafe {
            // Use a system icon as placeholder
            // In a full implementation, you'd embed a custom icon
            let icon = LoadImageW(
                None,
                windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION,
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE | LR_SHARED,
            )?;
            
            Ok(HICON(icon.0))
        }
    }

    /// Add the tray icon
    fn add(&mut self) -> Result<()> {
        let tooltip = to_wide_string("TopBar - Click to toggle");
        
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TOPBAR_TRAY,
            hIcon: self.icon,
            ..Default::default()
        };

        // Copy tooltip
        let tooltip_len = tooltip.len().min(128);
        nid.szTip[..tooltip_len].copy_from_slice(&tooltip[..tooltip_len]);

        unsafe {
            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                return Err(anyhow::anyhow!("Failed to add tray icon"));
            }
        }
        
        self.is_added = true;
        info!("Tray icon added");
        
        Ok(())
    }

    /// Remove the tray icon
    fn remove(&mut self) -> Result<()> {
        if !self.is_added {
            return Ok(());
        }

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };

        unsafe {
            if !Shell_NotifyIconW(NIM_DELETE, &nid).as_bool() {
                return Err(anyhow::anyhow!("Failed to remove tray icon"));
            }
        }
        
        self.is_added = false;
        info!("Tray icon removed");
        
        Ok(())
    }

    /// Update the tooltip
    pub fn set_tooltip(&mut self, text: &str) -> Result<()> {
        if !self.is_added {
            return Ok(());
        }

        let tooltip = to_wide_string(text);
        
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_TIP,
            ..Default::default()
        };

        let tooltip_len = tooltip.len().min(128);
        nid.szTip[..tooltip_len].copy_from_slice(&tooltip[..tooltip_len]);

        unsafe {
            if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                return Err(anyhow::anyhow!("Failed to update tray tooltip"));
            }
        }
        
        Ok(())
    }

    /// Update the icon
    pub fn set_icon(&mut self, icon: HICON) -> Result<()> {
        if !self.is_added {
            return Ok(());
        }

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON,
            hIcon: icon,
            ..Default::default()
        };

        unsafe {
            if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                return Err(anyhow::anyhow!("Failed to update tray icon"));
            }
        }
        
        self.icon = icon;
        
        Ok(())
    }

    /// Handle tray icon click
    pub fn handle_click(&self, lparam: LPARAM) {
        use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONUP, WM_RBUTTONUP, WM_LBUTTONDBLCLK};
        
        let message = (lparam.0 & 0xFFFF) as u32;
        
        match message {
            WM_LBUTTONUP => {
                debug!("Tray icon left clicked");
                // Toggle visibility
            }
            WM_RBUTTONUP => {
                debug!("Tray icon right clicked");
                // Show context menu
            }
            WM_LBUTTONDBLCLK => {
                debug!("Tray icon double clicked");
                // Open settings
            }
            _ => {}
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let _ = self.remove();
        unsafe {
            if !self.icon.is_invalid() {
                let _ = DestroyIcon(self.icon);
            }
        }
    }
}

/// Tray context menu
pub struct TrayMenu {
    items: Vec<TrayMenuItem>,
}

/// Tray menu item
pub struct TrayMenuItem {
    pub id: u32,
    pub label: String,
    pub is_separator: bool,
    pub is_checked: bool,
    pub is_disabled: bool,
}

impl TrayMenu {
    /// Create a new tray menu
    pub fn new() -> Self {
        Self {
            items: vec![
                TrayMenuItem {
                    id: 1,
                    label: "Show TopBar".to_string(),
                    is_separator: false,
                    is_checked: false,
                    is_disabled: false,
                },
                TrayMenuItem {
                    id: 0,
                    label: String::new(),
                    is_separator: true,
                    is_checked: false,
                    is_disabled: false,
                },
                TrayMenuItem {
                    id: 2,
                    label: "Settings...".to_string(),
                    is_separator: false,
                    is_checked: false,
                    is_disabled: false,
                },
                TrayMenuItem {
                    id: 3,
                    label: "About TopBar".to_string(),
                    is_separator: false,
                    is_checked: false,
                    is_disabled: false,
                },
                TrayMenuItem {
                    id: 0,
                    label: String::new(),
                    is_separator: true,
                    is_checked: false,
                    is_disabled: false,
                },
                TrayMenuItem {
                    id: 100,
                    label: "Exit".to_string(),
                    is_separator: false,
                    is_checked: false,
                    is_disabled: false,
                },
            ],
        }
    }

    /// Show the context menu at cursor position
    pub fn show(&self, hwnd: HWND) -> Option<u32> {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreatePopupMenu, DestroyMenu, InsertMenuW, SetForegroundWindow,
            TrackPopupMenu, GetCursorPos, MF_STRING, MF_SEPARATOR, MF_CHECKED,
            MF_GRAYED, TPM_RIGHTBUTTON, TPM_RETURNCMD,
        };
        use windows::Win32::Foundation::POINT;

        unsafe {
            let menu = CreatePopupMenu().ok()?;

            for item in &self.items {
                let mut flags = if item.is_separator {
                    MF_SEPARATOR
                } else {
                    MF_STRING
                };

                if item.is_checked {
                    flags |= MF_CHECKED;
                }
                if item.is_disabled {
                    flags |= MF_GRAYED;
                }

                if item.is_separator {
                    InsertMenuW(menu, u32::MAX, flags, 0, PCWSTR::null()).ok()?;
                } else {
                    let label = to_wide_string(&item.label);
                    InsertMenuW(
                        menu,
                        u32::MAX,
                        flags,
                        item.id as usize,
                        PCWSTR::from_raw(label.as_ptr()),
                    ).ok()?;
                }
            }

            let mut pt = POINT::default();
            GetCursorPos(&mut pt).ok()?;

            let _ = SetForegroundWindow(hwnd);

            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                pt.x,
                pt.y,
                0,
                hwnd,
                None,
            );

            DestroyMenu(menu).ok()?;

            if cmd.as_bool() {
                Some(cmd.0 as u32)
            } else {
                None
            }
        }
    }

    /// Handle menu command
    pub fn handle_command(&self, id: u32) {
        match id {
            1 => {
                // Toggle show
                debug!("Show TopBar clicked");
            }
            2 => {
                // Settings
                debug!("Settings clicked");
            }
            3 => {
                // About
                debug!("About clicked");
            }
            100 => {
                // Exit
                debug!("Exit clicked");
                std::process::exit(0);
            }
            _ => {}
        }
    }
}

impl Default for TrayMenu {
    fn default() -> Self {
        Self::new()
    }
}
