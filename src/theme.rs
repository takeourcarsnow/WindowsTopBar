//! Theming system for TopBar
//! 
//! Handles light/dark themes, colors, and visual styling.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::System::Registry::{
    RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
};
use windows::core::PCWSTR;

/// Theme mode setting
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    Auto,
}

/// RGBA Color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color from RGBA values
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create a new opaque color from RGB values
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create color from hex string (#RRGGBB or #RRGGBBAA)
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::new(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    /// Convert to COLORREF for Windows API
    pub fn to_colorref(&self) -> COLORREF {
        COLORREF(((self.b as u32) << 16) | ((self.g as u32) << 8) | (self.r as u32))
    }

    /// Convert to ARGB u32
    pub fn to_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Create with modified alpha
    pub fn with_alpha(&self, a: u8) -> Self {
        Self { a, ..*self }
    }

    /// Blend two colors
    pub fn blend(&self, other: &Color, factor: f32) -> Color {
        let factor = factor.clamp(0.0, 1.0);
        let inv = 1.0 - factor;
        Color {
            r: (self.r as f32 * inv + other.r as f32 * factor) as u8,
            g: (self.g as f32 * inv + other.g as f32 * factor) as u8,
            b: (self.b as f32 * inv + other.b as f32 * factor) as u8,
            a: (self.a as f32 * inv + other.a as f32 * factor) as u8,
        }
    }

    /// Lighten the color
    pub fn lighten(&self, amount: f32) -> Color {
        self.blend(&Color::rgb(255, 255, 255), amount)
    }

    /// Darken the color
    pub fn darken(&self, amount: f32) -> Color {
        self.blend(&Color::rgb(0, 0, 0), amount)
    }

    /// Check if color is dark
    pub fn is_dark(&self) -> bool {
        // Using relative luminance formula
        let luminance = 0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32;
        luminance < 128.0
    }
}

/// Theme colors and styling
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Whether this is a dark theme
    pub is_dark: bool,
    
    // Background colors
    pub background: Color,
    pub background_secondary: Color,
    pub background_hover: Color,
    pub background_active: Color,
    
    // Text colors
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub text_accent: Color,
    
    // Accent colors
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_active: Color,
    
    // Border colors
    pub border: Color,
    pub border_hover: Color,
    
    // Status colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    
    // Special colors
    pub shadow: Color,
    pub overlay: Color,
    
    // Module-specific colors
    pub battery_full: Color,
    pub battery_medium: Color,
    pub battery_low: Color,
    pub battery_critical: Color,
    pub battery_charging: Color,
    
    pub network_connected: Color,
    pub network_disconnected: Color,
    
    pub cpu_normal: Color,
    pub cpu_high: Color,
    pub cpu_critical: Color,
    
    pub memory_normal: Color,
    pub memory_high: Color,
    pub memory_critical: Color,
}

impl Theme {
    /// Create the light theme
    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),
            is_dark: false,
            
            // Modern translucent light background - refined for clarity
            background: Color::new(251, 251, 253, 235),  // Crisp white with ~92% opacity
            background_secondary: Color::new(242, 242, 247, 245),
            background_hover: Color::new(0, 0, 0, 10),  // Subtle hover state
            background_active: Color::new(0, 0, 0, 18),
            
            // Improved text contrast for readability
            text_primary: Color::rgb(28, 28, 30),
            text_secondary: Color::rgb(72, 72, 74),  // Better contrast
            text_disabled: Color::rgb(142, 142, 147),
            text_accent: Color::rgb(0, 113, 227),  // Refined blue
            
            // Modern accent - slightly deeper blue
            accent: Color::rgb(0, 113, 227),
            accent_hover: Color::rgb(0, 98, 204),
            accent_active: Color::rgb(0, 85, 179),
            
            border: Color::new(0, 0, 0, 12),  // Softer borders
            border_hover: Color::new(0, 0, 0, 25),
            
            // Vibrant status colors
            success: Color::rgb(40, 205, 65),
            warning: Color::rgb(255, 159, 10),
            error: Color::rgb(255, 59, 48),
            info: Color::rgb(0, 113, 227),
            
            shadow: Color::new(0, 0, 0, 25),  // Softer shadows
            overlay: Color::new(0, 0, 0, 40),
            
            // Battery colors - more vibrant
            battery_full: Color::rgb(40, 205, 65),
            battery_medium: Color::rgb(255, 204, 0),
            battery_low: Color::rgb(255, 159, 10),
            battery_critical: Color::rgb(255, 59, 48),
            battery_charging: Color::rgb(40, 205, 65),
            
            // Network status
            network_connected: Color::rgb(40, 205, 65),
            network_disconnected: Color::rgb(142, 142, 147),
            
            // System metrics - refined palette
            cpu_normal: Color::rgb(0, 113, 227),
            cpu_high: Color::rgb(255, 159, 10),
            cpu_critical: Color::rgb(255, 59, 48),
            
            memory_normal: Color::rgb(191, 90, 242),  // Vivid purple
            memory_high: Color::rgb(255, 159, 10),
            memory_critical: Color::rgb(255, 59, 48),
        }
    }

    /// Create the dark theme
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),
            is_dark: true,
            
            // Modern dark background - rich and sleek
            background: Color::new(22, 22, 24, 240),  // Deep charcoal with ~94% opacity
            background_secondary: Color::new(38, 38, 42, 250),
            background_hover: Color::new(255, 255, 255, 15),  // Subtle glow
            background_active: Color::new(255, 255, 255, 28),
            
            // Crisp white text with good secondary contrast
            text_primary: Color::rgb(250, 250, 252),
            text_secondary: Color::rgb(160, 160, 166),  // Better readability
            text_disabled: Color::rgb(88, 88, 92),
            text_accent: Color::rgb(32, 150, 255),  // Brighter for dark bg
            
            // Vibrant accent for dark mode
            accent: Color::rgb(32, 150, 255),
            accent_hover: Color::rgb(70, 170, 255),
            accent_active: Color::rgb(100, 185, 255),
            
            border: Color::new(255, 255, 255, 12),  // Subtle glass edge
            border_hover: Color::new(255, 255, 255, 28),
            
            // Vivid status colors optimized for dark
            success: Color::rgb(52, 211, 92),
            warning: Color::rgb(255, 179, 64),
            error: Color::rgb(255, 85, 74),
            info: Color::rgb(32, 150, 255),
            
            shadow: Color::new(0, 0, 0, 100),
            overlay: Color::new(0, 0, 0, 128),
            
            // Battery - bright and visible
            battery_full: Color::rgb(52, 211, 92),
            battery_medium: Color::rgb(255, 214, 64),
            battery_low: Color::rgb(255, 179, 64),
            battery_critical: Color::rgb(255, 85, 74),
            battery_charging: Color::rgb(52, 211, 92),
            
            // Network status
            network_connected: Color::rgb(52, 211, 92),
            network_disconnected: Color::rgb(128, 128, 134),
            
            // System metrics - vibrant gradients
            cpu_normal: Color::rgb(32, 150, 255),
            cpu_high: Color::rgb(255, 179, 64),
            cpu_critical: Color::rgb(255, 85, 74),
            
            memory_normal: Color::rgb(200, 110, 250),  // Rich purple
            memory_high: Color::rgb(255, 179, 64),
            memory_critical: Color::rgb(255, 85, 74),
        }
    }

    /// Get color for CPU usage percentage
    pub fn cpu_color(&self, usage: f32) -> Color {
        if usage >= 90.0 {
            self.cpu_critical
        } else if usage >= 70.0 {
            self.cpu_high
        } else {
            self.cpu_normal
        }
    }

    /// Get color for memory usage percentage
    pub fn memory_color(&self, usage: f32) -> Color {
        if usage >= 90.0 {
            self.memory_critical
        } else if usage >= 70.0 {
            self.memory_high
        } else {
            self.memory_normal
        }
    }

    /// Get color for battery percentage
    pub fn battery_color(&self, percentage: u32, is_charging: bool) -> Color {
        if is_charging {
            self.battery_charging
        } else if percentage <= 10 {
            self.battery_critical
        } else if percentage <= 20 {
            self.battery_low
        } else if percentage <= 50 {
            self.battery_medium
        } else {
            self.battery_full
        }
    }
}

/// Theme manager for handling theme switching and system theme detection
pub struct ThemeManager {
    current_theme: Theme,
    mode: ThemeMode,
    system_is_dark: AtomicBool,
}

impl ThemeManager {
    /// Create a new theme manager
    pub fn new(mode: ThemeMode) -> Self {
        let system_is_dark = detect_system_dark_mode();
        let current_theme = match mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
            ThemeMode::Auto => {
                if system_is_dark {
                    Theme::dark()
                } else {
                    Theme::light()
                }
            }
        };

        Self {
            current_theme,
            mode,
            system_is_dark: AtomicBool::new(system_is_dark),
        }
    }

    /// Get the current theme
    pub fn theme(&self) -> &Theme {
        &self.current_theme
    }

    /// Get the current theme mode
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// Set the theme mode
    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.mode = mode;
        self.update_theme();
    }

    /// Toggle between light and dark mode
    pub fn toggle(&mut self) {
        self.mode = match self.mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Auto => {
                if self.system_is_dark.load(Ordering::Relaxed) {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                }
            }
        };
        self.update_theme();
    }

    /// Check if system theme changed and update if in auto mode
    pub fn check_system_theme(&mut self) -> bool {
        let system_is_dark = detect_system_dark_mode();
        let prev = self.system_is_dark.swap(system_is_dark, Ordering::Relaxed);
        
        if prev != system_is_dark && self.mode == ThemeMode::Auto {
            self.update_theme();
            return true;
        }
        false
    }

    /// Update the current theme based on mode
    fn update_theme(&mut self) {
        self.current_theme = match self.mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
            ThemeMode::Auto => {
                if self.system_is_dark.load(Ordering::Relaxed) {
                    Theme::dark()
                } else {
                    Theme::light()
                }
            }
        };
    }

    /// Check if currently using dark theme
    pub fn is_dark(&self) -> bool {
        self.current_theme.is_dark
    }
}

/// Detect if Windows is using dark mode
fn detect_system_dark_mode() -> bool {
    unsafe {
        let mut key = windows::Win32::System::Registry::HKEY::default();
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect();
        
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        );

        if result.is_err() {
            return false;
        }

        let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
        let mut data: u32 = 1;
        let mut data_size: u32 = std::mem::size_of::<u32>() as u32;

        let result = RegQueryValueExW(
            key,
            PCWSTR::from_raw(value_name.as_ptr()),
            None,
            None,
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        );

        let _ = windows::Win32::System::Registry::RegCloseKey(key);

        if result.is_ok() {
            data == 0  // 0 means dark mode, 1 means light mode
        } else {
            false
        }
    }
}

/// Get Windows accent color
pub fn get_windows_accent_color() -> Option<Color> {
    unsafe {
        let mut key = windows::Win32::System::Registry::HKEY::default();
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\DWM\0"
            .encode_utf16()
            .collect();
        
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        );

        if result.is_err() {
            return None;
        }

        let value_name: Vec<u16> = "ColorizationColor\0".encode_utf16().collect();
        let mut data: u32 = 0;
        let mut data_size: u32 = std::mem::size_of::<u32>() as u32;

        let result = RegQueryValueExW(
            key,
            PCWSTR::from_raw(value_name.as_ptr()),
            None,
            None,
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        );

        let _ = windows::Win32::System::Registry::RegCloseKey(key);

        if result.is_ok() {
            // Data is in ARGB format
            let a = ((data >> 24) & 0xFF) as u8;
            let r = ((data >> 16) & 0xFF) as u8;
            let g = ((data >> 8) & 0xFF) as u8;
            let b = (data & 0xFF) as u8;
            Some(Color::new(r, g, b, a))
        } else {
            None
        }
    }
}
