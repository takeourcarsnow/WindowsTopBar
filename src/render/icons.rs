//! Icon system for TopBar
//! 
//! Provides icons using Segoe Fluent Icons font (Windows 11)
//! and Unicode symbols as fallback.

#![allow(dead_code)]

use std::collections::HashMap;

/// Icon manager
pub struct Icons {
    icons: HashMap<String, String>,
}

impl Icons {
    /// Create new icon manager with default icons
    pub fn new() -> Self {
        let mut icons = HashMap::new();
        
        // Menu icons
        icons.insert("menu".to_string(), "☰".to_string());
        icons.insert("search".to_string(), "🔍".to_string());
        icons.insert("settings".to_string(), "⚙".to_string());
        icons.insert("close".to_string(), "✕".to_string());
        
        // System icons
        icons.insert("power".to_string(), "⏻".to_string());
        icons.insert("restart".to_string(), "↻".to_string());
        icons.insert("sleep".to_string(), "☾".to_string());
        icons.insert("lock".to_string(), "🔒".to_string());
        icons.insert("user".to_string(), "👤".to_string());
        
        // Battery icons
        icons.insert("battery".to_string(), "🔋".to_string());
        icons.insert("battery_full".to_string(), "🔋".to_string());
        icons.insert("battery_medium".to_string(), "🔋".to_string());
        icons.insert("battery_low".to_string(), "🪫".to_string());
        icons.insert("battery_empty".to_string(), "🪫".to_string());
        icons.insert("battery_charging".to_string(), "⚡".to_string());
        icons.insert("power_plug".to_string(), "🔌".to_string());
        
        // Network icons
        icons.insert("wifi".to_string(), "📶".to_string());
        icons.insert("wifi_off".to_string(), "📵".to_string());
        icons.insert("ethernet".to_string(), "🔗".to_string());
        icons.insert("globe".to_string(), "🌐".to_string());
        icons.insert("airplane".to_string(), "✈".to_string());
        
        // Volume icons
        icons.insert("volume_high".to_string(), "🔊".to_string());
        icons.insert("volume_medium".to_string(), "🔉".to_string());
        icons.insert("volume_low".to_string(), "🔈".to_string());
        icons.insert("volume_mute".to_string(), "🔇".to_string());
        
        // Media icons
        icons.insert("play".to_string(), "▶".to_string());
        icons.insert("pause".to_string(), "⏸".to_string());
        icons.insert("stop".to_string(), "⏹".to_string());
        icons.insert("previous".to_string(), "⏮".to_string());
        icons.insert("next".to_string(), "⏭".to_string());
        icons.insert("music".to_string(), "🎵".to_string());
        
        // Weather icons
        icons.insert("sun".to_string(), "☀".to_string());
        icons.insert("cloud".to_string(), "☁".to_string());
        icons.insert("partly_cloudy".to_string(), "⛅".to_string());
        icons.insert("rain".to_string(), "🌧".to_string());
        icons.insert("thunderstorm".to_string(), "⛈".to_string());
        icons.insert("snow".to_string(), "❄".to_string());
        icons.insert("fog".to_string(), "🌫".to_string());
        icons.insert("temperature".to_string(), "🌡".to_string());
        
        // Status icons
        icons.insert("info".to_string(), "ℹ".to_string());
        icons.insert("warning".to_string(), "⚠".to_string());
        icons.insert("error".to_string(), "❌".to_string());
        icons.insert("success".to_string(), "✓".to_string());
        icons.insert("notification".to_string(), "🔔".to_string());
        icons.insert("notification_off".to_string(), "🔕".to_string());
        
        // Arrow icons
        icons.insert("arrow_up".to_string(), "↑".to_string());
        icons.insert("arrow_down".to_string(), "↓".to_string());
        icons.insert("arrow_left".to_string(), "←".to_string());
        icons.insert("arrow_right".to_string(), "→".to_string());
        icons.insert("chevron_down".to_string(), "⌄".to_string());
        icons.insert("chevron_right".to_string(), "›".to_string());
        
        // Misc icons
        icons.insert("calendar".to_string(), "📅".to_string());
        icons.insert("clock".to_string(), "🕐".to_string());
        icons.insert("folder".to_string(), "📁".to_string());
        icons.insert("file".to_string(), "📄".to_string());
        icons.insert("app".to_string(), "⬜".to_string());
        icons.insert("window".to_string(), "🗗".to_string());
        icons.insert("maximize".to_string(), "🗖".to_string());
        icons.insert("minimize".to_string(), "🗕".to_string());
        icons.insert("cpu".to_string(), "⬡".to_string());
        icons.insert("memory".to_string(), "⬢".to_string());
        icons.insert("disk".to_string(), "💾".to_string());
        icons.insert("gpu".to_string(), "🎮".to_string());
        icons.insert("bluetooth".to_string(), "📶".to_string());
        
        Self { icons }
    }

    /// Get an icon by name
    pub fn get(&self, name: &str) -> String {
        self.icons
            .get(name)
            .cloned()
            .unwrap_or_else(|| "?".to_string())
    }

    /// Add or update an icon
    pub fn set(&mut self, name: &str, icon: &str) {
        self.icons.insert(name.to_string(), icon.to_string());
    }

    /// Check if an icon exists
    pub fn has(&self, name: &str) -> bool {
        self.icons.contains_key(name)
    }

    /// Get all icon names
    pub fn names(&self) -> Vec<&str> {
        self.icons.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::new()
    }
}

/// Segoe Fluent Icons font characters (Windows 11)
/// These are the actual Unicode code points for Segoe Fluent Icons
pub mod fluent {
    pub const WIFI: char = '\u{E701}';
    pub const WIFI_OFF: char = '\u{F384}';
    pub const ETHERNET: char = '\u{E839}';
    pub const AIRPLANE: char = '\u{E709}';
    
    pub const BATTERY_0: char = '\u{E850}';
    pub const BATTERY_1: char = '\u{E851}';
    pub const BATTERY_2: char = '\u{E852}';
    pub const BATTERY_3: char = '\u{E853}';
    pub const BATTERY_4: char = '\u{E854}';
    pub const BATTERY_5: char = '\u{E855}';
    pub const BATTERY_6: char = '\u{E856}';
    pub const BATTERY_7: char = '\u{E857}';
    pub const BATTERY_8: char = '\u{E858}';
    pub const BATTERY_9: char = '\u{E859}';
    pub const BATTERY_10: char = '\u{E83F}';
    pub const BATTERY_CHARGING: char = '\u{E83E}';
    pub const BATTERY_SAVER: char = '\u{E853}';
    
    pub const VOLUME_0: char = '\u{E992}';
    pub const VOLUME_1: char = '\u{E993}';
    pub const VOLUME_2: char = '\u{E994}';
    pub const VOLUME_3: char = '\u{E995}';
    pub const VOLUME_MUTE: char = '\u{E74F}';
    
    pub const BRIGHTNESS: char = '\u{E706}';
    pub const BLUETOOTH: char = '\u{E702}';
    
    pub const SETTINGS: char = '\u{E713}';
    pub const SEARCH: char = '\u{E721}';
    pub const POWER: char = '\u{E7E8}';
    pub const LOCK: char = '\u{E72E}';
    pub const SIGNOUT: char = '\u{F3B1}';
    
    pub const PLAY: char = '\u{E768}';
    pub const PAUSE: char = '\u{E769}';
    pub const STOP: char = '\u{E71A}';
    pub const PREVIOUS: char = '\u{E892}';
    pub const NEXT: char = '\u{E893}';
    
    pub const CHEVRON_DOWN: char = '\u{E70D}';
    pub const CHEVRON_UP: char = '\u{E70E}';
    pub const CHEVRON_LEFT: char = '\u{E76B}';
    pub const CHEVRON_RIGHT: char = '\u{E76C}';
    
    pub const CHECK: char = '\u{E73E}';
    pub const CLOSE: char = '\u{E711}';
    pub const MORE: char = '\u{E712}';
    
    pub const INFO: char = '\u{E946}';
    pub const WARNING: char = '\u{E7BA}';
    pub const ERROR: char = '\u{E783}';
    
    pub const CALENDAR: char = '\u{E787}';
    pub const CLOCK: char = '\u{E823}';
    pub const NOTIFICATION: char = '\u{E7E7}';
    
    pub const WINDOW: char = '\u{E737}';
    pub const MAXIMIZE: char = '\u{E739}';
    pub const MINIMIZE: char = '\u{E738}';
    pub const RESTORE: char = '\u{E923}';
}

/// Get battery icon for percentage
pub fn battery_icon_for_percent(percent: u32, is_charging: bool) -> char {
    if is_charging {
        return fluent::BATTERY_CHARGING;
    }
    
    match percent {
        0..=5 => fluent::BATTERY_0,
        6..=15 => fluent::BATTERY_1,
        16..=25 => fluent::BATTERY_2,
        26..=35 => fluent::BATTERY_3,
        36..=45 => fluent::BATTERY_4,
        46..=55 => fluent::BATTERY_5,
        56..=65 => fluent::BATTERY_6,
        66..=75 => fluent::BATTERY_7,
        76..=85 => fluent::BATTERY_8,
        86..=95 => fluent::BATTERY_9,
        _ => fluent::BATTERY_10,
    }
}

/// Get volume icon for level
pub fn volume_icon_for_level(level: u32, is_muted: bool) -> char {
    if is_muted || level == 0 {
        return fluent::VOLUME_MUTE;
    }
    
    match level {
        1..=33 => fluent::VOLUME_1,
        34..=66 => fluent::VOLUME_2,
        _ => fluent::VOLUME_3,
    }
}

/// Get WiFi icon for signal strength
pub fn wifi_icon_for_strength(_strength: u32, is_connected: bool) -> char {
    if !is_connected {
        return fluent::WIFI_OFF;
    }
    
    // Could return different icons based on strength
    fluent::WIFI
}
