//! Clock module for displaying time and date

use chrono::Local;
use std::time::Instant;

use super::Module;

/// Clock module displaying time and date
pub struct ClockModule {
    cached_text: String,
    last_update: std::time::Instant,
}

impl ClockModule {
    pub fn new() -> Self {
        let module = Self {
            cached_text: String::new(),
            last_update: std::time::Instant::now(),
        };
        module
    }

    /// Format the current time
    fn format_time(&self, config: &crate::config::Config) -> String {
        let now = Local::now();
        
        let time_str = if config.modules.clock.format_24h {
            if config.modules.clock.show_seconds {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%H:%M").to_string()
            }
        } else if config.modules.clock.show_seconds {
            now.format("%I:%M:%S %p").to_string()
        } else {
            now.format("%I:%M %p").to_string()
        };

        let mut result = String::new();

        if config.modules.clock.show_day {
            result.push_str(&now.format("%a").to_string());
            result.push(' ');
        }

        if config.modules.clock.show_date {
            result.push_str(&now.format("%b %d").to_string());
            result.push_str("  ");
        }

        result.push_str(&time_str);
        result
    }

    /// Build the display text
    fn build_display_text(&self, config: &crate::config::Config) -> String {
        self.format_time(config)
    }
}

impl Default for ClockModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ClockModule {
    fn id(&self) -> &str {
        "clock"
    }

    fn name(&self) -> &str {
        "Clock"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        self.cached_text.clone()
    }

    fn update(&mut self, config: &crate::config::Config) {
        // Update cached text
        self.cached_text = self.build_display_text(config);
        self.last_update = Instant::now();
    }

    fn on_click(&mut self) {
        // Could open calendar widget
    }

    fn tooltip(&self) -> Option<String> {
        let now = Local::now();
        Some(now.format("%A, %B %d, %Y\n%I:%M:%S %p").to_string())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
