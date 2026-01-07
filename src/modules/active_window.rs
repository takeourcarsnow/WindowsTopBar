//! Active window module - shows the currently focused application

#![allow(dead_code)]

use std::time::Instant;
use windows::core::PWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::System::Threading::GetCurrentProcessId;

use super::Module;
use crate::utils::truncate_string;

/// Active window module
pub struct ActiveWindowModule {
    cached_text: String,
    window_title: String,
    process_name: String,
    last_update: Instant,
    max_title_length: usize,
    // Remember the last non-TopBar window so we can continue showing it when TopBar is focused
    last_non_topbar_title: String,
    last_non_topbar_process: String,
    // Store current process path and pid for icon lookup
    process_path: String,
    process_pid: u32,
    // Debounce candidate focus changes to avoid showing transient windows like Explorer during Alt-Tab
    candidate_title: String,
    candidate_process: String,
    candidate_process_path: String,
    candidate_pid: u32,
    candidate_since: Option<Instant>,
    focus_debounce_ms: u64,
}

impl ActiveWindowModule {
    pub fn new() -> Self {
        let mut module = Self {
            cached_text: String::new(),
            window_title: String::new(),
            process_name: String::new(),
            last_update: Instant::now(),
            max_title_length: 50,
            last_non_topbar_title: String::new(),
            last_non_topbar_process: String::new(),
            process_path: String::new(),
            process_pid: 0,
            candidate_title: String::new(),
            candidate_process: String::new(),
            candidate_process_path: String::new(),
            candidate_pid: 0,
            candidate_since: None,
            focus_debounce_ms: 200, // ms
        };
        module.force_update();
        module
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        // Get title, process name and process id for the foreground window
        let (title, process, pid) = self.get_active_window_info();

        // Compare to our own process id when possible
        let own_pid = unsafe { GetCurrentProcessId() };

        let lc_proc = process.to_lowercase();
        let lc_title = title.to_lowercase();
        let is_topbar = (pid != 0 && pid == own_pid) || lc_proc.contains("topbar") || lc_title.contains("topbar") || lc_title == "topbar";

        // If Alt is being pressed, ignore Explorer.exe as a transient focus (avoid showing it during Alt-Tab / taskbar switching)
        let is_explorer = lc_proc.contains("explorer") || lc_title.contains("explorer");
        let alt_down = unsafe { (GetAsyncKeyState(0x12) as u16 & 0x8000u16) != 0 };

        let now = Instant::now();

        if is_topbar || (is_explorer && alt_down) {
            // If TopBar or transient Explorer is focused, keep showing the last known non-TopBar window and clear any candidate
            if !self.last_non_topbar_title.is_empty() {
                self.window_title = self.last_non_topbar_title.clone();
                self.process_name = self.last_non_topbar_process.clone();
            }
            self.candidate_since = None;
            self.candidate_title.clear();
            self.candidate_process.clear();
            self.candidate_pid = 0;
        } else {
            // If this matches the currently shown non-TopBar window, apply immediately and clear candidate
            if title == self.last_non_topbar_title && process == self.last_non_topbar_process {
                self.window_title = title.clone();
                self.process_name = process.clone();
                self.candidate_since = None;
            } else {
                // New candidate focus
                if self.last_non_topbar_title.is_empty() {
                    // No previous value (startup) — accept immediately
                    self.last_non_topbar_title = title.clone();
                    self.last_non_topbar_process = process.clone();
                    self.window_title = title.clone();
                    self.process_name = process.clone();
                    self.candidate_since = None;
                } else {
                    // If candidate changed, reset timer
                    if self.candidate_since.is_none() || self.candidate_title != title || self.candidate_process != process || self.candidate_pid != pid {
                        self.candidate_title = title.clone();
                        self.candidate_process = process.clone();
                        // Store the current process path (gathered earlier during get_active_window_info)
                        self.candidate_process_path = self.process_path.clone();
                        self.candidate_pid = pid;
                        self.candidate_since = Some(now);
                    } else if let Some(since) = self.candidate_since {
                        if now.duration_since(since).as_millis() as u64 >= self.focus_debounce_ms {
                            // Commit candidate as stable foreground window
                            self.last_non_topbar_title = self.candidate_title.clone();
                            self.last_non_topbar_process = self.candidate_process.clone();
                            self.window_title = self.last_non_topbar_title.clone();
                            self.process_name = self.last_non_topbar_process.clone();
                            // Commit stored candidate pid/path
                            self.process_pid = self.candidate_pid;
                            self.process_path = self.candidate_process_path.clone();
                            self.candidate_since = None;
                        }
                    }
                }
            }
        }

        // Build display text - show process name like macOS
        self.cached_text = if self.process_name.is_empty() {
            "Desktop".to_string()
        } else {
            // Remove .exe extension and capitalize
            let name = self
                .process_name
                .trim_end_matches(".exe")
                .trim_end_matches(".EXE");

            // Capitalize first letter
            let mut chars: Vec<char> = name.chars().collect();
            if let Some(first) = chars.first_mut() {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }

            chars.into_iter().collect()
        };

        self.last_update = Instant::now();
    }

    /// Get active window information
    fn get_active_window_info(&mut self) -> (String, String, u32) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return (String::new(), String::new(), 0);
            }

            // Get window title
            let title = self.get_window_title(hwnd);

            // Get process id (may be 0 on failure)
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            // Try to get full process path for icon lookup
            let path = self.try_get_process_path(hwnd);
            if !path.is_empty() {
                self.process_path = path.clone();
            }

            // Determine display name from path or fallback to module base name
            let display_name = if !self.process_path.is_empty() {
                if let Some(filename) = std::path::Path::new(&self.process_path).file_name() {
                    filename.to_string_lossy().to_string()
                } else {
                    self.process_path.clone()
                }
            } else {
                // Fallback to existing module name extraction
                self.get_process_name(hwnd)
            };

            // Store current pid
            self.process_pid = process_id;

            (title, display_name, process_id)
        }
    }

    /// Get window title
    fn get_window_title(&self, hwnd: HWND) -> String {
        unsafe {
            let length = GetWindowTextLengthW(hwnd);
            if length == 0 {
                return String::new();
            }

            let mut buffer: Vec<u16> = vec![0; (length + 1) as usize];
            let copied = GetWindowTextW(hwnd, &mut buffer);

            if copied > 0 {
                String::from_utf16_lossy(&buffer[..copied as usize])
            } else {
                String::new()
            }
        }
    }

    /// Try to get full process path from window handle for icon lookup
    fn try_get_process_path(&self, hwnd: HWND) -> String {
        unsafe {
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            if process_id == 0 {
                return String::new();
            }

            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id);
            if let Ok(handle) = handle {
                let mut buffer: Vec<u16> = vec![0; 260];
                let mut size: u32 = buffer.len() as u32;
                let result = QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_FORMAT(0),
                    PWSTR(buffer.as_mut_ptr()),
                    &mut size,
                );

                let _ = windows::Win32::Foundation::CloseHandle(handle);

                if result.is_ok() && size > 0 {
                    return String::from_utf16_lossy(&buffer[..size as usize]);
                }
            }

            String::new()
        }
    }

    /// Get process name from window handle
    fn get_process_name(&self, hwnd: HWND) -> String {
        unsafe {
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            if process_id == 0 {
                return String::new();
            }

            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id);
            if let Ok(handle) = handle {
                // Try QueryFullProcessImageNameW first
                let mut buffer: Vec<u16> = vec![0; 260];
                let mut size: u32 = buffer.len() as u32;
                let result = QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_FORMAT(0),
                    PWSTR(buffer.as_mut_ptr()),
                    &mut size,
                );

                let _ = windows::Win32::Foundation::CloseHandle(handle);

                if result.is_ok() && size > 0 {
                    let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
                    // Extract filename from path
                    if let Some(filename) = std::path::Path::new(&full_path).file_name() {
                        if let Some(name_str) = filename.to_str() {
                            return name_str.to_string();
                        }
                    }
                    return full_path;
                }

                // Fallback to GetModuleBaseNameW
                let mut buffer: Vec<u16> = vec![0; 260];
                let length = GetModuleBaseNameW(handle, None, &mut buffer);

                if length > 0 {
                    return String::from_utf16_lossy(&buffer[..length as usize]);
                }
            }

            String::new()
        }
    }

    /// Get the window title
    pub fn window_title(&self) -> &str {
        &self.window_title
    }

    /// Get the process name
    pub fn process_name(&self) -> &str {
        &self.process_name
    }

    /// Get the process path (may be empty if not available)
    pub fn process_path(&self) -> &str {
        &self.process_path
    }

    /// Get the process id
    pub fn process_id(&self) -> u32 {
        self.process_pid
    }
}

impl Default for ActiveWindowModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ActiveWindowModule {
    fn id(&self) -> &str {
        "active_window"
    }

    fn name(&self) -> &str {
        "Active Window"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        truncate_string(&self.cached_text, self.max_title_length)
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Update every 100ms for very responsive window tracking
        if self.last_update.elapsed().as_millis() >= 100 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Could show window list or app switcher
    }

    fn tooltip(&self) -> Option<String> {
        if self.window_title.is_empty() {
            None
        } else {
            Some(format!("{}\n{}", self.process_name, self.window_title))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
