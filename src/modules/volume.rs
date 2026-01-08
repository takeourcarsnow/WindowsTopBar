//! Volume module for audio control with Windows Core Audio API

use std::time::Instant;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use super::Module;

/// Volume module with real Windows audio integration
pub struct VolumeModule {
    scroll_to_change: bool,
    scroll_step: u32,
    cached_text: String,
    volume_level: u32, // 0-100
    is_muted: bool,
    last_update: Instant,
    com_initialized: bool,
    output_device_name: String,
}

impl VolumeModule {
    pub fn new() -> Self {
        let mut module = Self {
            scroll_to_change: true,
            scroll_step: 5,
            cached_text: String::new(),
            volume_level: 50,
            is_muted: false,
            last_update: Instant::now(),
            com_initialized: false,
            output_device_name: String::new(),
        };
        module.init_com();
        module
    }

    /// Initialize COM for audio APIs
    fn init_com(&mut self) {
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() {
                self.com_initialized = true;
            }
        }
    }

    /// Force an immediate update
    fn force_update(&mut self, config: &crate::config::Config) {
        // Get volume from Windows
        self.get_system_volume();

        // Build display text
        self.cached_text = self.build_display_text(config);
        self.last_update = Instant::now();
    }

    /// Get audio endpoint volume interface
    fn get_audio_endpoint(&self) -> Option<IAudioEndpointVolume> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;

            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;

            let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;

            Some(endpoint_volume)
        }
    }

    /// Get system volume using Windows Core Audio API
    fn get_system_volume(&mut self) {
        if let Some(endpoint) = self.get_audio_endpoint() {
            unsafe {
                // Get volume level (0.0 - 1.0)
                if let Ok(level) = endpoint.GetMasterVolumeLevelScalar() {
                    self.volume_level = (level as f64 * 100.0).round() as u32;
                }

                // Get mute state
                if let Ok(muted) = endpoint.GetMute() {
                    self.is_muted = muted.0 != 0;
                }
            }
        }
    }

    /// Set system volume
    fn set_system_volume(&mut self, level: u32) {
        if let Some(endpoint) = self.get_audio_endpoint() {
            unsafe {
                let scalar = (level as f32 / 100.0).clamp(0.0, 1.0);
                let _ = endpoint.SetMasterVolumeLevelScalar(scalar, std::ptr::null());
                self.volume_level = level;
            }
        }
    }

    /// Set system mute
    fn set_system_mute(&mut self, muted: bool) {
        if let Some(endpoint) = self.get_audio_endpoint() {
            unsafe {
                let _ = endpoint.SetMute(muted, std::ptr::null());
                self.is_muted = muted;
            }
        }
    }

    /// Build the display text
    fn build_display_text(&self, config: &crate::config::Config) -> String {
        let icon = self.get_volume_icon();

        if config.modules.volume.show_percentage {
            format!("{} {}%", icon, self.volume_level)
        } else {
            icon.to_string()
        }
    }

    /// Rebuild the cached display text from current internal state and config
    pub fn rebuild_cached_text(&mut self, config: &crate::config::Config) {
        self.cached_text = self.build_display_text(config);
    }

    /// Get volume icon based on level
    fn get_volume_icon(&self) -> &'static str {
        if self.is_muted || self.volume_level == 0 {
            "🔇"
        } else if self.volume_level < 25 {
            "🔈"
        } else if self.volume_level < 75 {
            "🔉"
        } else {
            "🔊"
        }
    }

    /// Toggle mute (now with real system integration)
    pub fn toggle_mute(&mut self) {
        self.set_system_mute(!self.is_muted);
    }

    /// Change volume (now with real system integration)
    pub fn change_volume(&mut self, delta: i32) {
        let new_level = (self.volume_level as i32 + delta).clamp(0, 100) as u32;
        self.set_system_volume(new_level);
    }

    /// Get volume level
    pub fn volume_level(&self) -> u32 {
        self.volume_level
    }

    /// Check if muted
    pub fn is_muted(&self) -> bool {
        self.is_muted
    }

    /// Get output device name
    pub fn output_device_name(&self) -> &str {
        &self.output_device_name
    }
}

impl Default for VolumeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VolumeModule {
    fn id(&self) -> &str {
        "volume"
    }

    fn name(&self) -> &str {
        "Volume"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        // Return cached text to avoid rebuilding strings unnecessarily
        self.cached_text.clone()
    }

    fn update(&mut self, config: &crate::config::Config) {
        // Use configurable update interval from config (in milliseconds)
        // Check more frequently for responsive volume changes
        let interval_ms = config.modules.volume.update_interval_ms.max(100); // minimum 100ms
        if self.last_update.elapsed().as_millis() >= interval_ms as u128 {
            self.force_update(config);
        }
    }

    fn on_click(&mut self) {
        // Toggle mute with real system integration
        self.toggle_mute();
    }

    fn on_right_click(&mut self) {
        // Open sound settings
        crate::utils::open_url("ms-settings:sound");
    }

    fn on_scroll(&mut self, delta: i32) {
        if self.scroll_to_change {
            let step = if delta > 0 {
                self.scroll_step as i32
            } else {
                -(self.scroll_step as i32)
            };
            self.change_volume(step);
        }
    }

    fn tooltip(&self) -> Option<String> {
        let status = if self.is_muted { " (Muted)" } else { "" };
        Some(format!(
            "Volume: {}%{}\nScroll to adjust, click to mute",
            self.volume_level, status
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
