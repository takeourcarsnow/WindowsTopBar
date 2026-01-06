//! Battery module for displaying battery status

use std::time::Instant;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use super::Module;
use crate::utils::format_duration;

/// Battery module
pub struct BatteryModule {
    show_percentage: bool,
    show_time_remaining: bool,
    cached_text: String,
    battery_percent: u32,
    is_charging: bool,
    is_plugged_in: bool,
    seconds_remaining: Option<u32>,
    has_battery: bool,
    last_update: Instant,
}

impl BatteryModule {
    pub fn new() -> Self {
        let mut module = Self {
            show_percentage: true,
            show_time_remaining: false,
            cached_text: String::new(),
            battery_percent: 100,
            is_charging: false,
            is_plugged_in: false,
            seconds_remaining: None,
            has_battery: true,
            last_update: Instant::now(),
        };
        module.force_update();
        module
    }

    /// Set whether to show percentage
    pub fn set_show_percentage(&mut self, show: bool) {
        self.show_percentage = show;
    }

    /// Set whether to show time remaining
    pub fn set_show_time_remaining(&mut self, show: bool) {
        self.show_time_remaining = show;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        unsafe {
            let mut status = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut status).is_ok() {
                // Check if battery is present
                // BatteryFlag: 128 = no battery, 255 = unknown
                self.has_battery = status.BatteryFlag != 128 && status.BatteryFlag != 255;

                if self.has_battery {
                    // Battery percentage (255 = unknown)
                    if status.BatteryLifePercent != 255 {
                        self.battery_percent = status.BatteryLifePercent as u32;
                    }

                    // Charging status
                    // BatteryFlag: 8 = charging
                    self.is_charging = (status.BatteryFlag & 8) != 0;
                    
                    // AC power status (1 = plugged in)
                    self.is_plugged_in = status.ACLineStatus == 1;

                    // Time remaining (in seconds, -1 = unknown)
                    if status.BatteryLifeTime != u32::MAX {
                        self.seconds_remaining = Some(status.BatteryLifeTime);
                    } else {
                        self.seconds_remaining = None;
                    }
                }
            }
        }

        // Build display text
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        if !self.has_battery {
            return String::new();
        }

        let icon = self.get_battery_icon();
        let mut text = icon.to_string();

        if self.show_percentage {
            text.push_str(&format!(" {}%", self.battery_percent));
        }

        if self.show_time_remaining {
            if let Some(secs) = self.seconds_remaining {
                text.push_str(&format!(" ({})", format_duration(secs as u64)));
            }
        }

        if self.is_charging {
            text.push_str(" ⚡");
        }

        text
    }

    /// Get appropriate battery icon based on level
    fn get_battery_icon(&self) -> &'static str {
        if self.is_plugged_in && !self.is_charging {
            "🔌"
        } else if self.is_charging {
            "🔋"
        } else if self.battery_percent >= 80 {
            "🔋"
        } else if self.battery_percent >= 60 {
            "🔋"
        } else if self.battery_percent >= 40 {
            "🔋"
        } else if self.battery_percent >= 20 {
            "🪫"
        } else {
            "🪫"
        }
    }

    /// Get battery percentage
    pub fn battery_percent(&self) -> u32 {
        self.battery_percent
    }

    /// Check if charging
    pub fn is_charging(&self) -> bool {
        self.is_charging
    }

    /// Check if has battery
    pub fn has_battery(&self) -> bool {
        self.has_battery
    }
}

impl Default for BatteryModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BatteryModule {
    fn id(&self) -> &str {
        "battery"
    }

    fn name(&self) -> &str {
        "Battery"
    }

    fn display_text(&self, config: &crate::config::Config) -> String {
        if !self.has_battery {
            return String::new();
        }

        let icon = self.get_battery_icon();
        let mut text = icon.to_string();

        if config.modules.battery.show_percentage {
            text.push_str(&format!(" {}%", self.battery_percent));
        }

        if config.modules.battery.show_time_remaining {
            if let Some(secs) = self.seconds_remaining {
                text.push_str(&format!(" ({})", format_duration(secs as u64)));
            }
        }

        if self.is_charging {
            text.push_str(" ⚡");
        }

        text
    }

    fn update(&mut self) {
        // Update every 30 seconds
        if self.last_update.elapsed().as_secs() >= 30 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Open power settings
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:batterysaver"])
            .spawn();
    }

    fn tooltip(&self) -> Option<String> {
        if !self.has_battery {
            return Some("No battery detected".to_string());
        }

        let status = if self.is_charging {
            "Charging"
        } else if self.is_plugged_in {
            "Plugged in"
        } else {
            "On battery"
        };

        let mut tooltip = format!("Battery: {}%\nStatus: {}", self.battery_percent, status);

        if let Some(secs) = self.seconds_remaining {
            if !self.is_charging {
                tooltip.push_str(&format!("\nTime remaining: {}", format_duration(secs as u64)));
            }
        }

        Some(tooltip)
    }

    fn is_visible(&self) -> bool {
        self.has_battery
    }
}
