//! Uptime module - shows system uptime

use std::time::Instant;
use windows::Win32::System::SystemInformation::GetTickCount64;

use super::Module;

/// Uptime module
pub struct UptimeModule {
    cached_text: String,
    uptime_secs: u64,
    last_update: Instant,
}

impl UptimeModule {
    pub fn new() -> Self {
        let module = Self {
            cached_text: String::new(),
            uptime_secs: 0,
            last_update: Instant::now(),
        };
        module
    }

    /// Force an immediate update
    fn force_update(&mut self, config: &crate::config::Config) {
        // GetTickCount64 returns milliseconds since system start
        self.uptime_secs = unsafe { GetTickCount64() / 1000 };
        self.cached_text = self.build_display_text(config);
        self.last_update = Instant::now();
    }

    /// Build the display text
    fn build_display_text(&self, _config: &crate::config::Config) -> String {
        // Always use compact format and show days when present
        let days = self.uptime_secs / 86400;
        let hours = (self.uptime_secs % 86400) / 3600;
        let minutes = (self.uptime_secs % 3600) / 60;

        if days > 0 {
            format!("⏱ {}d {}h", days, hours)
        } else if hours > 0 {
            format!("⏱ {}h {}m", hours, minutes)
        } else {
            format!("⏱ {}m", minutes)
        }
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        self.uptime_secs
    }

    /// Get formatted uptime string (full)
    pub fn formatted_full(&self) -> String {
        let days = self.uptime_secs / 86400;
        let hours = (self.uptime_secs % 86400) / 3600;
        let minutes = (self.uptime_secs % 3600) / 60;
        let seconds = self.uptime_secs % 60;

        if days > 0 {
            format!("{} days, {} hours, {} minutes, {} seconds", days, hours, minutes, seconds)
        } else if hours > 0 {
            format!("{} hours, {} minutes, {} seconds", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{} minutes, {} seconds", minutes, seconds)
        } else {
            format!("{} seconds", seconds)
        }
    }
}

impl Default for UptimeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for UptimeModule {
    fn id(&self) -> &str {
        "uptime"
    }

    fn name(&self) -> &str {
        "System Uptime"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        self.cached_text.clone()
    }

    fn update(&mut self, config: &crate::config::Config) {
        // Update every minute, or immediately if we haven't updated yet
        if self.uptime_secs == 0 || self.last_update.elapsed().as_secs() >= 60 {
            self.force_update(config);
        }
    }

    fn on_click(&mut self) {
        // Open system information
        let _ = std::process::Command::new("msinfo32.exe").spawn();
    }

    fn tooltip(&self) -> Option<String> {
        Some(format!("System Uptime\n{}", self.formatted_full()))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
