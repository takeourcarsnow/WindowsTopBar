//! Weather module for displaying weather information
//!
//! Uses wttr.in service for weather data - no API key required!
//! Supports automatic location detection or custom city input.

#![allow(dead_code)]

use log::{error, info};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use super::Module;
use crate::config::TemperatureUnit;
use chrono::{Local, NaiveDate};

/// Weather condition codes from wttr.in (WWO codes)
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
            Self::Clear => "☀", // Sun
            // Cloudy-like conditions use a single cloud glyph
            Self::PartlyCloudy | Self::Cloudy | Self::Overcast | Self::Fog | Self::Windy => "☁",
            // Rain vs Thunderstorm are distinct (umbrella vs lightning)
            Self::Rain | Self::HeavyRain => "☂",
            Self::Thunderstorm => "⚡",
            Self::Snow => "❄",
            // Hide icon for unknown to avoid bad rendering
            Self::Unknown => "",
        }
    }

    /// Parse from wttr.in WWO weather code
    fn from_wwo_code(code: u32) -> Self {
        match code {
            113 => Self::Clear,                                   // Sunny/Clear
            116 => Self::PartlyCloudy,                            // Partly cloudy
            119 => Self::Cloudy,                                  // Cloudy
            122 => Self::Overcast,                                // Overcast
            143 | 248 | 260 => Self::Fog,                         // Mist/Fog/Freezing fog
            176 | 263 | 266 | 293 | 296 => Self::Rain,            // Light rain variants
            299 | 302 | 305 | 308 | 356 | 359 => Self::HeavyRain, // Heavy rain
            200 | 386 | 389 | 392 | 395 => Self::Thunderstorm,    // Thunder variants
            179 | 182 | 185 | 227 | 230 | 281 | 284 | 311 | 314 | 317 | 320 | 323 | 326 | 329
            | 332 | 335 | 338 | 350 | 353 | 362 | 365 | 368 | 371 | 374 | 377 => Self::Snow,
            _ => Self::Unknown,
        }
    }
}

/// Daily forecast entry
#[derive(Debug, Clone)]
pub struct DailyForecast {
    pub date: String,
    pub max: f32,
    pub min: f32,
    pub description: String,
    pub condition: WeatherCondition,
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
    pub wind_speed: f32,
    pub wind_dir: String,
    pub forecast: Vec<DailyForecast>,
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
            wind_speed: 0.0,
            wind_dir: String::new(),
            forecast: Vec::new(),
        }
    }
}

/// Weather fetch status
#[derive(Debug, Clone, PartialEq)]
pub enum FetchStatus {
    Idle,
    Fetching,
    Success,
    Error(String),
    LocationNotFound,
}

/// Weather module
pub struct WeatherModule {
    cached_text: String,
    enabled: bool,
    unit: TemperatureUnit,
    show_icon: bool,
    weather_data: Arc<Mutex<Option<WeatherData>>>,
    location: String,
    last_update: Instant,
    update_interval_min: u32,
    fetch_status: Arc<Mutex<FetchStatus>>,
    is_fetching: Arc<Mutex<bool>>,
}

impl WeatherModule {
    pub fn new() -> Self {
        let module = Self {
            cached_text: "...".to_string(), // Show loading indicator initially
            enabled: true,                     // Enabled by default - no API key needed!
            unit: TemperatureUnit::Celsius,
            show_icon: true,
            weather_data: Arc::new(Mutex::new(None)),
            location: "auto".to_string(), // Auto-detect by default
            last_update: Instant::now() - std::time::Duration::from_secs(3600), // Force initial update
            update_interval_min: 30,
            fetch_status: Arc::new(Mutex::new(FetchStatus::Idle)),
            is_fetching: Arc::new(Mutex::new(false)),
        };

        // Trigger initial fetch
        module.fetch_weather_initial();

        module
    }

    /// Initial weather fetch (called from new())
    fn fetch_weather_initial(&self) {
        let location = self.location.clone();
        let weather_data = Arc::clone(&self.weather_data);
        let fetch_status = Arc::clone(&self.fetch_status);
        let is_fetching = Arc::clone(&self.is_fetching);

        // Set fetching status
        *fetch_status.lock().unwrap() = FetchStatus::Fetching;
        *is_fetching.lock().unwrap() = true;

        std::thread::spawn(move || {
            let result = Self::fetch_weather_sync(&location);

            match result {
                Ok(data) => {
                    info!(
                        "Weather fetched for {}: {}°C, {}",
                        data.location, data.temperature, data.description
                    );
                    *weather_data.lock().unwrap() = Some(data);
                    *fetch_status.lock().unwrap() = FetchStatus::Success;
                }
                Err(e) => {
                    error!("Failed to fetch weather: {}", e);
                    if e.contains("not found") || e.contains("Unknown location") {
                        *fetch_status.lock().unwrap() = FetchStatus::LocationNotFound;
                    } else {
                        *fetch_status.lock().unwrap() = FetchStatus::Error(e);
                    }
                }
            }

            *is_fetching.lock().unwrap() = false;
        });
    }

    /// Enable/disable the module
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled && self.weather_data.lock().unwrap().is_none() {
            self.fetch_weather_async();
        }
    }

    /// Set temperature unit
    pub fn set_unit(&mut self, unit: TemperatureUnit) {
        self.unit = unit;
    }

    /// Set location - use city name like "London", "New York", "Tokyo"
    /// Or "auto" for automatic detection based on IP
    pub fn set_location(&mut self, location: &str) {
        let new_location = location.trim().to_string();
        if self.location != new_location {
            self.location = new_location;
            // Clear cached data and fetch new
            *self.weather_data.lock().unwrap() = None;
            self.fetch_weather_async();
        }
    }

    /// Get current location setting
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Get fetch status
    pub fn fetch_status(&self) -> FetchStatus {
        self.fetch_status.lock().unwrap().clone()
    }

    /// Fetch weather data asynchronously using wttr.in
    fn fetch_weather_async(&mut self) {
        // Check if already fetching
        {
            let mut is_fetching = self.is_fetching.lock().unwrap();
            if *is_fetching {
                return;
            }
            *is_fetching = true;
        }

        *self.fetch_status.lock().unwrap() = FetchStatus::Fetching;

        let location = self.location.clone();
        let weather_data = Arc::clone(&self.weather_data);
        let fetch_status = Arc::clone(&self.fetch_status);
        let is_fetching = Arc::clone(&self.is_fetching);

        thread::spawn(move || {
            let result = Self::fetch_weather_sync(&location);

            match result {
                Ok(data) => {
                    info!(
                        "Weather fetched for {}: {}°C, {}",
                        data.location, data.temperature, data.description
                    );
                    *weather_data.lock().unwrap() = Some(data);
                    *fetch_status.lock().unwrap() = FetchStatus::Success;
                }
                Err(e) => {
                    error!("Failed to fetch weather: {}", e);
                    if e.contains("not found") || e.contains("Unknown location") {
                        *fetch_status.lock().unwrap() = FetchStatus::LocationNotFound;
                    } else {
                        *fetch_status.lock().unwrap() = FetchStatus::Error(e);
                    }
                }
            }

            *is_fetching.lock().unwrap() = false;
        });

        self.last_update = Instant::now();
    }

    /// Synchronous weather fetch using wttr.in JSON API
    fn fetch_weather_sync(location: &str) -> Result<WeatherData, String> {
        // Build URL - wttr.in supports city names directly
        // Format: ?format=j1 returns JSON data
        let loc = if location.eq_ignore_ascii_case("auto") || location.is_empty() {
            String::new() // Empty = auto-detect
        } else {
            // URL encode the location
            location.replace(' ', "+")
        };

        let url = format!("https://wttr.in/{}?format=j1", loc);

        info!("Fetching weather from: {}", url);

        // Make HTTP request
        let response = ureq::get(&url)
            .set("User-Agent", "TopBar/1.0")
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .map_err(|e| format!("HTTP error: {}", e))?;

        let body = response
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Parse JSON response
        Self::parse_wttr_response(&body)
    }

    /// Parse wttr.in JSON response
    fn parse_wttr_response(json: &str) -> Result<WeatherData, String> {
        let parsed: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("JSON parse error: {}", e))?;

        // Check for error response
        if let Some(error) = parsed.get("error") {
            return Err(format!("Unknown location: {}", error));
        }

        // Get current condition
        let current = parsed
            .get("current_condition")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .ok_or("Missing current_condition")?;

        // Get location info
        let nearest_area = parsed
            .get("nearest_area")
            .and_then(|n| n.as_array())
            .and_then(|arr| arr.first())
            .ok_or("Missing nearest_area")?;

        let area_name = nearest_area
            .get("areaName")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let country = nearest_area
            .get("country")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Get weather data
        let temp_c = current
            .get("temp_C")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.0);

        let feels_like = current
            .get("FeelsLikeC")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(temp_c);

        let humidity = current
            .get("humidity")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let weather_code = current
            .get("weatherCode")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let description = current
            .get("weatherDesc")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let wind_speed = current
            .get("windspeedKmph")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.0);

        let wind_dir = current
            .get("winddir16Point")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Get today's forecast for high/low
        let weather = parsed
            .get("weather")
            .and_then(|w| w.as_array())
            .and_then(|arr| arr.first());

        let (high, low) = if let Some(today) = weather {
            let h = today
                .get("maxtempC")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(temp_c);
            let l = today
                .get("mintempC")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(temp_c);
            (h, l)
        } else {
            (temp_c, temp_c)
        };

        // Build multi-day forecast list (use up to 5 days)
        let mut forecasts: Vec<DailyForecast> = Vec::new();
        if let Some(days) = parsed.get("weather").and_then(|w| w.as_array()) {
            for day in days.iter().take(5) {
                let date = day.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let max = day
                    .get("maxtempC")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(temp_c);
                let min = day
                    .get("mintempC")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(temp_c);

                let desc = day
                    .get("hourly")
                    .and_then(|h| h.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|h0| h0.get("weatherDesc"))
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let code = day
                    .get("hourly")
                    .and_then(|h| h.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|h0| h0.get("weatherCode"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);

                forecasts.push(DailyForecast {
                    date,
                    max,
                    min,
                    description: desc,
                    condition: WeatherCondition::from_wwo_code(code),
                });
            }
        }

        let location_str = if country.is_empty() {
            area_name.to_string()
        } else {
            format!("{}, {}", area_name, country)
        };

        Ok(WeatherData {
            temperature: temp_c,
            feels_like,
            humidity,
            condition: WeatherCondition::from_wwo_code(weather_code),
            description,
            location: location_str,
            high,
            low,
            wind_speed,
            wind_dir,
            forecast: forecasts,
        })
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        if !self.enabled {
            self.cached_text = String::new();
            return;
        }

        self.cached_text = self.build_display_text();
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        let data_guard = self.weather_data.lock().unwrap();
        let Some(data) = data_guard.as_ref() else {
            // Show status while loading
            let status = self.fetch_status.lock().unwrap();
            return match &*status {
                FetchStatus::Fetching => "...".to_string(),
                FetchStatus::LocationNotFound => "Set location".to_string(),
                FetchStatus::Error(_) => "Error".to_string(),
                _ => String::new(),
            };
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

    /// Public helper: Format forecast date into relative terms (Today/Tomorrow/Weekday)
    pub fn relative_date_label(date_str: &str) -> String {
        // Expect date in YYYY-MM-DD format from wttr.in
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            let today = Local::now().date_naive();
            let days = (date - today).num_days();
            match days {
                0 => "Today".to_string(),
                1 => "Tomorrow".to_string(),
                -1 => "Yesterday".to_string(),
                2..=6 => date.format("%A").to_string(),
                _ => date.format("%Y-%m-%d").to_string(),
            }
        } else {
            date_str.to_string()
        }
    }

    // Instance helper removed; use the associated function `WeatherModule::relative_date_label(...)` instead.

    /// Get weather data
    pub fn weather_data(&self) -> Option<WeatherData> {
        self.weather_data.lock().unwrap().clone()
    }

    /// Manually trigger a refresh
    pub fn refresh(&mut self) {
        self.fetch_weather_async();
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

    fn update(&mut self, _config: &crate::config::Config) {
        // Update cached text from weather data
        self.cached_text = self.build_display_text();

        // Fetch new data based on configured interval
        if self.last_update.elapsed().as_secs() >= (self.update_interval_min * 60) as u64 {
            self.fetch_weather_async();
        }
    }

    fn on_click(&mut self) {
        // Show an in-app forecast popup with upcoming days, fall back to opening wttr.in
        let data_opt = self.weather_data.lock().unwrap().clone();
        if let Some(data) = data_opt {
            // Build message string
            let mut msg = format!("{} — {}\n\n", data.location, data.description);
            msg.push_str("Upcoming forecast:\n");
            for fc in data.forecast.iter() {
                let max = self.convert_temp(fc.max);
                let min = self.convert_temp(fc.min);
                let icon = fc.condition.icon();
                let label = WeatherModule::relative_date_label(&fc.date);
                let line = format!("{} {} {:.0}° / {:.0}° - {}\n", label, icon, max, min, fc.description);
                msg.push_str(&line);
            }
            msg.push_str("\nOpen full forecast in browser?");

            // Show MessageBox with Yes/No
            use crate::utils::to_wide_string;
            use windows::core::PCWSTR;
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONINFORMATION, IDYES};

            let title = to_wide_string("Weather");
            let text = to_wide_string(&msg);

            let resp = unsafe { MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), MB_YESNO | MB_ICONINFORMATION) };
            if resp == IDYES {
                let location = if self.location.eq_ignore_ascii_case("auto") {
                    String::new()
                } else {
                    self.location.replace(' ', "+")
                };
                let url = format!("https://wttr.in/{}", location);
                crate::utils::open_url(&url);

            }
        } else {
            // No data yet - request fetch and open website
            self.fetch_weather_async();
            let location = if self.location.eq_ignore_ascii_case("auto") {
                String::new()
            } else {
                self.location.replace(' ', "+")
            };
            let url = format!("https://wttr.in/{}", location);
            crate::utils::open_url(&url);

        }
    }

    fn tooltip(&self) -> Option<String> {
        let data_guard = self.weather_data.lock().unwrap();
        let Some(data) = data_guard.as_ref() else {
            let status = self.fetch_status.lock().unwrap();
            return match &*status {
                FetchStatus::Fetching => Some("Fetching weather data...".to_string()),
                FetchStatus::LocationNotFound => Some("Location not found. Set a custom city in config.\nExample: location = \"London\" or \"New York\"".to_string()),
                FetchStatus::Error(e) => Some(format!("Error: {}\nClick to retry", e)),
                _ => Some("Weather data not available.\nSet location in config.toml".to_string()),
            };
        };

        let unit = match self.unit {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        };

        Some(format!(
            "{}\n{}\n\nTemperature: {:.0}{}\nFeels like: {:.0}{}\nHumidity: {}%\nWind: {:.0} km/h {}\nHigh: {:.0}{} / Low: {:.0}{}",
            data.location,
            data.description,
            self.convert_temp(data.temperature), unit,
            self.convert_temp(data.feels_like), unit,
            data.humidity,
            data.wind_speed, data.wind_dir,
            self.convert_temp(data.high), unit,
            self.convert_temp(data.low), unit,
        ))
    }

    fn is_visible(&self) -> bool {
        self.enabled
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
