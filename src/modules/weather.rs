//! Weather module for displaying weather information

use std::time::Instant;

use super::Module;
use crate::config::TemperatureUnit;

/// Weather condition
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Overcast,
    Rain,
    HeavyRain,
    Thunderstorm,
    Snow,
    Fog,
    Windy,
    Unknown,
}

impl WeatherCondition {
    /// Get icon for weather condition
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Clear => "☀️",
            Self::PartlyCloudy => "⛅",
            Self::Cloudy => "☁️",
            Self::Overcast => "☁️",
            Self::Rain => "🌧️",
            Self::HeavyRain => "🌧️",
            Self::Thunderstorm => "⛈️",
            Self::Snow => "❄️",
            Self::Fog => "🌫️",
            Self::Windy => "💨",
            Self::Unknown => "🌡️",
        }
    }
}

/// Weather data
#[derive(Debug, Clone)]
pub struct WeatherData {
    pub temperature: f32,
    pub feels_like: f32,
    pub humidity: u32,
    pub condition: WeatherCondition,
    pub description: String,
    pub location: String,
    pub high: f32,
    pub low: f32,
}

impl Default for WeatherData {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            feels_like: 0.0,
            humidity: 0,
            condition: WeatherCondition::Unknown,
            description: String::new(),
            location: String::new(),
            high: 0.0,
            low: 0.0,
        }
    }
}

/// Weather module
pub struct WeatherModule {
    cached_text: String,
    enabled: bool,
    unit: TemperatureUnit,
    show_icon: bool,
    weather_data: Option<WeatherData>,
    location: String,
    last_update: Instant,
    update_interval_min: u32,
}

impl WeatherModule {
    pub fn new() -> Self {
        Self {
            cached_text: String::new(),
            enabled: false,  // Disabled by default since it needs API key
            unit: TemperatureUnit::Celsius,
            show_icon: true,
            weather_data: None,
            location: "auto".to_string(),
            last_update: Instant::now(),
            update_interval_min: 30,
        }
    }

    /// Enable/disable the module
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set temperature unit
    pub fn set_unit(&mut self, unit: TemperatureUnit) {
        self.unit = unit;
    }

    /// Set location
    pub fn set_location(&mut self, location: &str) {
        self.location = location.to_string();
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        if !self.enabled {
            self.cached_text = String::new();
            return;
        }

        // In a full implementation, this would fetch weather data from an API
        // For now, we'll use placeholder data
        
        // Simulate weather data (would come from API)
        // You would implement actual API calls here, e.g., to OpenWeatherMap
        
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        let Some(data) = &self.weather_data else {
            return String::new();
        };

        let mut text = String::new();

        if self.show_icon {
            text.push_str(data.condition.icon());
            text.push(' ');
        }

        let temp = match self.unit {
            TemperatureUnit::Celsius => data.temperature,
            TemperatureUnit::Fahrenheit => data.temperature * 9.0 / 5.0 + 32.0,
        };

        let unit_symbol = match self.unit {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        };

        text.push_str(&format!("{:.0}{}", temp, unit_symbol));

        text
    }

    /// Convert temperature to display unit
    fn convert_temp(&self, celsius: f32) -> f32 {
        match self.unit {
            TemperatureUnit::Celsius => celsius,
            TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
        }
    }

    /// Get weather data
    pub fn weather_data(&self) -> Option<&WeatherData> {
        self.weather_data.as_ref()
    }
}

impl Default for WeatherModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for WeatherModule {
    fn id(&self) -> &str {
        "weather"
    }

    fn name(&self) -> &str {
        "Weather"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        self.cached_text.clone()
    }

    fn update(&mut self) {
        // Update based on configured interval
        if self.last_update.elapsed().as_secs() >= (self.update_interval_min * 60) as u64 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Open weather app or website
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "https://weather.com"])
            .spawn();
    }

    fn tooltip(&self) -> Option<String> {
        let Some(data) = &self.weather_data else {
            return Some("Weather data not available".to_string());
        };

        let unit = match self.unit {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        };

        Some(format!(
            "{}\n{}\n\nTemperature: {:.0}{}\nFeels like: {:.0}{}\nHumidity: {}%\nHigh: {:.0}{} / Low: {:.0}{}",
            data.location,
            data.description,
            self.convert_temp(data.temperature), unit,
            self.convert_temp(data.feels_like), unit,
            data.humidity,
            self.convert_temp(data.high), unit,
            self.convert_temp(data.low), unit,
        ))
    }

    fn is_visible(&self) -> bool {
        self.enabled && self.weather_data.is_some()
    }
}
