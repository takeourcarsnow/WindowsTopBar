//! Render module - graphics and drawing
//! 
//! Contains all rendering-related code including the main renderer,
//! and icon handling.

#![allow(dead_code, unused_unsafe)]

mod icons;

pub use icons::Icons;

use anyhow::Result;
use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;
use chrono::Local;

use crate::theme::{Theme};
use crate::utils::Rect;
use crate::modules::{ModuleRegistry, ModuleRenderContext};
use crate::window::get_window_state;

/// Main renderer for the topbar
pub struct Renderer {
    hwnd: HWND,
    dpi: u32,
    pub module_registry: ModuleRegistry,
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
                    let _ = DeleteDC(self.back_buffer);
                }
                if !self.back_bitmap.is_invalid() {
                    let _ = DeleteObject(self.back_bitmap);
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
            let _ = BitBlt(
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
            let _ = DeleteObject(brush);

            // Draw subtle bottom border
            let border_brush = CreateSolidBrush(theme.border.to_colorref());
            let border_rect = windows::Win32::Foundation::RECT {
                left: 0,
                top: rect.height - 1,
                right: rect.width,
                bottom: rect.height,
            };
            FillRect(hdc, &border_rect, border_brush);
            let _ = DeleteObject(border_brush);
        }
    }

    /// Draw all modules
    fn draw_modules(&mut self, hdc: HDC, bar_rect: &Rect, theme: &Theme) {
        // Get enabled modules and config from state (and current drag state)
        let (left_modules, right_modules, config, dragging_module) = get_window_state()
            .map(|s| {
                let state = s.read();
                (
                    state.config.modules.left_modules.clone(),
                    state.config.modules.right_modules.clone(),
                    state.config.clone(),
                    state.dragging_module.clone(),
                )
            })
            .unwrap_or_else(|| {
                let default_config = crate::config::Config::default();
                (
                    vec!["app_menu".to_string(), "active_app".to_string()],
                    vec!["clock".to_string()],
                    std::sync::Arc::new(default_config),
                    None,
                )
            });
        let dragging = dragging_module.clone();

        // First update all modules to get fresh data
        self.module_registry.update_all(&config);
        
        let padding = self.scale(10);  // More breathing room
        let item_spacing = self.scale(6);  // Better spacing between modules
        let item_padding = self.scale(10);  // Larger touch targets

        // Create font - use Variable Text for optimal readability at UI sizes
        let font = self.create_font("Segoe UI Variable Text", self.scale(13), false);
        let bold_font = self.create_font("Segoe UI Variable Display", self.scale(13), true);

        unsafe {
            let old_font = SelectObject(hdc, font);
            SetBkMode(hdc, TRANSPARENT);

            // === LEFT SECTION ===
            let mut x = padding;

            // App menu button (always show)
            if left_modules.contains(&"app_menu".to_string()) && dragging.as_deref() != Some("app_menu") {
                let menu_icon = self.icons.get("menu");
                let menu_rect = self.draw_module_button(
                    hdc, x, bar_rect.height, &menu_icon, item_padding, theme, false
                );
                self.module_bounds.insert("app_menu".to_string(), menu_rect);
                x += menu_rect.width + item_spacing;
            }

            // Active application name
            if left_modules.contains(&"active_app".to_string()) && dragging.as_deref() != Some("active_app") {
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

            // === CENTER SECTION ===
            let mut center_list = config.modules.center_modules.clone();
            // If clock has explicit center flag, ensure it's in the center list
            if config.modules.clock.center && !center_list.iter().any(|m| m == "clock") {
                center_list.push("clock".to_string());
            }

            if !center_list.is_empty() {
                // First compute widths for all center items
                let mut total_width = 0;
                let mut center_widths: Vec<(String, i32)> = Vec::new();
                for id in center_list.iter() {
                    if dragging.as_deref() == Some(id.as_str()) { continue; }
                    let w = match id.as_str() {
                        "clock" => {
                            // Use sample text to get fixed width and prevent layout shifting
                            let sample = self.clock_sample_text(&config);
                            let (tw, _) = self.measure_text(hdc, &sample);
                            tw + item_padding * 2
                        }
                        _ => {
                            // Default measurement for text modules
                            let text = self.module_registry
                                .get(id.as_str())
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_default();
                            let (tw, _) = self.measure_text(hdc, &text);
                            tw + item_padding * 2
                        }
                    };
                    center_widths.push((id.clone(), w));
                    total_width += w + item_spacing;
                }

                if total_width > 0 {
                    total_width = total_width.saturating_sub(item_spacing); // remove trailing spacing
                    let mut cx = (bar_rect.width - total_width) / 2;
                    for (id, w) in center_widths.iter() {
                        // Draw each center item
                        if id == "clock" {
                            let clock_text = self.module_registry
                                .get("clock")
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                            let rect = self.draw_module_text_fixed(hdc, cx, bar_rect.height, &clock_text, item_padding, *w, theme);
                            self.module_bounds.insert("clock".to_string(), rect);
                        } else {
                            let text = self.module_registry
                                .get(id.as_str())
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_default();
                            let rect = self.draw_module_text(hdc, cx, bar_rect.height, &text, item_padding, theme, false);
                            self.module_bounds.insert(id.clone(), rect);
                        }
                        cx += w + item_spacing;
                    }
                }
            }

            // === RIGHT SECTION (draw right-to-left based on config order) ===
            x = bar_rect.width - padding;

            for id in right_modules.iter().rev() {
                if dragging.as_deref() == Some(id.as_str()) { continue; }

                match id.as_str() {
                    "clock" => {
                        let clock_text = self.module_registry
                            .get("clock")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                        // Use sample text to get fixed width and prevent layout shifting
                        let sample = self.clock_sample_text(&config);
                        let (sample_width, _) = self.measure_text(hdc, &sample);
                        let min_width = sample_width + item_padding * 2;
                        x -= min_width;
                        let clock_rect = self.draw_module_text_fixed(
                            hdc, x, bar_rect.height, &clock_text, item_padding, min_width, theme
                        );
                        self.module_bounds.insert("clock".to_string(), clock_rect);
                        x -= item_spacing;
                    }

                    "battery" => {
                        let battery_text = self.module_registry
                            .get("battery")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| {
                                let icon = self.icons.get("battery");
                                format!("{} --", icon)
                            });
                        if !battery_text.is_empty() {
                            let min_width = self.scale(70);
                            x -= min_width;
                            let battery_rect = self.draw_module_text_fixed(
                                hdc, x, bar_rect.height, &battery_text, item_padding, min_width, theme
                            );
                            self.module_bounds.insert("battery".to_string(), battery_rect);
                            x -= item_spacing;
                        }
                    }

                    "volume" => {
                        let volume_text = self.module_registry
                            .get("volume")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| self.icons.get("volume_high"));
                        let min_width = self.scale(68);
                        x -= min_width;
                        let volume_rect = self.draw_module_text_fixed(
                            hdc, x, bar_rect.height, &volume_text, item_padding, min_width, theme
                        );
                        self.module_bounds.insert("volume".to_string(), volume_rect);
                        x -= item_spacing;
                    }

                    "network" => {
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

                    "system_info" => {
                        let show_graph = config.modules.system_info.show_graph;
                        if show_graph {
                            let graph_width = self.scale(60);
                            let graph_height = bar_rect.height - self.scale(8);
                            x -= graph_width + item_padding * 2;

                            let rect = Rect::new(x, (bar_rect.height - graph_height) / 2, graph_width + item_padding * 2, graph_height);
                            unsafe {
                                let bg_brush = CreateSolidBrush(theme.background_secondary.to_colorref());
                                let r = windows::Win32::Foundation::RECT { left: rect.x, top: rect.y, right: rect.x + rect.width, bottom: rect.y + rect.height };
                                FillRect(hdc, &r, bg_brush);
                                let _ = DeleteObject(bg_brush);

                                if let Some(module) = self.module_registry.get("system_info") {
                                    if let Some(values) = module.graph_values() {
                                        if !values.is_empty() {
                                            let len = values.len() as i32;
                                            let inner_w = rect.width - item_padding * 2;
                                            let inner_h = rect.height - 4;
                                            let step = inner_w as f32 / (len.max(1) as f32);
                                            let mut x_pos = rect.x + item_padding;

                                            let mut points: Vec<(i32,i32)> = Vec::new();
                                            for v in values.iter() {
                                                let clamped = v.clamp(0.0, 100.0) / 100.0;
                                                let y = rect.y + 2 + ((1.0 - clamped) * inner_h as f32) as i32;
                                                points.push((x_pos, y));
                                                x_pos += step as i32;
                                            }

                                            let pen = CreatePen(PS_SOLID, 2, theme.cpu_normal.to_colorref());
                                            let old_pen = SelectObject(hdc, pen);
                                            if let Some((sx, sy)) = points.first() {
                                                let _ = MoveToEx(hdc, *sx, *sy, Some(std::ptr::null_mut()));
                                                for (px, py) in points.iter().skip(1) {
                                                    let _ = LineTo(hdc, *px, *py);
                                                }
                                            }
                                            let _ = SelectObject(hdc, old_pen);
                                            let _ = DeleteObject(pen);
                                        }
                                    }
                                }
                            }

                            self.module_bounds.insert("system_info".to_string(), rect);
                            x -= item_spacing;
                        } else {
                            let sysinfo_text = self.module_registry
                                .get("system_info")
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_else(|| "CPU --  RAM --".to_string());

                            // Compute a sensible minimum width based on which parts are configured
                            // to be shown (CPU and/or Memory). This avoids leaving a large empty
                            // area when memory is hidden while still preventing layout jitter.
                            let sample_text = match (
                                config.modules.system_info.show_cpu,
                                config.modules.system_info.show_memory,
                            ) {
                                (true, true) => "CPU 100%  RAM 100%",
                                (true, false) => "CPU 100%",
                                (false, true) => "RAM 100%",
                                _ => "CPU --  RAM --",
                            };
                            let (sample_w, _) = self.measure_text(hdc, sample_text);
                            // Add horizontal padding and enforce a small minimum so the area isn't too tight
                            let mut min_width = sample_w + item_padding * 2;
                            min_width = min_width.max(self.scale(60));

                            x -= min_width;
                            let sysinfo_rect = self.draw_module_text_fixed(
                                hdc, x, bar_rect.height, &sysinfo_text, item_padding, min_width, theme
                            );
                            self.module_bounds.insert("system_info".to_string(), sysinfo_rect);
                            x -= item_spacing;
                        }
                    }

                    "media" => {
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
                            x -= item_spacing;
                        }
                    }

                    "clipboard" => {
                        // Render clipboard module (shows latest entry or icon)
                        let clipboard_text = self.module_registry
                            .get("clipboard")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "📋".to_string());
                        let (text_width, _) = self.measure_text(hdc, &clipboard_text);
                        x -= text_width + item_padding * 2;
                        let clip_rect = self.draw_module_text(
                            hdc, x, bar_rect.height, &clipboard_text, item_padding, theme, false
                        );
                        self.module_bounds.insert("clipboard".to_string(), clip_rect);
                        x -= item_spacing;
                    }

                    "gpu" => {
                        let show_graph = config.modules.gpu.show_graph;
                        if show_graph {
                            let graph_width = self.scale(60);
                            let graph_height = bar_rect.height - self.scale(8);
                            x -= graph_width + item_padding * 2;

                            let rect = Rect::new(x, (bar_rect.height - graph_height) / 2, graph_width + item_padding * 2, graph_height);
                            unsafe {
                                let bg_brush = CreateSolidBrush(theme.background_secondary.to_colorref());
                                let r = windows::Win32::Foundation::RECT { left: rect.x, top: rect.y, right: rect.x + rect.width, bottom: rect.y + rect.height };
                                FillRect(hdc, &r, bg_brush);
                                let _ = DeleteObject(bg_brush);

                                if let Some(module) = self.module_registry.get("gpu") {
                                    if let Some(values) = module.graph_values() {
                                        if !values.is_empty() {
                                            let len = values.len() as i32;
                                            let inner_w = rect.width - item_padding * 2;
                                            let inner_h = rect.height - 4;
                                            let step = inner_w as f32 / (len.max(1) as f32);
                                            let mut x_pos = rect.x + item_padding;

                                            // Normalize values to 0..1 based on 0..100
                                            let mut points: Vec<(i32, i32)> = Vec::new();
                                            for v in values.iter() {
                                                let clamped = v.clamp(0.0, 100.0) / 100.0;
                                                let y = rect.y + 2 + ((1.0 - clamped) * inner_h as f32) as i32;
                                                points.push((x_pos, y));
                                                x_pos += step as i32;
                                            }

                                            let pen = CreatePen(PS_SOLID, 2, theme.accent.to_colorref());
                                            let old_pen = SelectObject(hdc, pen);
                                            // Move to first
                                            if let Some((sx, sy)) = points.first() {
                                                let _ = MoveToEx(hdc, *sx, *sy, Some(std::ptr::null_mut()));
                                                for (px, py) in points.iter().skip(1) {
                                                    let _ = LineTo(hdc, *px, *py);
                                                }
                                            }
                                            let _ = SelectObject(hdc, old_pen);
                                            let _ = DeleteObject(pen);
                                        }
                                    }
                                }
                            }

                            self.module_bounds.insert("gpu".to_string(), rect);
                            x -= item_spacing;
                        } else {
                            let gpu_text = self.module_registry
                                .get("gpu")
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_else(|| self.icons.get("gpu"));
                            // Fixed width for "GPU 100%" format
                            let min_width = self.scale(75);
                            x -= min_width;
                            let gpu_rect = self.draw_module_text_fixed(
                                hdc, x, bar_rect.height, &gpu_text, item_padding, min_width, theme
                            );
                            self.module_bounds.insert("gpu".to_string(), gpu_rect);
                            x -= item_spacing;
                        }
                    }

                    "keyboard_layout" => {
                        let keyboard_text = self.module_registry
                            .get("keyboard_layout")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "EN".to_string());
                        let (text_width, _) = self.measure_text(hdc, &keyboard_text);
                        x -= text_width + item_padding * 2;
                        let keyboard_rect = self.draw_module_text(
                            hdc, x, bar_rect.height, &keyboard_text, item_padding, theme, false
                        );
                        self.module_bounds.insert("keyboard_layout".to_string(), keyboard_rect);
                        x -= item_spacing;
                    }

                    "uptime" => {
                        let uptime_text = self.module_registry
                            .get("uptime")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "0d 0h".to_string());
                        let min_width = self.scale(72);
                        x -= min_width;
                        let uptime_rect = self.draw_module_text_fixed(
                            hdc, x, bar_rect.height, &uptime_text, item_padding, min_width, theme
                        );
                        self.module_bounds.insert("uptime".to_string(), uptime_rect);
                        x -= item_spacing;
                    }

                    "bluetooth" => {
                        let bluetooth_text = self.module_registry
                            .get("bluetooth")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| self.icons.get("bluetooth"));
                        let (text_width, _) = self.measure_text(hdc, &bluetooth_text);
                        x -= text_width + item_padding * 2;
                        let bluetooth_rect = self.draw_module_text(
                            hdc, x, bar_rect.height, &bluetooth_text, item_padding, theme, false
                        );
                        self.module_bounds.insert("bluetooth".to_string(), bluetooth_rect);
                        x -= item_spacing;
                    }

                    "disk" => {
                        let disk_text = self.module_registry
                            .get("disk")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| self.icons.get("disk"));
                        let (text_width, _) = self.measure_text(hdc, &disk_text);
                        x -= text_width + item_padding * 2;
                        let disk_rect = self.draw_module_text(
                            hdc, x, bar_rect.height, &disk_text, item_padding, theme, false
                        );
                        self.module_bounds.insert("disk".to_string(), disk_rect);
                        x -= item_spacing;
                    }

                    "weather" => {
                        let weather_text = self.module_registry
                            .get("weather")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "🌡️ ...".to_string());
                        if !weather_text.is_empty() {
                            let (text_width, _) = self.measure_text(hdc, &weather_text);
                            x -= text_width + item_padding * 2;
                            let weather_rect = self.draw_module_text(
                                hdc, x, bar_rect.height, &weather_text, item_padding, theme, false
                            );
                            self.module_bounds.insert("weather".to_string(), weather_rect);
                            x -= item_spacing;
                        }
                    }

                    _ => {}
                }
            }

                // If a drag is active, draw the dragged item as an overlay and a drop marker
                if let Some(state) = get_window_state() {
                    let s = state.read();
                    if let Some(drag_id) = &s.dragging_module {
                        // Determine display text for dragged module
                        let display = self.module_registry
                            .get(drag_id)
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| drag_id.clone());

                        let (text_w, text_h) = self.measure_text(hdc, &display);
                        let width = text_w + item_padding * 2;
                        let height = text_h + item_padding + 2;
                        let y = (bar_rect.height - height) / 2;
                        let x_pos = s.drag_current_x - width / 2;

                        unsafe {
                            // Draw background
                            let bg_brush = CreateSolidBrush(theme.background_secondary.to_colorref());
                            let r = windows::Win32::Foundation::RECT { left: x_pos, top: y, right: x_pos + width, bottom: y + height };
                            FillRect(hdc, &r, bg_brush);
                            let _ = DeleteObject(bg_brush);

                            // Draw text
                            SetTextColor(hdc, theme.text_primary.to_colorref());
                            self.draw_text(hdc, x_pos + item_padding, (bar_rect.height - text_h) / 2, &display);

                            // Draw insertion marker
                            let pen = CreatePen(PS_SOLID, 2, theme.accent.to_colorref());
                            let old_pen = SelectObject(hdc, pen);
                            let top = self.scale(6);
                            let bottom = bar_rect.height - self.scale(6);
                            let _ = MoveToEx(hdc, s.drag_current_x, top, None);
                            let _ = LineTo(hdc, s.drag_current_x, bottom);
                            let _ = SelectObject(hdc, old_pen);
                            let _ = DeleteObject(pen);
                        }
                    }
                }
            }
    }

    /// Draw a module button with modern hover effect
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
        let height = text_height + padding + 4;  // Slightly taller for better tap targets
        let y = (bar_height - height) / 2;

        unsafe {
            // Draw subtle rounded background on hover
            if is_hovered {
                let brush = CreateSolidBrush(theme.background_hover.to_colorref());
                let rect = windows::Win32::Foundation::RECT {
                    left: x + 2,  // Slight inset for visual softness
                    top: y + 1,
                    right: x + width - 2,
                    bottom: y + height - 1,
                };
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(brush);
            }

            // Draw text with proper color
            SetTextColor(hdc, theme.text_primary.to_colorref());
            let text_y = (bar_height - text_height) / 2;
            self.draw_text(hdc, x + padding, text_y, text);
        }

        Rect::new(x, y, width, height)
    }

    /// Draw module text with improved layout
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
        let height = text_height + padding + 2;  // Balanced height
        let y = (bar_height - height) / 2;

        unsafe {
            // Use primary text color for good contrast
            SetTextColor(hdc, theme.text_primary.to_colorref());
            // Center text vertically with slight adjustment for visual balance
            let text_y = (bar_height - text_height) / 2;
            self.draw_text(hdc, x + padding, text_y, text);
        }

        Rect::new(x, y, width, height)
    }

    /// Compute a sample clock string representing the widest possible time
    /// for the current configuration, used to calculate fixed width and prevent layout shifting.
    fn clock_sample_text(&self, config: &crate::config::Config) -> String {
        let mut result = String::new();

        if config.modules.clock.show_day {
            // "Wed" is typically widest day abbreviation
            result.push_str("Wed ");
        }

        if config.modules.clock.show_date {
            // Use "Sep 00" as sample – September is often widest month abbreviation
            result.push_str("Sep 00  ");
        }

        // Time portion: use widest digits (0 is often widest)
        if config.modules.clock.format_24h {
            if config.modules.clock.show_seconds {
                result.push_str("00:00:00");
            } else {
                result.push_str("00:00");
            }
        } else {
            if config.modules.clock.show_seconds {
                result.push_str("00:00:00 PM");
            } else {
                result.push_str("00:00 PM");
            }
        }

        result
    }

    /// Draw module text with a minimum width to prevent layout shifting
    fn draw_module_text_fixed(
        &self,
        hdc: HDC,
        x: i32,
        bar_height: i32,
        text: &str,
        padding: i32,
        min_width: i32,
        theme: &Theme,
    ) -> Rect {
        let (text_width, text_height) = self.measure_text(hdc, text);
        let width = (text_width + padding * 2).max(min_width);
        let height = text_height + padding + 2;
        let y = (bar_height - height) / 2;

        unsafe {
            SetTextColor(hdc, theme.text_primary.to_colorref());
            let text_y = (bar_height - text_height) / 2;
            // Center text within the fixed width
            let text_x = x + (width - text_width) / 2;
            self.draw_text(hdc, text_x, text_y, text);
        }

        Rect::new(x, y, width, height)
    }

    /// Measure text dimensions
    fn measure_text(&self, hdc: HDC, text: &str) -> (i32, i32) {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let mut size = windows::Win32::Foundation::SIZE::default();
            let _ = GetTextExtentPoint32W(hdc, &wide[..wide.len()-1], &mut size);
            (size.cx, size.cy)
        }
    }

    /// Draw text at position
    fn draw_text(&self, hdc: HDC, x: i32, y: i32, text: &str) {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = TextOutW(hdc, x, y, &wide[..wide.len()-1]);
        }
    }

    /// Create a font with optimized rendering for Windows 11
    fn create_font(&self, family: &str, size: i32, bold: bool) -> HFONT {
        unsafe {
            let family_wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
            let mut lf = LOGFONTW::default();
            lf.lfHeight = -size;
            // Use medium weight for regular text, semibold for emphasis
            lf.lfWeight = if bold { FW_SEMIBOLD.0 as i32 } else { FW_MEDIUM.0 as i32 };
            lf.lfCharSet = DEFAULT_CHARSET;
            lf.lfOutPrecision = OUT_TT_PRECIS;  // TrueType preferred
            lf.lfClipPrecision = CLIP_DEFAULT_PRECIS;
            lf.lfQuality = CLEARTYPE_QUALITY;  // Best text rendering
            lf.lfPitchAndFamily = VARIABLE_PITCH.0 | FF_SWISS.0;
            // Enable antialiasing for smooth edges
            
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
                let _ = DeleteDC(self.back_buffer);
            }
            if !self.back_bitmap.is_invalid() {
                let _ = DeleteObject(self.back_bitmap);
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
