//! Utility functions and helpers for TopBar

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;

/// Convert a Rust string to a wide string for Windows API
pub fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Create a PCWSTR from a wide string slice
pub fn to_pcwstr(s: &[u16]) -> PCWSTR {
    PCWSTR::from_raw(s.as_ptr())
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration in seconds to human-readable string
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    
    if hours > 0 {
        format!("{}:{:02}", hours, minutes)
    } else {
        format!("{} min", minutes)
    }
}

/// Format percentage with proper rounding
pub fn format_percentage(value: f32) -> String {
    if value >= 100.0 {
        "100%".to_string()
    } else if value >= 10.0 {
        format!("{:.0}%", value)
    } else {
        format!("{:.1}%", value)
    }
}

/// Calculate DPI scaling factor
pub fn get_dpi_scale(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}

/// Scale a value by DPI
pub fn scale_by_dpi(value: i32, dpi: u32) -> i32 {
    ((value as f32) * get_dpi_scale(dpi)) as i32
}

/// Rectangle structure for layout calculations
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub fn center_x(&self) -> i32 {
        self.x + self.width / 2
    }

    pub fn center_y(&self) -> i32 {
        self.y + self.height / 2
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    pub fn shrink(&self, amount: i32) -> Rect {
        Rect {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - amount * 2).max(0),
            height: (self.height - amount * 2).max(0),
        }
    }

    pub fn expand(&self, amount: i32) -> Rect {
        Rect {
            x: self.x - amount,
            y: self.y - amount,
            width: self.width + amount * 2,
            height: self.height + amount * 2,
        }
    }
}

/// Point structure
#[derive(Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Size structure
#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

/// Animation easing functions
pub mod easing {
    /// Linear interpolation
    pub fn linear(t: f32) -> f32 {
        t
    }

    /// Ease in quad
    pub fn ease_in_quad(t: f32) -> f32 {
        t * t
    }

    /// Ease out quad
    pub fn ease_out_quad(t: f32) -> f32 {
        t * (2.0 - t)
    }

    /// Ease in out quad
    pub fn ease_in_out_quad(t: f32) -> f32 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            -1.0 + (4.0 - 2.0 * t) * t
        }
    }

    /// Ease out cubic
    pub fn ease_out_cubic(t: f32) -> f32 {
        let t = t - 1.0;
        t * t * t + 1.0
    }

    /// Ease in out cubic
    pub fn ease_in_out_cubic(t: f32) -> f32 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            let t = 2.0 * t - 2.0;
            0.5 * t * t * t + 1.0
        }
    }

    /// Ease out back (slight overshoot)
    pub fn ease_out_back(t: f32) -> f32 {
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        let t = t - 1.0;
        1.0 + c3 * t * t * t + c1 * t * t
    }
}

/// Simple animator for smooth transitions
pub struct Animator {
    start_value: f32,
    end_value: f32,
    current_value: f32,
    duration_ms: u32,
    elapsed_ms: u32,
    is_running: bool,
    easing: fn(f32) -> f32,
}

impl Animator {
    pub fn new(initial_value: f32) -> Self {
        Self {
            start_value: initial_value,
            end_value: initial_value,
            current_value: initial_value,
            duration_ms: 150,
            elapsed_ms: 0,
            is_running: false,
            easing: easing::ease_out_cubic,
        }
    }

    pub fn animate_to(&mut self, target: f32, duration_ms: u32) {
        self.start_value = self.current_value;
        self.end_value = target;
        self.duration_ms = duration_ms;
        self.elapsed_ms = 0;
        self.is_running = true;
    }

    pub fn set_easing(&mut self, easing: fn(f32) -> f32) {
        self.easing = easing;
    }

    pub fn update(&mut self, delta_ms: u32) -> bool {
        if !self.is_running {
            return false;
        }

        self.elapsed_ms += delta_ms;
        
        if self.elapsed_ms >= self.duration_ms {
            self.current_value = self.end_value;
            self.is_running = false;
            return true;
        }

        let t = self.elapsed_ms as f32 / self.duration_ms as f32;
        let eased_t = (self.easing)(t);
        self.current_value = self.start_value + (self.end_value - self.start_value) * eased_t;
        true
    }

    pub fn value(&self) -> f32 {
        self.current_value
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn set_immediate(&mut self, value: f32) {
        self.current_value = value;
        self.end_value = value;
        self.is_running = false;
    }
}

/// Truncate string to fit width with ellipsis
pub fn truncate_string(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    }
}

/// Get the primary monitor work area
pub fn get_primary_work_area() -> Option<Rect> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    use windows::Win32::Foundation::RECT;

    unsafe {
        let mut rect = RECT::default();
        let result = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        if result.is_ok() {
            Some(Rect {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            })
        } else {
            None
        }
    }
}

/// Get screen dimensions
pub fn get_screen_size() -> Size {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    unsafe {
        Size {
            width: GetSystemMetrics(SM_CXSCREEN),
            height: GetSystemMetrics(SM_CYSCREEN),
        }
    }
}

/// Check if running with administrator privileges
pub fn is_elevated() -> bool {
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size,
            &mut size,
        );

        let _ = windows::Win32::Foundation::CloseHandle(token);

        if result.is_ok() {
            elevation.TokenIsElevated != 0
        } else {
            false
        }
    }
}
