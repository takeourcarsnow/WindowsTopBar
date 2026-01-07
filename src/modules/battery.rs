//! Battery module for displaying battery status

use std::time::Instant;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use super::Module;
use crate::utils::format_duration;

/// Battery module
pub struct BatteryModule {
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
        Self {
            cached_text: String::new(),
            battery_percent: 100,
            is_charging: false,
            is_plugged_in: false,
            seconds_remaining: None,
            has_battery: true,
            last_update: Instant::now(),
        }
    }

    /// Force an immediate update
    fn force_update(&mut self, config: &crate::config::Config) {
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
        self.cached_text = self.build_display_text(config);
        self.last_update = Instant::now();
    }

    /// Build the display text
    fn build_display_text(&self, config: &crate::config::Config) -> String {
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
                text.push_str(&format!(" {}", format_duration(secs as u64)));
            }
        }

        // We already encode charging/plug state in the leading icon, so avoid
        // duplicating the charging emoji at the end.
        text
    }

    /// Get appropriate battery icon based on level
    fn get_battery_icon(&self) -> &'static str {
        if self.is_plugged_in && !self.is_charging {
            "🔌" // Plugged in but not charging (full)
        } else if self.is_charging {
            "⚡" // Charging
        } else if self.battery_percent >= 30 {
            "🔋" // Good level
        } else {
            "🪫" // Low or critical
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

    /// Check if plugged in
    pub fn is_plugged_in(&self) -> bool {
        self.is_plugged_in
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

    fn display_text(&self, _config: &crate::config::Config) -> String {
        // Return cached text to avoid rebuilding strings unnecessarily
        self.cached_text.clone()
    }

    fn update(&mut self, config: &crate::config::Config) {
        // Update every 30 seconds
        if self.last_update.elapsed().as_secs() >= 30 {
            self.force_update(config);
        }
    }

    fn on_click(&mut self) {
        // Open power settings
        crate::utils::open_url("ms-settings:batterysaver");
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
                tooltip.push_str(&format!(
                    "\nTime remaining: {}",
                    format_duration(secs as u64)
                ));
            }
        }

        Some(tooltip)
    }

    fn is_visible(&self) -> bool {
        self.has_battery
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
