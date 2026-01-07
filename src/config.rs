//! Configuration management for TopBar
//!
//! Handles loading, saving, and managing user preferences and settings.

use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::theme::ThemeMode;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// General application settings
    pub general: GeneralConfig,
    /// Appearance settings
    pub appearance: AppearanceConfig,
    /// Module configurations
    pub modules: ModulesConfig,
    /// Behavior settings
    pub behavior: BehaviorConfig,
    /// Hotkey configurations
    pub hotkeys: HotkeyConfig,
}

impl Config {
    /// Get the configuration file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("topbar")
            .join("config.toml")
    }

    /// Load configuration from file or create default
    pub fn load_or_default() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            info!("Loading configuration from: {:?}", config_path);
            let content = std::fs::read_to_string(&config_path)?;
            match toml::from_str(&content) {
                Ok(config) => return Ok(config),
                Err(e) => {
                    warn!("Failed to parse config, using defaults: {}", e);
                }
            }
        }

        let config = Self::default();
        config.save()?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        info!("Configuration saved to: {:?}", config_path);
        Ok(())
    }
}

/// General application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Start with Windows
    pub start_with_windows: bool,
    /// Show in taskbar
    pub show_in_taskbar: bool,
    /// Language (ISO 639-1 code)
    pub language: String,
    /// Check for updates automatically
    pub auto_update_check: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            show_in_taskbar: false,
            language: "en".to_string(),
            auto_update_check: true,
        }
    }
}

/// Appearance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Theme mode (light, dark, auto)
    pub theme_mode: ThemeMode,
    /// Custom accent color (hex)
    pub accent_color: Option<String>,
    /// Bar height in pixels
    pub bar_height: u32,
    /// Bar opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Enable blur effect
    pub blur_enabled: bool,
    /// Blur intensity (0-100)
    pub blur_intensity: u32,
    /// Corner radius for menus
    pub corner_radius: u32,
    /// Font family
    pub font_family: String,
    /// Font size
    pub font_size: u32,
    /// Enable animations
    pub animations_enabled: bool,
    /// Animation speed (ms)
    pub animation_speed: u32,
    /// Shadow enabled
    pub shadow_enabled: bool,
    /// Bar position (top or bottom)
    pub position: BarPosition,
    /// Monitor index (0 = primary, -1 = all)
    pub monitor: i32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Auto,
            accent_color: None,
            bar_height: 34, // macOS-inspired height for better proportions
            opacity: 0.90,  // Balanced opacity for modern glass aesthetic
            blur_enabled: true,
            blur_intensity: 50, // Enhanced blur for premium glass effect
            corner_radius: 12,  // macOS-style rounded corners
            font_family: "Segoe UI Variable Text".to_string(), // SF Pro-inspired modern font
            font_size: 13,
            animations_enabled: true,
            animation_speed: 100, // macOS-style snappy animations (100ms)
            shadow_enabled: true,
            position: BarPosition::Top,
            monitor: 0,
        }
    }
}

/// Bar position enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum BarPosition {
    Top,
    Bottom,
}

/// Module configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesConfig {
    /// Clock module settings
    pub clock: ClockConfig,
    /// System info module settings
    pub system_info: SystemInfoConfig,
    /// Weather module settings
    pub weather: WeatherConfig,
    /// App menu settings
    pub app_menu: AppMenuConfig,
    /// Media controls settings
    pub media: MediaConfig,
    /// Network module settings
    pub network: NetworkConfig,
    /// Battery module settings
    pub battery: BatteryConfig,
    /// Volume module settings
    pub volume: VolumeConfig,
    /// GPU module settings
    pub gpu: GpuConfig,
    /// Keyboard layout module settings
    pub keyboard_layout: KeyboardLayoutConfig,
    /// Uptime module settings
    pub uptime: UptimeConfig,
    /// Bluetooth module settings
    pub bluetooth: BluetoothConfig,
    /// Disk module settings
    pub disk: DiskConfig,
    /// Enabled modules in order (left side)
    pub left_modules: Vec<String>,
    /// Enabled modules in order (center)
    pub center_modules: Vec<String>,
    /// Enabled modules in order (right side)
    pub right_modules: Vec<String>,
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            clock: ClockConfig::default(),
            system_info: SystemInfoConfig::default(),
            weather: WeatherConfig::default(),
            app_menu: AppMenuConfig::default(),
            media: MediaConfig::default(),
            network: NetworkConfig::default(),
            battery: BatteryConfig::default(),
            volume: VolumeConfig::default(),
            gpu: GpuConfig::default(),
            keyboard_layout: KeyboardLayoutConfig::default(),
            uptime: UptimeConfig::default(),
            bluetooth: BluetoothConfig::default(),
            disk: DiskConfig::default(),
            left_modules: vec!["app_menu".to_string(), "active_app".to_string()],
            center_modules: vec![],
            right_modules: vec![
                "weather".to_string(),
                "media".to_string(),
                "keyboard_layout".to_string(),
                "gpu".to_string(),
                "system_info".to_string(),
                "disk".to_string(),
                "network".to_string(),
                "bluetooth".to_string(),
                "volume".to_string(),
                "battery".to_string(),
                "uptime".to_string(),
                "clock".to_string(),
            ],
        }
    }
}

/// Clock module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockConfig {
    /// Time format (12h or 24h)
    pub format_24h: bool,
    /// Show seconds
    pub show_seconds: bool,
    /// Show date
    pub show_date: bool,
    /// Show day of week
    pub show_day: bool,
    /// Center the clock in the bar
    pub center: bool,
    /// Date format
    pub date_format: String,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            format_24h: false,
            show_seconds: false,
            show_date: true,
            show_day: true,
            center: false,
            date_format: "%a, %b %d".to_string(), // Include day name: "Tue, Jan 7"
        }
    }
}

/// System info module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoConfig {
    /// Show CPU usage
    pub show_cpu: bool,
    /// Show memory usage
    pub show_memory: bool,
    /// Show disk usage
    pub show_disk: bool,
    /// Show GPU usage
    pub show_gpu: bool,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
    /// Show as graph
    pub show_graph: bool,
}

impl Default for SystemInfoConfig {
    fn default() -> Self {
        Self {
            show_cpu: true,
            show_memory: true,
            show_disk: false,
            show_gpu: false,
            update_interval_ms: 1500, // Slightly faster updates for responsiveness
            show_graph: false,
        }
    }
}

/// Weather module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConfig {
    /// Enable weather module
    pub enabled: bool,
    /// Location (city name like "London", "New York", "Tokyo" or "auto" for automatic detection)
    pub location: String,
    /// Temperature unit (celsius or fahrenheit)
    pub unit: TemperatureUnit,
    /// Show condition icon
    pub show_icon: bool,
    /// Update interval in minutes
    pub update_interval_min: u32,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: true,                // Enabled by default - no API key needed!
            location: "auto".to_string(), // Auto-detect based on IP
            unit: TemperatureUnit::Celsius,
            show_icon: true,
            update_interval_min: 30,
        }
    }
}

/// Temperature unit enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

/// App menu configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMenuConfig {
    /// Show app icon
    pub show_icon: bool,
    /// Menu items
    pub items: Vec<MenuItemConfig>,
    /// Show search
    pub show_search: bool,
    /// Show recent apps
    pub show_recent: bool,
    /// Max recent apps count
    pub recent_count: usize,
}

impl Default for AppMenuConfig {
    fn default() -> Self {
        Self {
            show_icon: true,
            items: vec![
                MenuItemConfig {
                    label: "About This PC".to_string(),
                    action: MenuAction::SystemInfo,
                    icon: Some("info".to_string()),
                    submenu: vec![],
                },
                MenuItemConfig {
                    label: "System Preferences".to_string(),
                    action: MenuAction::OpenSettings,
                    icon: Some("settings".to_string()),
                    submenu: vec![],
                },
                MenuItemConfig {
                    label: "-".to_string(),
                    action: MenuAction::Separator,
                    icon: None,
                    submenu: vec![],
                },
                MenuItemConfig {
                    label: "Sleep".to_string(),
                    action: MenuAction::Sleep,
                    icon: Some("sleep".to_string()),
                    submenu: vec![],
                },
                MenuItemConfig {
                    label: "Restart".to_string(),
                    action: MenuAction::Restart,
                    icon: Some("restart".to_string()),
                    submenu: vec![],
                },
                MenuItemConfig {
                    label: "Shut Down".to_string(),
                    action: MenuAction::Shutdown,
                    icon: Some("power".to_string()),
                    submenu: vec![],
                },
            ],
            show_search: true,
            show_recent: true,
            recent_count: 5,
        }
    }
}

/// Menu item configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItemConfig {
    /// Display label
    pub label: String,
    /// Action to perform
    pub action: MenuAction,
    /// Icon name
    pub icon: Option<String>,
    /// Submenu items
    pub submenu: Vec<MenuItemConfig>,
}

/// Menu action enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuAction {
    /// Open system info
    SystemInfo,
    /// Open Windows settings
    OpenSettings,
    /// Visual separator
    Separator,
    /// Sleep the computer
    Sleep,
    /// Restart the computer
    Restart,
    /// Shut down the computer
    Shutdown,
    /// Lock the computer
    Lock,
    /// Sign out
    SignOut,
    /// Open a URL
    OpenUrl(String),
    /// Run a command
    RunCommand(String),
    /// Open a file
    OpenFile(String),
    /// Custom action
    Custom(String),
    /// No action (for submenu parents)
    None,
}

/// Media controls configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    /// Show now playing
    pub show_now_playing: bool,
    /// Show album art
    pub show_album_art: bool,
    /// Show playback controls
    pub show_controls: bool,
    /// Scroll title if too long
    pub scroll_title: bool,
    /// Max title length before scrolling
    pub max_title_length: usize,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            show_now_playing: true,
            show_album_art: true,
            show_controls: true,
            scroll_title: true,
            max_title_length: 35, // Slightly longer for better context
        }
    }
}

/// Network module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Show network type icon
    pub show_icon: bool,
    /// Show network name
    pub show_name: bool,
    /// Show speed
    pub show_speed: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            show_icon: true,
            show_name: false,
            show_speed: false,
        }
    }
}

/// Battery module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryConfig {
    /// Show percentage
    pub show_percentage: bool,
    /// Show time remaining
    pub show_time_remaining: bool,
    /// Low battery threshold
    pub low_threshold: u32,
    /// Critical battery threshold
    pub critical_threshold: u32,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        Self {
            show_percentage: true,
            show_time_remaining: false,
            low_threshold: 20,
            critical_threshold: 10,
        }
    }
}

/// Volume module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    /// Show percentage
    pub show_percentage: bool,
    /// Show on scroll change
    pub scroll_to_change: bool,
    /// Volume step for scroll
    pub scroll_step: u32,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        Self {
            show_percentage: false,
            scroll_to_change: true,
            scroll_step: 5,
        }
    }
}

/// GPU module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// Enable GPU module
    pub enabled: bool,
    /// Show GPU usage percentage
    pub show_usage: bool,
    /// Show as a moving graph instead of percentage
    pub show_graph: bool,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_usage: true,
            show_graph: false,
            update_interval_ms: 1500, // More responsive updates
        }
    }
}

/// Keyboard layout module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardLayoutConfig {
    /// Enable keyboard layout module
    pub enabled: bool,
    /// Show full language name
    pub show_full_name: bool,
    /// Show flag emoji
    pub show_flag: bool,
}

impl Default for KeyboardLayoutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_full_name: false,
            show_flag: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    /// Auto-hide the bar
    pub auto_hide: bool,
    /// Auto-hide delay in milliseconds
    pub auto_hide_delay_ms: u32,
    /// Reserve screen space (push windows down)
    pub reserve_space: bool,
    /// Show on all virtual desktops
    pub all_desktops: bool,
    /// Allow dragging windows by bar
    pub drag_to_move: bool,
    /// Double click action
    pub double_click_action: DoubleClickAction,
    /// Focus follows mouse for menus
    pub focus_follows_mouse: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_hide: false,
            auto_hide_delay_ms: 800, // Faster response for better UX
            reserve_space: true,
            all_desktops: true,
            drag_to_move: false,
            double_click_action: DoubleClickAction::None,
            focus_follows_mouse: true,
        }
    }
}

/// Double click action enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DoubleClickAction {
    None,
    MaximizeWindow,
    MinimizeWindow,
    OpenSettings,
}

/// Hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Toggle bar visibility
    pub toggle_bar: Option<String>,
    /// Open app menu
    pub open_menu: Option<String>,
    /// Quick search
    pub quick_search: Option<String>,
    /// Toggle theme
    pub toggle_theme: Option<String>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            toggle_bar: Some("Alt+T".to_string()),
            open_menu: Some("Alt+Space".to_string()),
            quick_search: Some("Alt+S".to_string()),
            toggle_theme: Some("Alt+D".to_string()),
        }
    }
}

/// Uptime module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeConfig {
    /// Enable uptime module
    pub enabled: bool,
    /// Show icon
    pub show_icon: bool,
}

impl Default for UptimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_icon: true,
        }
    }
}

/// Bluetooth module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothConfig {
    /// Enable Bluetooth module
    pub enabled: bool,
    /// Show connected device count
    pub show_device_count: bool,
    /// Show connected device names
    pub show_device_names: bool,
}

impl Default for BluetoothConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_device_count: true,
            show_device_names: false,
        }
    }
}

/// Disk module configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConfig {
    /// Enable disk module
    pub enabled: bool,
    /// Primary disk to monitor (e.g., "C:")
    pub primary_disk: String,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            primary_disk: "C:".to_string(),
            update_interval_ms: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp_dir() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let mut p = env::temp_dir();
        p.push(format!("topbar_test_{}", n));
        p
    }

    #[test]
    fn toml_roundtrip_default() {
        let cfg = Config::default();
        let s = toml::to_string_pretty(&cfg).expect("serialize");
        let parsed: Config = toml::from_str(&s).expect("parse");
        assert_eq!(cfg.general.language, parsed.general.language);
        assert_eq!(cfg.appearance.bar_height, parsed.appearance.bar_height);
        assert_eq!(cfg.modules.left_modules, parsed.modules.left_modules);
    }

    #[test]
    fn config_path_respects_env() {
        let tmp = unique_tmp_dir();
        env::set_var("APPDATA", &tmp);
        env::set_var("XDG_CONFIG_HOME", &tmp);
        let p = Config::config_path();
        // Ensure filename is correct and contains the topbar directory.
        let s = p.to_string_lossy();
        assert!(
            s.ends_with("topbar/config.toml") || s.ends_with("topbar\\config.toml"),
            "config path does not end with topbar/config.toml: {}",
            s
        );
        assert!(
            s.contains("topbar"),
            "config path does not contain topbar: {}",
            s
        );
    }

    #[test]
    fn save_and_load_or_default_reads_file() {
        let tmp = unique_tmp_dir();
        env::set_var("APPDATA", &tmp);
        env::set_var("XDG_CONFIG_HOME", &tmp);
        // ensure clean dir
        if tmp.exists() {
            fs::remove_dir_all(&tmp).unwrap();
        }
        // create default and modify
        let mut cfg = Config::default();
        cfg.general.language = "fr".to_string();
        cfg.save().expect("save");
        let loaded = Config::load_or_default().expect("load");
        assert_eq!(loaded.general.language, "fr");
        // cleanup
        let _ = fs::remove_dir_all(&tmp);
    }
}
