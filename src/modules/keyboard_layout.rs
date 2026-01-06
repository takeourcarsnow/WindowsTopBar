//! Keyboard layout module - shows current input language/layout

use std::time::Instant;
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use super::Module;

// HKL is a handle type - we'll use isize
type HKL = isize;

/// Common language IDs
const LANG_ENGLISH: u16 = 0x09;
const LANG_SPANISH: u16 = 0x0A;
const LANG_FRENCH: u16 = 0x0C;
const LANG_GERMAN: u16 = 0x07;
const LANG_ITALIAN: u16 = 0x10;
const LANG_PORTUGUESE: u16 = 0x16;
const LANG_RUSSIAN: u16 = 0x19;
const LANG_CHINESE: u16 = 0x04;
const LANG_JAPANESE: u16 = 0x11;
const LANG_KOREAN: u16 = 0x12;
const LANG_ARABIC: u16 = 0x01;
const LANG_HEBREW: u16 = 0x0D;
const LANG_POLISH: u16 = 0x15;
const LANG_DUTCH: u16 = 0x13;
const LANG_TURKISH: u16 = 0x1F;
const LANG_VIETNAMESE: u16 = 0x2A;
const LANG_THAI: u16 = 0x1E;
const LANG_HINDI: u16 = 0x39;
const LANG_UKRAINIAN: u16 = 0x22;
const LANG_CZECH: u16 = 0x05;
const LANG_GREEK: u16 = 0x08;
const LANG_SWEDISH: u16 = 0x1D;
const LANG_NORWEGIAN: u16 = 0x14;
const LANG_DANISH: u16 = 0x06;
const LANG_FINNISH: u16 = 0x0B;

/// Keyboard layout module
pub struct KeyboardLayoutModule {
    cached_text: String,
    current_layout: HKL,
    language_code: String,
    language_name: String,
    last_update: Instant,
}

impl KeyboardLayoutModule {
    pub fn new() -> Self {
        let mut module = Self {
            cached_text: String::new(),
            current_layout: 0,
            language_code: String::new(),
            language_name: String::new(),
            last_update: Instant::now(),
        };
        module.force_update();
        module
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        self.query_keyboard_layout();
        self.cached_text = self.language_code.clone();
        self.last_update = Instant::now();
    }

    /// Query current keyboard layout
    fn query_keyboard_layout(&mut self) {
        unsafe {
            // Get the foreground window to get its keyboard layout
            let hwnd = GetForegroundWindow();
            let thread_id = if hwnd.0.is_null() {
                0
            } else {
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, None)
            };
            
            let layout = GetKeyboardLayout(thread_id);
            self.current_layout = layout.0 as isize;
            
            // Extract language ID (low word of HKL)
            let lang_id = (self.current_layout as usize & 0xFFFF) as u16;
            let primary_lang = lang_id & 0x3FF;
            
            // Map language ID to code and name
            let (code, name) = self.get_language_info(primary_lang);
            self.language_code = code;
            self.language_name = name;
        }
    }

    /// Get language code and name from primary language ID
    fn get_language_info(&self, primary_lang: u16) -> (String, String) {
        match primary_lang {
            LANG_ENGLISH => ("EN".to_string(), "English".to_string()),
            LANG_SPANISH => ("ES".to_string(), "Spanish".to_string()),
            LANG_FRENCH => ("FR".to_string(), "French".to_string()),
            LANG_GERMAN => ("DE".to_string(), "German".to_string()),
            LANG_ITALIAN => ("IT".to_string(), "Italian".to_string()),
            LANG_PORTUGUESE => ("PT".to_string(), "Portuguese".to_string()),
            LANG_RUSSIAN => ("RU".to_string(), "Russian".to_string()),
            LANG_CHINESE => ("ZH".to_string(), "Chinese".to_string()),
            LANG_JAPANESE => ("JA".to_string(), "Japanese".to_string()),
            LANG_KOREAN => ("KO".to_string(), "Korean".to_string()),
            LANG_ARABIC => ("AR".to_string(), "Arabic".to_string()),
            LANG_HEBREW => ("HE".to_string(), "Hebrew".to_string()),
            LANG_POLISH => ("PL".to_string(), "Polish".to_string()),
            LANG_DUTCH => ("NL".to_string(), "Dutch".to_string()),
            LANG_TURKISH => ("TR".to_string(), "Turkish".to_string()),
            LANG_VIETNAMESE => ("VI".to_string(), "Vietnamese".to_string()),
            LANG_THAI => ("TH".to_string(), "Thai".to_string()),
            LANG_HINDI => ("HI".to_string(), "Hindi".to_string()),
            LANG_UKRAINIAN => ("UK".to_string(), "Ukrainian".to_string()),
            LANG_CZECH => ("CS".to_string(), "Czech".to_string()),
            LANG_GREEK => ("EL".to_string(), "Greek".to_string()),
            LANG_SWEDISH => ("SV".to_string(), "Swedish".to_string()),
            LANG_NORWEGIAN => ("NO".to_string(), "Norwegian".to_string()),
            LANG_DANISH => ("DA".to_string(), "Danish".to_string()),
            LANG_FINNISH => ("FI".to_string(), "Finnish".to_string()),
            _ => ("??".to_string(), "Unknown".to_string()),
        }
    }

    /// Get current language code
    pub fn language_code(&self) -> &str {
        &self.language_code
    }

    /// Get current language name
    pub fn language_name(&self) -> &str {
        &self.language_name
    }

    /// Switch to next keyboard layout
    pub fn switch_layout(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{
            PostMessageW, GetForegroundWindow, WM_INPUTLANGCHANGEREQUEST,
        };
        use windows::Win32::Foundation::WPARAM;
        use windows::Win32::Foundation::LPARAM;
        
        unsafe {
            let hwnd = GetForegroundWindow();
            if !hwnd.0.is_null() {
                // Send message to switch to next keyboard layout
                // INPUTLANGCHANGE_FORWARD = 2
                let _ = PostMessageW(hwnd, WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(1));
            }
        }
        
        // Update after a short delay
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.force_update();
    }
}

impl Default for KeyboardLayoutModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for KeyboardLayoutModule {
    fn id(&self) -> &str {
        "keyboard_layout"
    }

    fn name(&self) -> &str {
        "Keyboard Layout"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        format!("⌨ {}", self.language_code)
    }

    fn update(&mut self) {
        // Update every 500ms to catch keyboard layout changes
        if self.last_update.elapsed().as_millis() >= 500 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Switch to next layout on click
        self.switch_layout();
    }

    fn on_right_click(&mut self) {
        // Open language settings
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:keyboard"])
            .spawn();
    }

    fn tooltip(&self) -> Option<String> {
        Some(format!(
            "Keyboard Layout: {}\nClick to switch layout",
            self.language_name
        ))
    }
}
