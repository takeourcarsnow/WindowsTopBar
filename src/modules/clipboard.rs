//! Clipboard manager module

use std::time::Instant;
use super::Module;
use crate::utils::truncate_string;

pub struct ClipboardModule {
    history: Vec<String>,
    max_entries: usize,
    cached_text: String,
    last_update: Instant,
}

impl ClipboardModule {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            max_entries: 10,
            cached_text: String::from("📋"),
            last_update: Instant::now(),
        }
    }

    pub fn get_history(&self) -> Vec<String> {
        self.history.clone()
    }

    /// Try to read the clipboard text and update history when it changes
    fn poll_clipboard(&mut self) {
        if let Some(text) = read_clipboard_text() {
            if text.trim().is_empty() {
                return;
            }

            // Avoid duplicate adjacent entries
            if self.history.first().map(|s| s == &text).unwrap_or(false) {
                return;
            }

            // Remove any existing duplicate elsewhere
            self.history.retain(|h| h != &text);

            // Insert at front and cap size
            self.history.insert(0, text.clone());
            if self.history.len() > self.max_entries {
                self.history.truncate(self.max_entries);
            }

            // Update cached_text (show truncated most recent)
            self.cached_text = format!("📋 {}", truncate_string(&text, 25));
        }
    }

    /// Copy provided text back into clipboard
    pub fn set_clipboard_text(&self, text: &str) -> bool {
        // Use `arboard` crate for cross-platform clipboard access to avoid windows-core
        // version incompatibilities and simplify handling.
        match arboard::Clipboard::new() {
            Ok(mut cb) => cb.set_text(text.to_string()).is_ok(),
            Err(_) => false,
        }
    }
}

impl Default for ClipboardModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ClipboardModule {
    fn id(&self) -> &str {
        "clipboard"
    }

    fn name(&self) -> &str {
        "Clipboard"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        // Show only an icon in the bar; full text is visible in the dropdown
        "📋".to_string()
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Poll clipboard immediately if we have no history (ensure module shows something when enabled),
        // otherwise poll at most once per second
        if self.history.is_empty() || self.last_update.elapsed().as_secs() >= 1 {
            self.poll_clipboard();
            self.last_update = Instant::now();
        }
    }

    fn on_click(&mut self) {
        // Actual dropdown/paste handled by window click handler which can access history
    }

    fn tooltip(&self) -> Option<String> {
        if self.history.is_empty() {
            Some("No clipboard history".to_string())
        } else {
            // Show a short preview of the most recent item plus count
            let preview = truncate_string(&self.history[0], 80);
            Some(format!("{}\n{} entries", preview, self.history.len()))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Read Unicode text from clipboard (best-effort)
fn read_clipboard_text() -> Option<String> {
    // Use `arboard` crate for clipboard access
    match arboard::Clipboard::new() {
        Ok(mut cb) => match cb.get_text() {
            Ok(text) => Some(text),
            Err(_) => None,
        },
        Err(_) => None,
    }
}
