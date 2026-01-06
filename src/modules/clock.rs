//! Clock module for displaying time and date

use chrono::{Local, Timelike, Datelike};

use super::Module;

/// Clock module displaying time and date
pub struct ClockModule {
    format_24h: bool,
    show_seconds: bool,
    show_date: bool,
    show_day: bool,
    cached_text: String,
    last_update: std::time::Instant,
}

impl ClockModule {
    pub fn new() -> Self {
        let mut module = Self {
            format_24h: false,
            show_seconds: false,
            show_date: true,
            show_day: true,
            cached_text: String::new(),
            last_update: std::time::Instant::now(),
        };
        module.update();
        module
    }

    /// Set time format (12h or 24h)
    pub fn set_format_24h(&mut self, format_24h: bool) {
        self.format_24h = format_24h;
    }

    /// Set whether to show seconds
    pub fn set_show_seconds(&mut self, show: bool) {
        self.show_seconds = show;
    }

    /// Set whether to show date
    pub fn set_show_date(&mut self, show: bool) {
        self.show_date = show;
    }

    /// Set whether to show day of week
    pub fn set_show_day(&mut self, show: bool) {
        self.show_day = show;
    }

    /// Format the current time
    fn format_time(&self) -> String {
        let now = Local::now();
        
        let time_str = if self.format_24h {
            if self.show_seconds {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%H:%M").to_string()
            }
        } else {
            if self.show_seconds {
                now.format("%I:%M:%S %p").to_string()
            } else {
                now.format("%I:%M %p").to_string()
            }
        };

        let mut result = String::new();

        if self.show_day {
            result.push_str(&now.format("%a").to_string());
            result.push(' ');
        }

        if self.show_date {
            result.push_str(&now.format("%b %d").to_string());
            result.push_str("  ");
        }

        result.push_str(&time_str);
        result
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

    fn display_text(&self, config: &crate::config::Config) -> String {
        let now = Local::now();
        
        let time_str = if config.modules.clock.format_24h {
            if config.modules.clock.show_seconds {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%H:%M").to_string()
            }
        } else {
            if config.modules.clock.show_seconds {
                now.format("%I:%M:%S %p").to_string()
            } else {
                now.format("%I:%M %p").to_string()
            }
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

    fn update(&mut self) {
        // Time updates automatically
    }

    fn on_click(&mut self) {
        // Could open calendar widget
    }

    fn tooltip(&self) -> Option<String> {
        let now = Local::now();
        Some(now.format("%A, %B %d, %Y\n%I:%M:%S %p").to_string())
    }
}
