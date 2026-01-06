//! Render module - graphics and drawing
//! 
//! Contains all rendering-related code including the main renderer,
//! dropdown menus, and icon handling.

mod dropdown;
mod icons;

pub use dropdown::{DropdownMenu, DropdownItem};
pub use icons::Icons;

use anyhow::Result;
use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;
use chrono::Local;

use crate::theme::{Color, Theme};
use crate::utils::Rect;
use crate::modules::{ModuleRegistry, ModuleRenderContext};
use crate::window::get_window_state;

/// Main renderer for the topbar
pub struct Renderer {
    hwnd: HWND,
    dpi: u32,
    module_registry: ModuleRegistry,
    module_bounds: HashMap<String, Rect>,
    icons: Icons,
    // Double buffering
    back_buffer: HDC,
    back_bitmap: HBITMAP,
    buffer_size: (i32, i32),
}

impl Renderer {
    /// Create a new renderer
    pub fn new(hwnd: HWND, dpi: u32) -> Result<Self> {
        let module_registry = ModuleRegistry::new();
        let icons = Icons::new();

        Ok(Self {
            hwnd,
            dpi,
            module_registry,
            module_bounds: HashMap::new(),
            icons,
            back_buffer: HDC::default(),
            back_bitmap: HBITMAP::default(),
            buffer_size: (0, 0),
        })
    }

    /// Update DPI
    pub fn set_dpi(&mut self, dpi: u32) {
        self.dpi = dpi;
    }

    /// Ensure back buffer is correct size
    fn ensure_back_buffer(&mut self, hdc: HDC, width: i32, height: i32) {
        if self.buffer_size.0 != width || self.buffer_size.1 != height {
            unsafe {
                // Clean up old buffer
                if !self.back_buffer.is_invalid() {
                    DeleteDC(self.back_buffer);
                }
                if !self.back_bitmap.is_invalid() {
                    DeleteObject(self.back_bitmap);
                }

                // Create new buffer
                self.back_buffer = CreateCompatibleDC(hdc);
                self.back_bitmap = CreateCompatibleBitmap(hdc, width, height);
                SelectObject(self.back_buffer, self.back_bitmap);
                self.buffer_size = (width, height);
            }
        }
    }

    /// Main paint function
    pub fn paint(&mut self, hdc: HDC, bar_rect: &Rect, theme: &Theme) {
        self.ensure_back_buffer(hdc, bar_rect.width, bar_rect.height);
        
        // Clear module bounds
        self.module_bounds.clear();

        // Draw to back buffer
        self.draw_background(self.back_buffer, bar_rect, theme);
        self.draw_modules(self.back_buffer, bar_rect, theme);
        
        // Copy to screen
        unsafe {
            BitBlt(
                hdc,
                0, 0,
                bar_rect.width, bar_rect.height,
                self.back_buffer,
                0, 0,
                SRCCOPY,
            );
        }
    }

    /// Draw the background
    fn draw_background(&self, hdc: HDC, rect: &Rect, theme: &Theme) {
        unsafe {
            let brush = CreateSolidBrush(theme.background.to_colorref());
            let win_rect = windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: rect.width,
                bottom: rect.height,
            };
            FillRect(hdc, &win_rect, brush);
            DeleteObject(brush);

            // Draw subtle bottom border
            let border_brush = CreateSolidBrush(theme.border.to_colorref());
            let border_rect = windows::Win32::Foundation::RECT {
                left: 0,
                top: rect.height - 1,
                right: rect.width,
                bottom: rect.height,
            };
            FillRect(hdc, &border_rect, border_brush);
            DeleteObject(border_brush);
        }
    }

    /// Draw all modules
    fn draw_modules(&mut self, hdc: HDC, bar_rect: &Rect, theme: &Theme) {
        // First update all modules to get fresh data
        self.module_registry.update_all();
        
        // Get enabled modules and config from state
        let (left_modules, right_modules, config) = get_window_state()
            .map(|s| {
                let state = s.read();
                (
                    state.config.modules.left_modules.clone(),
                    state.config.modules.right_modules.clone(),
                    state.config.clone(),
                )
            })
            .unwrap_or_else(|| {
                let default_config = crate::config::Config::default();
                (
                    vec!["app_menu".to_string(), "active_app".to_string()],
                    vec!["clock".to_string()],
                    std::sync::Arc::new(default_config),
                )
            });
        
        let padding = self.scale(8);
        let item_spacing = self.scale(4);
        let item_padding = self.scale(8);

        // Create font
        let font = self.create_font("Segoe UI Variable", self.scale(13), false);
        let bold_font = self.create_font("Segoe UI Variable", self.scale(13), true);

        unsafe {
            let old_font = SelectObject(hdc, font);
            SetBkMode(hdc, TRANSPARENT);

            // === LEFT SECTION ===
            let mut x = padding;

            // App menu button (always show)
            if left_modules.contains(&"app_menu".to_string()) {
                let menu_icon = self.icons.get("menu");
                let menu_rect = self.draw_module_button(
                    hdc, x, bar_rect.height, &menu_icon, item_padding, theme, false
                );
                self.module_bounds.insert("app_menu".to_string(), menu_rect);
                x += menu_rect.width + item_spacing;
            }

            // Active application name
            if left_modules.contains(&"active_app".to_string()) {
                SelectObject(hdc, bold_font);
                let app_name = self.module_registry
                    .get("active_window")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_else(|| "TopBar".to_string());
                let app_rect = self.draw_module_text(
                    hdc, x, bar_rect.height, &app_name, item_padding, theme, true
                );
                SelectObject(hdc, font);
                self.module_bounds.insert("active_app".to_string(), app_rect);
            }

            // === RIGHT SECTION (draw right-to-left) ===
            x = bar_rect.width - padding;

            // Clock (rightmost if enabled)
            if right_modules.contains(&"clock".to_string()) {
                let clock_text = self.module_registry
                    .get("clock")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                let (text_width, _) = self.measure_text(hdc, &clock_text);
                x -= text_width + item_padding * 2;
                let clock_rect = self.draw_module_text(
                    hdc, x, bar_rect.height, &clock_text, item_padding, theme, false
                );
                self.module_bounds.insert("clock".to_string(), clock_rect);
                x -= item_spacing;
            }

            // Battery
            if right_modules.contains(&"battery".to_string()) {
                let battery_text = self.module_registry
                    .get("battery")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_else(|| {
                        let icon = self.icons.get("battery");
                        format!("{} --", icon)
                    });
                if !battery_text.is_empty() {
                    let (text_width, _) = self.measure_text(hdc, &battery_text);
                    x -= text_width + item_padding * 2;
                    let battery_rect = self.draw_module_text(
                        hdc, x, bar_rect.height, &battery_text, item_padding, theme, false
                    );
                    self.module_bounds.insert("battery".to_string(), battery_rect);
                    x -= item_spacing;
                }
            }

            // Volume
            if right_modules.contains(&"volume".to_string()) {
                let volume_text = self.module_registry
                    .get("volume")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_else(|| self.icons.get("volume_high"));
                let (text_width, _) = self.measure_text(hdc, &volume_text);
                x -= text_width + item_padding * 2;
                let volume_rect = self.draw_module_text(
                    hdc, x, bar_rect.height, &volume_text, item_padding, theme, false
                );
                self.module_bounds.insert("volume".to_string(), volume_rect);
                x -= item_spacing;
            }

            // Network
            if right_modules.contains(&"network".to_string()) {
                let network_text = self.module_registry
                    .get("network")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_else(|| self.icons.get("wifi"));
                let (text_width, _) = self.measure_text(hdc, &network_text);
                x -= text_width + item_padding * 2;
                let network_rect = self.draw_module_text(
                    hdc, x, bar_rect.height, &network_text, item_padding, theme, false
                );
                self.module_bounds.insert("network".to_string(), network_rect);
                x -= item_spacing;
            }

            // System info (CPU/Memory)
            if right_modules.contains(&"system_info".to_string()) {
                let sysinfo_text = self.module_registry
                    .get("system_info")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_else(|| "CPU --  MEM --".to_string());
                let (text_width, _) = self.measure_text(hdc, &sysinfo_text);
                x -= text_width + item_padding * 2;
                let sysinfo_rect = self.draw_module_text(
                    hdc, x, bar_rect.height, &sysinfo_text, item_padding, theme, false
                );
                self.module_bounds.insert("system_info".to_string(), sysinfo_rect);
                x -= item_spacing;
            }

            // Media controls
            if right_modules.contains(&"media".to_string()) {
                let media_text = self.module_registry
                    .get("media")
                    .map(|m| m.display_text(&*config))
                    .unwrap_or_default();
                if !media_text.is_empty() {
                    let (text_width, _) = self.measure_text(hdc, &media_text);
                    x -= text_width + item_padding * 2;
                    let media_rect = self.draw_module_text(
                        hdc, x, bar_rect.height, &media_text, item_padding, theme, false
                    );
                    self.module_bounds.insert("media".to_string(), media_rect);
                }
            }

            SelectObject(hdc, old_font);
            DeleteObject(font);
            DeleteObject(bold_font);
        }
    }

    /// Draw a module button with hover effect
    fn draw_module_button(
        &self,
        hdc: HDC,
        x: i32,
        bar_height: i32,
        text: &str,
        padding: i32,
        theme: &Theme,
        is_hovered: bool,
    ) -> Rect {
        let (text_width, text_height) = self.measure_text(hdc, text);
        let width = text_width + padding * 2;
        let height = text_height + padding;
        let y = (bar_height - height) / 2;

        unsafe {
            // Draw background on hover
            if is_hovered {
                let brush = CreateSolidBrush(theme.background_hover.to_colorref());
                let rect = windows::Win32::Foundation::RECT {
                    left: x,
                    top: y,
                    right: x + width,
                    bottom: y + height,
                };
                FillRect(hdc, &rect, brush);
                DeleteObject(brush);
            }

            // Draw text
            SetTextColor(hdc, theme.text_primary.to_colorref());
            let text_y = (bar_height - text_height) / 2;
            self.draw_text(hdc, x + padding, text_y, text);
        }

        Rect::new(x, y, width, height)
    }

    /// Draw module text
    fn draw_module_text(
        &self,
        hdc: HDC,
        x: i32,
        bar_height: i32,
        text: &str,
        padding: i32,
        theme: &Theme,
        _bold: bool,
    ) -> Rect {
        let (text_width, text_height) = self.measure_text(hdc, text);
        let width = text_width + padding * 2;
        let height = text_height + padding;
        let y = (bar_height - height) / 2;

        unsafe {
            SetTextColor(hdc, theme.text_primary.to_colorref());
            let text_y = (bar_height - text_height) / 2;
            self.draw_text(hdc, x + padding, text_y, text);
        }

        Rect::new(x, y, width, height)
    }

    /// Measure text dimensions
    fn measure_text(&self, hdc: HDC, text: &str) -> (i32, i32) {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let mut size = windows::Win32::Foundation::SIZE::default();
            GetTextExtentPoint32W(hdc, &wide[..wide.len()-1], &mut size);
            (size.cx, size.cy)
        }
    }

    /// Draw text at position
    fn draw_text(&self, hdc: HDC, x: i32, y: i32, text: &str) {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            TextOutW(hdc, x, y, &wide[..wide.len()-1]);
        }
    }

    /// Create a font
    fn create_font(&self, family: &str, size: i32, bold: bool) -> HFONT {
        unsafe {
            let family_wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
            let mut lf = LOGFONTW::default();
            lf.lfHeight = -size;
            lf.lfWeight = if bold { FW_SEMIBOLD.0 as i32 } else { FW_NORMAL.0 as i32 };
            lf.lfCharSet = DEFAULT_CHARSET;
            lf.lfOutPrecision = OUT_TT_PRECIS;
            lf.lfClipPrecision = CLIP_DEFAULT_PRECIS;
            lf.lfQuality = CLEARTYPE_QUALITY;
            lf.lfPitchAndFamily = VARIABLE_PITCH.0 | FF_SWISS.0;
            
            let face_len = family_wide.len().min(32);
            lf.lfFaceName[..face_len].copy_from_slice(&family_wide[..face_len]);
            
            CreateFontIndirectW(&lf)
        }
    }

    /// Scale a value by DPI
    fn scale(&self, value: i32) -> i32 {
        (value as f32 * self.dpi as f32 / 96.0) as i32
    }

    /// Hit test to find which module was clicked
    pub fn hit_test(&self, x: i32, y: i32) -> Option<String> {
        for (id, rect) in &self.module_bounds {
            if rect.contains(x, y) {
                return Some(id.clone());
            }
        }
        None
    }

    /// Get module bounds
    pub fn module_bounds(&self) -> &HashMap<String, Rect> {
        &self.module_bounds
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            if !self.back_buffer.is_invalid() {
                DeleteDC(self.back_buffer);
            }
            if !self.back_bitmap.is_invalid() {
                DeleteObject(self.back_bitmap);
            }
        }
    }
}

/// Render context passed to modules
impl ModuleRenderContext {
    pub fn new(hdc: HDC, theme: &Theme, dpi: u32) -> Self {
        Self {
            hdc,
            theme: theme.clone(),
            dpi,
        }
    }
}
