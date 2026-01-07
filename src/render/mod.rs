//! Render module - graphics and drawing
//!
//! Contains all rendering-related code including the main renderer,
//! and icon handling.

#![allow(dead_code, unused_unsafe)]

mod icons;

pub use icons::Icons;

use anyhow::Result;
use chrono::Local;
use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;

use crate::modules::{ModuleRegistry, ModuleRenderContext};
use crate::theme::Theme;
use crate::utils::Rect;
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
                0,
                0,
                bar_rect.width,
                bar_rect.height,
                self.back_buffer,
                0,
                0,
                SRCCOPY,
            );
        }
    }

    /// Draw the background
    fn draw_background(&self, hdc: HDC, rect: &Rect, theme: &Theme) {
        unsafe {
            let brush = CreateSolidBrush(theme.background.colorref());
            let win_rect = windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: rect.width,
                bottom: rect.height,
            };
            FillRect(hdc, &win_rect, brush);
            let _ = DeleteObject(brush);

            // Draw subtle bottom border
            let border_brush = CreateSolidBrush(theme.border.colorref());
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

    #[allow(clippy::explicit_auto_deref)]
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

        let padding = self.scale(8); // Edge padding
        let item_spacing = self.scale(4); // Minimal spacing between items
        let item_padding = self.scale(8); // Internal item padding

        // Create font - use optimized modern fonts for macOS-like aesthetics
        // Segoe UI Variable offers better clarity, while Inter is a great fallback
        let font = self.create_font("Segoe UI Variable Text", self.scale(13), false);
        let bold_font = self.create_font("Segoe UI Variable Display", self.scale(13), true);

        unsafe {
            let _old_font = SelectObject(hdc, font);
            SetBkMode(hdc, TRANSPARENT);

            // === LEFT SECTION ===
            let mut x = padding;

            // App menu button (always show)
            if left_modules.contains(&"app_menu".to_string())
                && dragging.as_deref() != Some("app_menu")
            {
                let menu_icon = self.icons.get("menu");
                let menu_rect = self.draw_module_button(
                    hdc,
                    x,
                    bar_rect.height,
                    &menu_icon,
                    item_padding,
                    theme,
                    false,
                );
                self.module_bounds.insert("app_menu".to_string(), menu_rect);
                x += menu_rect.width + item_spacing;
            }

            // Active application name
            if left_modules.contains(&"active_app".to_string())
                && dragging.as_deref() != Some("active_app")
            {
                SelectObject(hdc, bold_font);
                let app_name = self
                    .module_registry
                    .get("active_window")
                    .map(|m| m.display_text(config.as_ref()))
                    .unwrap_or_else(|| "TopBar".to_string());
                let app_rect = self.draw_module_text(
                    hdc,
                    x,
                    bar_rect.height,
                    &app_name,
                    item_padding,
                    theme,
                    true,
                );
                SelectObject(hdc, font);
                self.module_bounds
                    .insert("active_app".to_string(), app_rect);
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
                    if dragging.as_deref() == Some(id.as_str()) {
                        continue;
                    }
                    let w = match id.as_str() {
                        "clock" => {
                            // Use sample text to get fixed width and prevent layout shifting
                            let sample = self.clock_sample_text(&config);
                            let (tw, _) = self.measure_text(hdc, &sample);
                            tw + item_padding * 2
                        }
                        _ => {
                            // Default measurement for text modules
                            let text = self
                                .module_registry
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
                            let clock_text = self
                                .module_registry
                                .get("clock")
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                            let rect = self.draw_module_text_fixed(
                                hdc,
                                cx,
                                bar_rect.height,
                                &clock_text,
                                item_padding,
                                *w,
                                theme,
                            );
                            self.module_bounds.insert("clock".to_string(), rect);
                        } else {
                            let text = self
                                .module_registry
                                .get(id.as_str())
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_default();
                            let rect = self.draw_module_text(
                                hdc,
                                cx,
                                bar_rect.height,
                                &text,
                                item_padding,
                                theme,
                                false,
                            );
                            self.module_bounds.insert(id.clone(), rect);
                        }
                        cx += w + item_spacing;
                    }
                }
            }

            // === RIGHT SECTION (draw right-to-left based on config order) ===
            x = bar_rect.width - padding;

            for id in right_modules.iter().rev() {
                if dragging.as_deref() == Some(id.as_str()) {
                    continue;
                }

                match id.as_str() {
                    "clock" => {
                        let clock_text = self
                            .module_registry
                            .get("clock")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                        // Use sample text to get fixed width and prevent layout shifting
                        let sample = self.clock_sample_text(&config);
                        let (sample_width, _) = self.measure_text(hdc, &sample);
                        let min_width = sample_width + item_padding * 2;
                        x -= min_width;
                        let clock_rect = self.draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &clock_text,
                            item_padding,
                            min_width,
                            theme,
                        );
                        self.module_bounds.insert("clock".to_string(), clock_rect);
                        x -= item_spacing;
                    }

                    "battery" => {
                        let battery_text = self
                            .module_registry
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
                                hdc,
                                x,
                                bar_rect.height,
                                &battery_text,
                                item_padding,
                                min_width,
                                theme,
                            );
                            self.module_bounds
                                .insert("battery".to_string(), battery_rect);
                            x -= item_spacing;
                        }
                    }

                    "volume" => {
                        let volume_text = self
                            .module_registry
                            .get("volume")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| self.icons.get("volume_high"));
                        let min_width = self.scale(68);
                        x -= min_width;
                        let volume_rect = self.draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &volume_text,
                            item_padding,
                            min_width,
                            theme,
                        );
                        self.module_bounds.insert("volume".to_string(), volume_rect);
                        x -= item_spacing;
                    }

                    "network" => {
                        let network_text = self
                            .module_registry
                            .get("network")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| self.icons.get("wifi"));

                        // Reserve a fixed minimum width for the numeric speed portion to prevent layout shifting
                        let sample = format!("{} 000.0↓/000.0↑MB/s", self.icons.get("ethernet"));
                        let (sample_width, _) = self.measure_text(hdc, &sample);
                        let min_width = sample_width + item_padding * 2;

                        x -= min_width;
                        let network_rect = self.draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &network_text,
                            item_padding,
                            min_width,
                            theme,
                        );
                        self.module_bounds
                            .insert("network".to_string(), network_rect);
                        x -= item_spacing;
                    }

                    "system_info" => {
                        let show_graph = config.modules.system_info.show_graph;
                        if show_graph {
                            let graph_width = self.scale(60);
                            let graph_height = bar_rect.height - self.scale(8);
                            x -= graph_width + item_padding * 2;

                            let rect = Rect::new(
                                x,
                                (bar_rect.height - graph_height) / 2,
                                graph_width + item_padding * 2,
                                graph_height,
                            );
                            unsafe {
                                let bg_brush =
                                    CreateSolidBrush(theme.background_secondary.colorref());
                                let r = windows::Win32::Foundation::RECT {
                                    left: rect.x,
                                    top: rect.y,
                                    right: rect.x + rect.width,
                                    bottom: rect.y + rect.height,
                                };
                                FillRect(hdc, &r, bg_brush);
                                let _ = DeleteObject(bg_brush);

                                if let Some(module) = self.module_registry.get("system_info") {
                                    if let Some(values) = module.graph_values() {
                                        if !values.is_empty() {
                                            let inner_w = rect.width - item_padding * 2;
                                            let inner_h = rect.height - 4;
                                            let max_bars = inner_w.max(1) as usize;

                                            // Downsample or use full values depending on available pixels
                                            let bars: Vec<f32> = if values.len() <= max_bars {
                                                values
                                            } else {
                                                let mut out = Vec::with_capacity(max_bars);
                                                let chunk = values.len() / max_bars;
                                                let mut idx = 0usize;
                                                for _ in 0..max_bars {
                                                    let end = (idx + chunk).min(values.len());
                                                    let slice = &values[idx..end];
                                                    if !slice.is_empty() {
                                                        let avg = slice.iter().copied().sum::<f32>() / slice.len() as f32;
                                                        out.push(avg);
                                                    } else {
                                                        out.push(0.0);
                                                    }
                                                    idx = end;
                                                }
                                                // If any remaining samples, fold them into the last bar
                                                if idx < values.len() {
                                                    let mut rem_sum = 0.0f32;
                                                    let mut rem_count = 0usize;
                                                    for i in idx..values.len() {
                                                        rem_sum += values[i];
                                                        rem_count += 1;
                                                    }
                                                    if rem_count > 0 && !out.is_empty() {
                                                        let last = out.last_mut().unwrap();
                                                        *last = (*last + rem_sum / rem_count as f32) / 2.0;
                                                    }
                                                }
                                                out
                                            };

                                            let mut bar_width = inner_w / bars.len().max(1) as i32;
                                            if bar_width < 1 {
                                                bar_width = 1;
                                            }

                                            let mut x_pos = rect.x + item_padding;
                                            for v in bars.iter() {
                                                let clamped = v.clamp(0.0, 100.0) / 100.0;
                                                let bar_height = (clamped * inner_h as f32) as i32;
                                                let bar_y = rect.y + 2 + (inner_h - bar_height);

                                                unsafe {
                                                    let color = theme.cpu_normal.colorref();
                                                    let bar_brush = CreateSolidBrush(color);
                                                    let bar_rect = windows::Win32::Foundation::RECT {
                                                        left: x_pos,
                                                        top: bar_y,
                                                        right: x_pos + bar_width - 1,
                                                        bottom: bar_y + bar_height,
                                                    };
                                                    FillRect(hdc, &bar_rect, bar_brush);
                                                    let _ = DeleteObject(bar_brush);
                                                }

                                                x_pos += bar_width;
                                            }
                                        }
                                    }
                                }
                            }

                            self.module_bounds.insert("system_info".to_string(), rect);
                            x -= item_spacing;
                        } else {
                            let sysinfo_text = self
                                .module_registry
                                .get("system_info")
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_else(|| "CPU --  RAM --".to_string());

                            // Compute a sensible minimum width based on which parts are configured
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
                            let mut min_width = sample_w + item_padding * 2;
                            min_width = min_width.max(self.scale(64));

                            x -= min_width;

                            // Draw the percentage-only text (CPU / RAM)
                            let sysinfo_rect = self.draw_module_text_fixed(
                                hdc,
                                x,
                                bar_rect.height,
                                &sysinfo_text,
                                item_padding,
                                min_width,
                                theme,
                            );

                            self.module_bounds
                                .insert("system_info".to_string(), sysinfo_rect);
                            x -= item_spacing;
                        }
                    }

                    "media" => {
                        let media_text = self
                            .module_registry
                            .get("media")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_default();
                        if !media_text.is_empty() {
                            let (text_width, _) = self.measure_text(hdc, &media_text);
                            x -= text_width + item_padding * 2;
                            let media_rect = self.draw_module_text(
                                hdc,
                                x,
                                bar_rect.height,
                                &media_text,
                                item_padding,
                                theme,
                                false,
                            );
                            self.module_bounds.insert("media".to_string(), media_rect);
                            x -= item_spacing;
                        }
                    }

                    "clipboard" => {
                        // Render clipboard module (shows latest entry or icon)
                        let clipboard_text = self
                            .module_registry
                            .get("clipboard")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "📋".to_string());
                        let (text_width, _) = self.measure_text(hdc, &clipboard_text);
                        x -= text_width + item_padding * 2;
                        let clip_rect = self.draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &clipboard_text,
                            item_padding,
                            theme,
                            false,
                        );
                        self.module_bounds
                            .insert("clipboard".to_string(), clip_rect);
                        x -= item_spacing;
                    }

                    "gpu" => {
                        let show_graph = config.modules.gpu.show_graph;
                        if show_graph {
                            let graph_width = self.scale(60);
                            let graph_height = bar_rect.height - self.scale(8);
                            x -= graph_width + item_padding * 2;

                            let rect = Rect::new(
                                x,
                                (bar_rect.height - graph_height) / 2,
                                graph_width + item_padding * 2,
                                graph_height,
                            );
                            unsafe {
                                let bg_brush =
                                    CreateSolidBrush(theme.background_secondary.colorref());
                                let r = windows::Win32::Foundation::RECT {
                                    left: rect.x,
                                    top: rect.y,
                                    right: rect.x + rect.width,
                                    bottom: rect.y + rect.height,
                                };
                                FillRect(hdc, &r, bg_brush);
                                let _ = DeleteObject(bg_brush);

                                if let Some(module) = self.module_registry.get("gpu") {
                                    if let Some(values) = module.graph_values() {
                                        if !values.is_empty() {
                                            let inner_w = rect.width - item_padding * 2;
                                            let inner_h = rect.height - 4;
                                            let max_bars = inner_w.max(1) as usize;

                                            let bars: Vec<f32> = if values.len() <= max_bars {
                                                values
                                            } else {
                                                let mut out = Vec::with_capacity(max_bars);
                                                let chunk = values.len() / max_bars;
                                                let mut idx = 0usize;
                                                for _ in 0..max_bars {
                                                    let end = (idx + chunk).min(values.len());
                                                    let slice = &values[idx..end];
                                                    if !slice.is_empty() {
                                                        let avg = slice.iter().copied().sum::<f32>() / slice.len() as f32;
                                                        out.push(avg);
                                                    } else {
                                                        out.push(0.0);
                                                    }
                                                    idx = end;
                                                }
                                                if idx < values.len() {
                                                    let mut rem_sum = 0.0f32;
                                                    let mut rem_count = 0usize;
                                                    for i in idx..values.len() {
                                                        rem_sum += values[i];
                                                        rem_count += 1;
                                                    }
                                                    if rem_count > 0 && !out.is_empty() {
                                                        let last = out.last_mut().unwrap();
                                                        *last = (*last + rem_sum / rem_count as f32) / 2.0;
                                                    }
                                                }
                                                out
                                            };

                                            let mut bar_width = inner_w / bars.len().max(1) as i32;
                                            if bar_width < 1 {
                                                bar_width = 1;
                                            }

                                            let mut x_pos = rect.x + item_padding;
                                            for v in bars.iter() {
                                                let clamped = v.clamp(0.0, 100.0) / 100.0;
                                                let bar_height = (clamped * inner_h as f32) as i32;
                                                let bar_y = rect.y + 2 + (inner_h - bar_height);

                                                unsafe {
                                                    let bar_brush = CreateSolidBrush(theme.accent.colorref());
                                                    let bar_rect = windows::Win32::Foundation::RECT {
                                                        left: x_pos,
                                                        top: bar_y,
                                                        right: x_pos + bar_width - 1,
                                                        bottom: bar_y + bar_height,
                                                    };
                                                    FillRect(hdc, &bar_rect, bar_brush);
                                                    let _ = DeleteObject(bar_brush);
                                                }

                                                x_pos += bar_width;
                                            }
                                        }
                                    }
                                }
                            }

                            self.module_bounds.insert("gpu".to_string(), rect);
                            x -= item_spacing;
                        } else {
                            let gpu_text = self
                                .module_registry
                                .get("gpu")
                                .map(|m| m.display_text(&*config))
                                .unwrap_or_else(|| self.icons.get("gpu"));
                            // Fixed width for "GPU 100%" format
                            let min_width = self.scale(92);
                            x -= min_width;

                            // Simple text-only rendering for GPU (percentage text)
                            let gpu_rect = self.draw_module_text_fixed(
                                hdc,
                                x,
                                bar_rect.height,
                                &gpu_text,
                                item_padding,
                                min_width,
                                theme,
                            );
                            self.module_bounds.insert("gpu".to_string(), gpu_rect);
                            x -= item_spacing;
                        }
                    }

                    "keyboard_layout" => {
                        let keyboard_text = self
                            .module_registry
                            .get("keyboard_layout")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "EN".to_string());
                        let (text_width, _) = self.measure_text(hdc, &keyboard_text);
                        x -= text_width + item_padding * 2;
                        let keyboard_rect = self.draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &keyboard_text,
                            item_padding,
                            theme,
                            false,
                        );
                        self.module_bounds
                            .insert("keyboard_layout".to_string(), keyboard_rect);
                        x -= item_spacing;
                    }

                    "uptime" => {
                        let uptime_text = self
                            .module_registry
                            .get("uptime")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "0d 0h".to_string());
                        let min_width = self.scale(72);
                        x -= min_width;
                        let uptime_rect = self.draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &uptime_text,
                            item_padding,
                            min_width,
                            theme,
                        );
                        self.module_bounds.insert("uptime".to_string(), uptime_rect);
                        x -= item_spacing;
                    }

                    "bluetooth" => {
                        let bluetooth_text = self
                            .module_registry
                            .get("bluetooth")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| self.icons.get("bluetooth"));
                        let (text_width, _) = self.measure_text(hdc, &bluetooth_text);
                        x -= text_width + item_padding * 2;
                        let bluetooth_rect = self.draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &bluetooth_text,
                            item_padding,
                            theme,
                            false,
                        );
                        self.module_bounds
                            .insert("bluetooth".to_string(), bluetooth_rect);
                        x -= item_spacing;
                    }

                    "disk" => {
                        let disk_width = self.scale(24);
                        let disk_height = bar_rect.height - self.scale(8);
                        x -= disk_width + item_padding * 2;

                        let rect = Rect::new(
                            x,
                            (bar_rect.height - disk_height) / 2,
                            disk_width + item_padding * 2,
                            disk_height,
                        );
                        unsafe {
                            let bg_brush = CreateSolidBrush(theme.background_secondary.colorref());
                            let r = windows::Win32::Foundation::RECT {
                                left: rect.x,
                                top: rect.y,
                                right: rect.x + rect.width,
                                bottom: rect.y + rect.height,
                            };
                            FillRect(hdc, &r, bg_brush);
                            let _ = DeleteObject(bg_brush);

                            if let Some(module) = self.module_registry.get("disk") {
                                if let Some(disk_module) = module.as_any().downcast_ref::<crate::modules::disk::DiskModule>() {
                                    let usage_percent = disk_module.primary_usage_percent() as f32 / 100.0;
                                    
                                    // Draw pie chart
                                    let center_x = rect.x + rect.width / 2;
                                    let center_y = rect.y + rect.height / 2;
                                    let radius = (rect.width.min(rect.height) / 2 - 2) as f32;
                                    
                                    // Draw used portion (filled arc)
                                    if usage_percent > 0.0 {
                                        let used_angle = (usage_percent * 360.0f32).to_radians();
                                        let start_angle = -90.0f32.to_radians(); // Start from top
                                        let end_angle = start_angle + used_angle;
                                        
                                        let steps = ((used_angle / std::f32::consts::PI * 180.0) as i32).max(8);
                                        let angle_step = used_angle / steps as f32;
                                        
                                        let mut points = Vec::new();
                                        points.push((center_x, center_y)); // Center point
                                        
                                        for i in 0..=steps {
                                            let angle = start_angle + angle_step * i as f32;
                                            let px = center_x + (angle.cos() * radius) as i32;
                                            let py = center_y + (angle.sin() * radius) as i32;
                                            points.push((px, py));
                                        }
                                        
                                        if points.len() >= 3 {
                                            let pie_brush = CreateSolidBrush(theme.accent.colorref());
                                            let pie_pen = CreatePen(PS_SOLID, 1, theme.accent.colorref());
                                            let old_pen = SelectObject(hdc, pie_pen);
                                            let old_brush = SelectObject(hdc, pie_brush);
                                            
let vertices: Vec<windows::Win32::Foundation::POINT> = points.iter()
                                            .map(|(x, y)| windows::Win32::Foundation::POINT { x: *x, y: *y })
                                                .collect();
                                            
                                            let _ = Polygon(hdc, &vertices);
                                            let _ = SelectObject(hdc, old_brush);
                                            let _ = SelectObject(hdc, old_pen);
                                            let _ = DeleteObject(pie_brush);
                                            let _ = DeleteObject(pie_pen);
                                        }
                                    }
                                    
                                    // Draw remaining portion (outline)
                                    let remaining_angle = ((1.0 - usage_percent) * 360.0f32).to_radians();
                                    if remaining_angle > 0.0 {
                                        let start_angle = (-90.0f32 + usage_percent * 360.0f32).to_radians();
                                        let end_angle = start_angle + remaining_angle;
                                        
                                        let steps = ((remaining_angle / std::f32::consts::PI * 180.0) as i32).max(8);
                                        let angle_step = remaining_angle / steps as f32;
                                        
                                        let outline_pen = CreatePen(PS_SOLID, 2, theme.text_secondary.colorref());
                                        let old_pen = SelectObject(hdc, outline_pen);
                                        
                                        for i in 0..steps {
                                            let angle1 = start_angle + angle_step * i as f32;
                                            let angle2 = start_angle + angle_step * (i + 1) as f32;
                                            
                                            let x1 = center_x + (angle1.cos() * radius) as i32;
                                            let y1 = center_y + (angle1.sin() * radius) as i32;
                                            let x2 = center_x + (angle2.cos() * radius) as i32;
                                            let y2 = center_y + (angle2.sin() * radius) as i32;
                                            
                                            let _ = MoveToEx(hdc, x1, y1, None);
                                            let _ = LineTo(hdc, x2, y2);
                                        }
                                        
                                        let _ = SelectObject(hdc, old_pen);
                                        let _ = DeleteObject(outline_pen);
                                    }
                                }
                            }
                        }

                        self.module_bounds.insert("disk".to_string(), rect);
                        x -= item_spacing;
                    }

                    "weather" => {
                        let weather_text = self
                            .module_registry
                            .get("weather")
                            .map(|m| m.display_text(&*config))
                            .unwrap_or_else(|| "🌡️ ...".to_string());
                        if !weather_text.is_empty() {
                            let (text_width, _) = self.measure_text(hdc, &weather_text);
                            x -= text_width + item_padding * 2;
                            let weather_rect = self.draw_module_text(
                                hdc,
                                x,
                                bar_rect.height,
                                &weather_text,
                                item_padding,
                                theme,
                                false,
                            );
                            self.module_bounds
                                .insert("weather".to_string(), weather_rect);
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
                    let display = self
                        .module_registry
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
                        let bg_brush = CreateSolidBrush(theme.background_secondary.colorref());
                        let r = windows::Win32::Foundation::RECT {
                            left: x_pos,
                            top: y,
                            right: x_pos + width,
                            bottom: y + height,
                        };
                        FillRect(hdc, &r, bg_brush);
                        let _ = DeleteObject(bg_brush);

                        // Draw text
                        SetTextColor(hdc, theme.text_primary.colorref());
                        self.draw_text(
                            hdc,
                            x_pos + item_padding,
                            (bar_rect.height - text_h) / 2,
                            &display,
                        );

                        // Draw insertion marker
                        let pen = CreatePen(PS_SOLID, 2, theme.accent.colorref());
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

    #[allow(clippy::too_many_arguments)]
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
        let height = text_height + padding + 4; // Slightly taller for better tap targets
        let y = (bar_height - height) / 2;

        unsafe {
            // Draw subtle rounded background on hover
            if is_hovered {
                let brush = CreateSolidBrush(theme.background_hover.colorref());
                let rect = windows::Win32::Foundation::RECT {
                    left: x + 2, // Slight inset for visual softness
                    top: y + 1,
                    right: x + width - 2,
                    bottom: y + height - 1,
                };
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(brush);
            }

            // Draw text with proper color
            SetTextColor(hdc, theme.text_primary.colorref());
            let text_y = (bar_height - text_height) / 2;
            self.draw_text(hdc, x + padding, text_y, text);
        }

        Rect::new(x, y, width, height)
    }

    #[allow(clippy::too_many_arguments)]
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
        let height = text_height + padding + 2; // Balanced height
        let y = (bar_height - height) / 2;

        unsafe {
            // Use primary text color for good contrast
            SetTextColor(hdc, theme.text_primary.colorref());
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
        } else if config.modules.clock.show_seconds {
            result.push_str("00:00:00 PM");
        } else {
            result.push_str("00:00 PM");
        }

        result
    }

    #[allow(clippy::too_many_arguments)]
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
            SetTextColor(hdc, theme.text_primary.colorref());
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
            let _ = GetTextExtentPoint32W(hdc, &wide[..wide.len() - 1], &mut size);
            (size.cx, size.cy)
        }
    }

    /// Draw text at position
    fn draw_text(&self, hdc: HDC, x: i32, y: i32, text: &str) {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = TextOutW(hdc, x, y, &wide[..wide.len() - 1]);
        }
    }

    /// Create a font with optimized rendering for modern UI (macOS-inspired)
    fn create_font(&self, family: &str, size: i32, bold: bool) -> HFONT {
        unsafe {
            let family_wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
            let mut lf = LOGFONTW {
                lfHeight: -size,
                lfWeight: if bold { 600 } else { 400 },
                lfCharSet: DEFAULT_CHARSET,
                lfOutPrecision: OUT_TT_PRECIS,
                lfClipPrecision: CLIP_DEFAULT_PRECIS,
                lfQuality: CLEARTYPE_QUALITY,
                lfPitchAndFamily: VARIABLE_PITCH.0 | FF_SWISS.0,
                ..Default::default()
            };

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
