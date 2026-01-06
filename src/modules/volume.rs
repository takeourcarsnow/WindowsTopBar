//! Volume module for audio control

use std::time::Instant;

use super::Module;

/// Volume module
pub struct VolumeModule {
    show_percentage: bool,
    scroll_to_change: bool,
    scroll_step: u32,
    cached_text: String,
    volume_level: u32,  // 0-100
    is_muted: bool,
    last_update: Instant,
}

impl VolumeModule {
    pub fn new() -> Self {
        let mut module = Self {
            show_percentage: false,
            scroll_to_change: true,
            scroll_step: 5,
            cached_text: String::new(),
            volume_level: 50,
            is_muted: false,
            last_update: Instant::now(),
        };
        module.force_update();
        module
    }

    /// Set whether to show percentage
    pub fn set_show_percentage(&mut self, show: bool) {
        self.show_percentage = show;
    }

    /// Set scroll step
    pub fn set_scroll_step(&mut self, step: u32) {
        self.scroll_step = step;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        // Get volume from Windows
        self.get_system_volume();
        
        // Build display text
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Get system volume using Windows Audio API
    fn get_system_volume(&mut self) {
        // Using COM to access IAudioEndpointVolume
        // This is a simplified version - full implementation would use proper COM
        
        // For now, we'll use a placeholder
        // In a full implementation, you'd use:
        // - CoInitialize
        // - CoCreateInstance(CLSID_MMDeviceEnumerator)
        // - GetDefaultAudioEndpoint
        // - Activate IAudioEndpointVolume
        // - GetMasterVolumeLevelScalar
        
        // Placeholder values
        self.volume_level = 50;
        self.is_muted = false;
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        let icon = self.get_volume_icon();
        
        if self.show_percentage {
            format!("{} {}%", icon, self.volume_level)
        } else {
            icon.to_string()
        }
    }

    /// Get volume icon based on level
    fn get_volume_icon(&self) -> &'static str {
        if self.is_muted {
            "🔇"
        } else if self.volume_level == 0 {
            "🔇"
        } else if self.volume_level < 33 {
            "🔈"
        } else if self.volume_level < 66 {
            "🔉"
        } else {
            "🔊"
        }
    }

    /// Toggle mute
    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
        self.cached_text = self.build_display_text();
    }

    /// Change volume
    pub fn change_volume(&mut self, delta: i32) {
        let new_level = (self.volume_level as i32 + delta).clamp(0, 100) as u32;
        self.volume_level = new_level;
        self.cached_text = self.build_display_text();
    }

    /// Get volume level
    pub fn volume_level(&self) -> u32 {
        self.volume_level
    }

    /// Check if muted
    pub fn is_muted(&self) -> bool {
        self.is_muted
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

    fn display_text(&self, config: &crate::config::Config) -> String {
        let icon = self.get_volume_icon();
        
        if config.modules.volume.show_percentage {
            format!("{} {}%", icon, self.volume_level)
        } else {
            icon.to_string()
        }
    }

    fn update(&mut self) {
        // Update every 5 seconds
        if self.last_update.elapsed().as_secs() >= 5 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Toggle mute
        self.toggle_mute();
    }

    fn on_right_click(&mut self) {
        // Open sound settings
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:sound"])
            .spawn();
    }

    fn on_scroll(&mut self, delta: i32) {
        if self.scroll_to_change {
            let step = if delta > 0 { self.scroll_step as i32 } else { -(self.scroll_step as i32) };
            self.change_volume(step);
        }
    }

    fn tooltip(&self) -> Option<String> {
        if self.is_muted {
            Some(format!("Volume: {}% (Muted)", self.volume_level))
        } else {
            Some(format!("Volume: {}%", self.volume_level))
        }
    }
}
