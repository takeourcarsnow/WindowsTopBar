//! Active window module - shows the currently focused application

use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::core::PWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowTextLengthW,
    GetWindowThreadProcessId,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
};
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;

use super::Module;
use crate::utils::truncate_string;

/// Active window module
pub struct ActiveWindowModule {
    cached_text: String,
    window_title: String,
    process_name: String,
    last_update: Instant,
    max_title_length: usize,
}

impl ActiveWindowModule {
    pub fn new() -> Self {
        let mut module = Self {
            cached_text: String::new(),
            window_title: String::new(),
            process_name: String::new(),
            last_update: Instant::now(),
            max_title_length: 50,
        };
        module.force_update();
        module
    }

    /// Set maximum title length before truncation
    pub fn set_max_title_length(&mut self, length: usize) {
        self.max_title_length = length;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        let (title, process) = self.get_active_window_info();
        self.window_title = title;
        self.process_name = process;
        
        // Build display text - show process name like macOS
        self.cached_text = if self.process_name.is_empty() {
            "Desktop".to_string()
        } else {
            // Remove .exe extension and capitalize
            let name = self.process_name
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
    fn get_active_window_info(&self) -> (String, String) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return (String::new(), String::new());
            }

            // Get window title
            let title = self.get_window_title(hwnd);

            // Get process name
            let process = self.get_process_name(hwnd);

            (title, process)
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

    fn update(&mut self) {
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
}
