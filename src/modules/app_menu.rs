//! App menu module - macOS-style application menu

#![allow(dead_code)]

use super::Module;

/// Menu item structure
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    pub action: MenuAction,
    pub submenu: Vec<MenuItem>,
    pub is_separator: bool,
    pub is_disabled: bool,
}

impl MenuItem {
    pub fn new(label: &str, action: MenuAction) -> Self {
        Self {
            label: label.to_string(),
            icon: None,
            shortcut: None,
            action,
            submenu: Vec::new(),
            is_separator: false,
            is_disabled: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            label: String::new(),
            icon: None,
            shortcut: None,
            action: MenuAction::None,
            submenu: Vec::new(),
            is_separator: true,
            is_disabled: false,
        }
    }

    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    pub fn with_submenu(mut self, submenu: Vec<MenuItem>) -> Self {
        self.submenu = submenu;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.is_disabled = true;
        self
    }
}

/// Menu action types
#[derive(Debug, Clone)]
pub enum MenuAction {
    None,
    SystemInfo,
    OpenSettings,
    Sleep,
    Restart,
    Shutdown,
    Lock,
    SignOut,
    OpenUrl(String),
    RunCommand(String),
    OpenFile(String),
    Custom(String),
}

/// App menu module
pub struct AppMenuModule {
    cached_text: String,
    menu_items: Vec<MenuItem>,
    is_open: bool,
}

impl AppMenuModule {
    pub fn new() -> Self {
        let menu_items = Self::default_menu_items();

        Self {
            cached_text: "☰".to_string(), // Hamburger menu icon
            menu_items,
            is_open: false,
        }
    }

    /// Create default menu items
    fn default_menu_items() -> Vec<MenuItem> {
        vec![
            MenuItem::new("About This PC", MenuAction::SystemInfo).with_icon("ℹ️"),
            MenuItem::separator(),
            MenuItem::new("System Preferences...", MenuAction::OpenSettings).with_icon("⚙️"),
            MenuItem::new(
                "App Store...",
                MenuAction::OpenUrl("ms-windows-store:".to_string()),
            )
            .with_icon("🏪"),
            MenuItem::separator(),
            MenuItem::new("Recent Items", MenuAction::None).with_submenu(vec![MenuItem::new(
                "No recent items",
                MenuAction::None,
            )
            .disabled()]),
            MenuItem::separator(),
            MenuItem::new(
                "Force Quit...",
                MenuAction::RunCommand("taskmgr".to_string()),
            )
            .with_icon("⚠️")
            .with_shortcut("Ctrl+Alt+Del"),
            MenuItem::separator(),
            MenuItem::new("Sleep", MenuAction::Sleep).with_icon("😴"),
            MenuItem::new("Restart...", MenuAction::Restart).with_icon("🔄"),
            MenuItem::new("Shut Down...", MenuAction::Shutdown).with_icon("⏻"),
            MenuItem::separator(),
            MenuItem::new("Lock Screen", MenuAction::Lock)
                .with_icon("🔒")
                .with_shortcut("Win+L"),
            MenuItem::new("Sign Out...", MenuAction::SignOut).with_icon("🚪"),
        ]
    }

    /// Execute a menu action
    pub fn execute_action(&self, action: &MenuAction) {
        match action {
            MenuAction::None => {}
            MenuAction::SystemInfo => {
                crate::utils::open_url("ms-settings:about");
            }
            MenuAction::OpenSettings => {
                crate::utils::open_url("ms-settings:");
            }
            MenuAction::Sleep => {
                let _ = std::process::Command::new("rundll32.exe")
                    .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                    .spawn();
            }
            MenuAction::Restart => {
                let _ = std::process::Command::new("shutdown")
                    .args(["/r", "/t", "0"])
                    .spawn();
            }
            MenuAction::Shutdown => {
                let _ = std::process::Command::new("shutdown")
                    .args(["/s", "/t", "0"])
                    .spawn();
            }
            MenuAction::Lock => {
                let _ = std::process::Command::new("rundll32.exe")
                    .args(["user32.dll,LockWorkStation"])
                    .spawn();
            }
            MenuAction::SignOut => {
                let _ = std::process::Command::new("shutdown").args(["/l"]).spawn();
            }
            MenuAction::OpenUrl(url) => {
                crate::utils::open_url(url);
            }
            MenuAction::RunCommand(cmd) => {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("cmd")
                    .args(["/c", cmd])
                    .creation_flags(0x08000000)
                    .spawn();
            }
            MenuAction::OpenFile(path) => {
                crate::utils::open_url(path);
            }
            MenuAction::Custom(_id) => {
                // Custom action handling would go here
            }
        }
    }

    /// Get menu items
    pub fn menu_items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Set menu items
    pub fn set_menu_items(&mut self, items: Vec<MenuItem>) {
        self.menu_items = items;
    }

    /// Toggle menu open state
    pub fn toggle_menu(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Check if menu is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }
}

impl Default for AppMenuModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for AppMenuModule {
    fn id(&self) -> &str {
        "app_menu"
    }

    fn name(&self) -> &str {
        "App Menu"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        "☰".to_string()
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Menu doesn't need periodic updates
    }

    fn on_click(&mut self) {
        self.toggle_menu();
        // In a full implementation, this would show the dropdown menu
    }

    fn tooltip(&self) -> Option<String> {
        Some("Click for menu".to_string())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
