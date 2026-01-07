//! Media controls module - shows now playing and playback controls

use std::time::Instant;

use super::Module;
use crate::utils::truncate_string;

/// Media playback state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Media module
pub struct MediaModule {
    cached_text: String,
    show_now_playing: bool,
    show_controls: bool,
    max_title_length: usize,
    
    // Current media info
    track_title: Option<String>,
    track_artist: Option<String>,
    track_album: Option<String>,
    playback_state: PlaybackState,
    
    last_update: Instant,
}

impl MediaModule {
    pub fn new() -> Self {
        Self {
            cached_text: String::new(),
            show_now_playing: true,
            show_controls: true,
            max_title_length: 30,
            track_title: None,
            track_artist: None,
            track_album: None,
            playback_state: PlaybackState::Stopped,
            last_update: Instant::now(),
        }
    }

    /// Set whether to show now playing info
    pub fn set_show_now_playing(&mut self, show: bool) {
        self.show_now_playing = show;
    }

    /// Set maximum title length
    pub fn set_max_title_length(&mut self, length: usize) {
        self.max_title_length = length;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        // In a full implementation, this would use Windows.Media.Control
        // (SystemMediaTransportControlsSessionManager) to get media info
        // from apps like Spotify, browser media, etc.
        
        // For now, show placeholder when nothing is playing
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        if self.playback_state == PlaybackState::Stopped {
            return String::new();
        }

        let mut text = String::new();

        // Add playback icon
        let icon = match self.playback_state {
            PlaybackState::Playing => "▶",
            PlaybackState::Paused => "⏸",
            PlaybackState::Stopped => "",
        };
        text.push_str(icon);

        // Add track info
        if self.show_now_playing {
            if let Some(ref title) = self.track_title {
                text.push(' ');
                text.push_str(&truncate_string(title, self.max_title_length));
                
                if let Some(ref artist) = self.track_artist {
                    text.push_str(" - ");
                    text.push_str(&truncate_string(artist, 20));
                }
            }
        }

        text
    }

    /// Send play command
    pub fn play(&mut self) {
        // Would send media key
        self.playback_state = PlaybackState::Playing;
        self.cached_text = self.build_display_text();
    }

    /// Send pause command
    pub fn pause(&mut self) {
        self.playback_state = PlaybackState::Paused;
        self.cached_text = self.build_display_text();
    }

    /// Toggle play/pause
    pub fn toggle_playback(&mut self) {
        self.playback_state = match self.playback_state {
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Paused => PlaybackState::Playing,
            PlaybackState::Stopped => PlaybackState::Playing,
        };
        self.cached_text = self.build_display_text();
        
        // Send media key
        self.send_media_key(MediaKey::PlayPause);
    }

    /// Send previous track command
    pub fn previous(&mut self) {
        self.send_media_key(MediaKey::Previous);
    }

    /// Send next track command
    pub fn next(&mut self) {
        self.send_media_key(MediaKey::Next);
    }

    /// Send a media key
    fn send_media_key(&self, key: MediaKey) {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
            VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_NEXT_TRACK,
        };

        let vk = match key {
            MediaKey::PlayPause => VK_MEDIA_PLAY_PAUSE,
            MediaKey::Previous => VK_MEDIA_PREV_TRACK,
            MediaKey::Next => VK_MEDIA_NEXT_TRACK,
        };

        unsafe {
            let mut inputs = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: vk,
                            wScan: 0,
                            dwFlags: KEYEVENTF_EXTENDEDKEY,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: vk,
                            wScan: 0,
                            dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];
            
            SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// Get playback state
    pub fn playback_state(&self) -> PlaybackState {
        self.playback_state
    }

    /// Check if anything is playing
    pub fn is_playing(&self) -> bool {
        self.playback_state == PlaybackState::Playing
    }
}

/// Media key types
enum MediaKey {
    PlayPause,
    Previous,
    Next,
}

impl Default for MediaModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MediaModule {
    fn id(&self) -> &str {
        "media"
    }

    fn name(&self) -> &str {
        "Media Controls"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        self.cached_text.clone()
    }

    fn update(&mut self) {
        // Update every 2 seconds
        if self.last_update.elapsed().as_secs() >= 2 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        self.toggle_playback();
    }

    fn on_scroll(&mut self, delta: i32) {
        if delta > 0 {
            self.next();
        } else {
            self.previous();
        }
    }

    fn tooltip(&self) -> Option<String> {
        if self.playback_state == PlaybackState::Stopped {
            return Some("No media playing".to_string());
        }

        let state = match self.playback_state {
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Stopped => "Stopped",
        };

        let mut tooltip = String::new();
        
        if let Some(ref title) = self.track_title {
            tooltip.push_str(title);
            tooltip.push('\n');
        }
        
        if let Some(ref artist) = self.track_artist {
            tooltip.push_str(artist);
            tooltip.push('\n');
        }
        
        if let Some(ref album) = self.track_album {
            tooltip.push_str(album);
            tooltip.push('\n');
        }
        
        tooltip.push_str(&format!("Status: {}", state));
        
        Some(tooltip)
    }

    fn is_visible(&self) -> bool {
        // Only show when something is playing/paused
        self.playback_state != PlaybackState::Stopped
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
