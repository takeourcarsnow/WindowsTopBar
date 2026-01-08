use chrono::Local;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{DrawIconEx, DI_NORMAL, HICON};

use crate::theme::Theme;
use crate::utils::Rect;
use crate::window::state::get_window_state;
use super::drawing::{create_font, measure_text, draw_text, scale, draw_line_graph, downsample_values};

/// Draw all modules
pub fn draw_modules(
    renderer: &mut super::renderer::Renderer,
    hdc: HDC,
    bar_rect: &Rect,
    theme: &Theme,
) {
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
    renderer.module_registry.update_all(&config);

    let padding = scale(8, renderer.dpi); // Edge padding
    let item_spacing = scale(4, renderer.dpi); // Minimal spacing between items
    let item_padding = scale(8, renderer.dpi); // Internal item padding

    // Create font - use optimized modern fonts for macOS-like aesthetics
    // Segoe UI Variable offers better clarity, while Inter is a great fallback
    let font = create_font("Segoe UI Variable Text", scale(13, renderer.dpi), false);
    let bold_font = create_font("Segoe UI Variable Display", scale(13, renderer.dpi), true);

    unsafe {
        let _old_font = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT);

        // === LEFT SECTION ===
        let mut x = padding;

        // App menu button (always show)
        if left_modules.contains(&"app_menu".to_string())
            && dragging.as_deref() != Some("app_menu")
        {
            let menu_icon = renderer.icons.get("menu");
            let menu_rect = draw_module_button(
                hdc,
                x,
                bar_rect.height,
                &menu_icon,
                item_padding,
                theme,
                false,
                renderer.dpi,
            );
            renderer.module_bounds.insert("app_menu".to_string(), menu_rect);
            x += menu_rect.width + item_spacing;
        }

        // Quick search button (visible when enabled in config)
        if config.search.enabled && dragging.as_deref() != Some("search") {
            let search_icon = renderer.icons.get("search");
            let search_rect = draw_module_button(
                hdc,
                x,
                bar_rect.height,
                &search_icon,
                item_padding,
                theme,
                false,
                renderer.dpi,
            );
            renderer.module_bounds.insert("search".to_string(), search_rect);
            x += search_rect.width + item_spacing;
        }

        // Active application name
        if left_modules.contains(&"active_app".to_string())
            && dragging.as_deref() != Some("active_app")
        {
            SelectObject(hdc, bold_font);
            let app_name = renderer
                .module_registry
                .get("active_window")
                .map(|m| m.display_text(config.as_ref()))
                .unwrap_or_else(|| "TopBar".to_string());
            // Try load a small app icon for the active application
            let mut app_icon: Option<HICON> = None;
            // Avoid holding an immutable borrow across calls that need &mut self
            let mut path_opt: Option<String> = None;
            {
                if let Some(m) = renderer.module_registry.get("active_window") {
                    if let Some(awm) = m.as_any().downcast_ref::<crate::modules::active_window::ActiveWindowModule>() {
                        let p = awm.process_path().to_string();
                        if !p.is_empty() {
                            path_opt = Some(p);
                        }
                    }
                }
            }

            if let Some(path) = path_opt {
                app_icon = get_small_icon_for_path(renderer, &path);
            }

            let app_rect = draw_module_text(
                hdc,
                x,
                bar_rect.height,
                &app_name,
                item_padding,
                theme,
                true,
                app_icon,
                renderer.dpi,
            );

            SelectObject(hdc, font);
            renderer.module_bounds
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
                        let sample = clock_sample_text(&config, renderer.dpi);
                        let (tw, _) = measure_text(hdc, &sample);
                        tw + item_padding * 2
                    }
                    _ => {
                        // Default measurement for text modules
                        let text = renderer
                            .module_registry
                            .get(id.as_str())
                            .map(|m| m.display_text(config.as_ref()))
                            .unwrap_or_default();
                        let (tw, _) = measure_text(hdc, &text);
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
                        let clock_text = renderer
                            .module_registry
                            .get("clock")
                            .map(|m| m.display_text(config.as_ref()))
                            .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                        let rect = draw_module_text_fixed(
                            hdc,
                            cx,
                            bar_rect.height,
                            &clock_text,
                            item_padding,
                            *w,
                            theme,
                            renderer.dpi,
                        );
                        renderer.module_bounds.insert("clock".to_string(), rect);
                    } else {
                        let text = renderer
                            .module_registry
                            .get(id.as_str())
                            .map(|m| m.display_text(config.as_ref()))
                            .unwrap_or_default();
                        let rect = draw_module_text(
                            hdc,
                            cx,
                            bar_rect.height,
                            &text,
                            item_padding,
                            theme,
                            false,
                            None,
                            renderer.dpi,
                        );
                        renderer.module_bounds.insert(id.clone(), rect);
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
                    let clock_text = renderer
                        .module_registry
                        .get("clock")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| Local::now().format("%I:%M %p").to_string());
                    // Use sample text to get fixed width and prevent layout shifting
                    let sample = clock_sample_text(&config, renderer.dpi);
                    let (sample_width, _) = measure_text(hdc, &sample);
                    let min_width = sample_width + item_padding * 2;
                    x -= min_width;
                    let clock_rect = draw_module_text_fixed(
                        hdc,
                        x,
                        bar_rect.height,
                        &clock_text,
                        item_padding,
                        min_width,
                        theme,
                        renderer.dpi,
                    );
                    renderer.module_bounds.insert("clock".to_string(), clock_rect);
                    x -= item_spacing;
                }

                "battery" => {
                    let battery_text = renderer
                        .module_registry
                        .get("battery")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| {
                            let icon = renderer.icons.get("battery");
                            format!("{} --", icon)
                        });
                    if !battery_text.is_empty() {
                        // Dynamically calculate width based on actual display text
                        let (text_width, _) = measure_text(hdc, &battery_text);
                        let min_width = text_width + item_padding * 2;
                        x -= min_width;
                        let battery_rect = draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &battery_text,
                            item_padding,
                            min_width,
                            theme,
                            renderer.dpi,
                        );
                        renderer.module_bounds
                            .insert("battery".to_string(), battery_rect);
                        x -= item_spacing;
                    }
                }

                "volume" => {
                    let volume_text = renderer
                        .module_registry
                        .get("volume")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| renderer.icons.get("volume_high"));
                    // Dynamically calculate width based on actual display text
                    let (text_width, _) = measure_text(hdc, &volume_text);
                    let min_width = text_width + item_padding * 2;
                    x -= min_width;
                    let volume_rect = draw_module_text_fixed(
                        hdc,
                        x,
                        bar_rect.height,
                        &volume_text,
                        item_padding,
                        min_width,
                        theme,
                        renderer.dpi,
                    );
                    renderer.module_bounds.insert("volume".to_string(), volume_rect);
                    x -= item_spacing;
                }

                "network" => {
                    // Use Segoe Fluent Icons for the network glyphs so they render correctly
                    let net_font = create_font("Segoe Fluent Icons", scale(15, renderer.dpi), false);
                    unsafe {
                        let old_font = SelectObject(hdc, net_font);

                        let network_text = renderer
                            .module_registry
                            .get("network")
                            .map(|m| {
                                let t = m.display_text(config.as_ref());
                                if t.trim().is_empty() {
                                    renderer.icons.get("wifi")
                                } else {
                                    t
                                }
                            })
                            .unwrap_or_else(|| renderer.icons.get("wifi"));

                        // Switch back to default font for measuring text with speed numbers
                        let _ = SelectObject(hdc, old_font);

                        // Dynamically calculate width based on actual display text
                        let (text_width, text_height) = measure_text(hdc, &network_text);
                        let width = text_width + item_padding * 2;
                        let height = text_height + item_padding + 2;
                        let y = (bar_rect.height - height) / 2;

                        x -= width;

                        // Switch back to Fluent font for drawing the icon
                        let _ = SelectObject(hdc, net_font);
                        SetTextColor(hdc, theme.text_primary.colorref());
                        let text_y = (bar_rect.height - text_height) / 2;
                        // Center text within the calculated width
                        let text_x = x + (width - text_width) / 2;
                        draw_text(hdc, text_x, text_y, &network_text);

                        let network_rect = Rect::new(x, y, width, height);
                        renderer.module_bounds
                            .insert("network".to_string(), network_rect);
                        x -= item_spacing;

                        let _ = SelectObject(hdc, old_font);
                        let _ = DeleteObject(net_font);
                    }
                }

                "system_info" => {
                    let show_graph = config.modules.system_info.show_graph;
                    if show_graph {
                        let graph_width = scale(60, renderer.dpi);
                        let graph_height = bar_rect.height - scale(8, renderer.dpi);
                        x -= graph_width + item_padding * 2;

                        let rect = Rect::new(
                            x,
                            (bar_rect.height - graph_height) / 2,
                            graph_width + item_padding * 2,
                            graph_height,
                        );
                        
                        // Draw system info graphs (CPU and RAM)
                        if let Some(module) = renderer.module_registry.get("system_info") {
                            use crate::modules::system_info::SystemInfoModule;
                            let max_points = (rect.width - item_padding * 2).max(1) as usize;

                            if let Some(si) = module.as_any().downcast_ref::<SystemInfoModule>() {
                                let cpu_bars = downsample_values(si.cpu_history(), max_points);
                                let mem_bars = downsample_values(si.memory_history(), max_points);

                                draw_line_graph(hdc, &cpu_bars, &rect, item_padding, theme.text_primary.colorref());
                                draw_line_graph(hdc, &mem_bars, &rect, item_padding, theme.text_secondary.colorref());

                                // Labels
                                unsafe {
                                    let small_font = create_font("Segoe UI Variable Text", scale(9, renderer.dpi), false);
                                    let prev_font = SelectObject(hdc, small_font);
                                    let label_x = rect.x + item_padding + 2;
                                    let label_y = rect.y + 2;
                                    let _ = SetTextColor(hdc, theme.text_primary.colorref());
                                    draw_text(hdc, label_x, label_y, "CPU");
                                    let _ = SetTextColor(hdc, theme.text_secondary.colorref());
                                    draw_text(hdc, label_x + scale(30, renderer.dpi), label_y, "RAM");
                                    let _ = SelectObject(hdc, prev_font);
                                    let _ = DeleteObject(small_font);
                                }
                            } else if let Some(values) = module.graph_values() {
                                let bars = downsample_values(values, max_points);
                                draw_line_graph(hdc, &bars, &rect, item_padding, theme.text_secondary.colorref());
                                
                                unsafe {
                                    let small_font = create_font("Segoe UI Variable Text", scale(9, renderer.dpi), false);
                                    let prev_font = SelectObject(hdc, small_font);
                                    let _ = SetTextColor(hdc, theme.text_secondary.colorref());
                                    draw_text(hdc, rect.x + item_padding + 2, rect.y + 2, "CPU");
                                    let _ = SelectObject(hdc, prev_font);
                                    let _ = DeleteObject(small_font);
                                }
                            }
                        }

                        renderer.module_bounds.insert("system_info".to_string(), rect);
                        x -= item_spacing;
                    } else {
                        let sysinfo_text = renderer
                            .module_registry
                            .get("system_info")
                            .map(|m| m.display_text(config.as_ref()))
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
                        let (sample_w, _) = measure_text(hdc, sample_text);
                        let mut min_width = sample_w + item_padding * 2;
                        min_width = min_width.max(scale(64, renderer.dpi));

                        x -= min_width;

                        // Draw the percentage-only text (CPU / RAM)
                        let sysinfo_rect = draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &sysinfo_text,
                            item_padding,
                            min_width,
                            theme,
                            renderer.dpi,
                        );

                        renderer.module_bounds
                            .insert("system_info".to_string(), sysinfo_rect);
                        x -= item_spacing;
                    }
                }

                "media" => {
                    let media_text = renderer
                        .module_registry
                        .get("media")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_default();
                    if !media_text.is_empty() {
                        let (text_width, _) = measure_text(hdc, &media_text);
                        x -= text_width + item_padding * 2;
                        let media_rect = draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &media_text,
                            item_padding,
                            theme,
                            false,
                            None,
                            renderer.dpi,
                        );
                        renderer.module_bounds.insert("media".to_string(), media_rect);
                        x -= item_spacing;
                    }
                }

                "clipboard" => {
                    // Render clipboard module (shows latest entry or icon)
                    let clipboard_text = renderer
                        .module_registry
                        .get("clipboard")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| "📋".to_string());
                    let (text_width, _) = measure_text(hdc, &clipboard_text);
                    x -= text_width + item_padding * 2;
                    let clip_rect = draw_module_text(
                        hdc,
                        x,
                        bar_rect.height,
                        &clipboard_text,
                        item_padding,
                        theme,
                        false,
                        None,
                        renderer.dpi,
                    );
                    renderer.module_bounds
                        .insert("clipboard".to_string(), clip_rect);
                    x -= item_spacing;
                }

                "gpu" => {
                    let show_graph = config.modules.gpu.show_graph;
                    if show_graph {
                        let graph_width = scale(60, renderer.dpi);
                        let graph_height = bar_rect.height - scale(8, renderer.dpi);
                        x -= graph_width + item_padding * 2;

                        let rect = Rect::new(
                            x,
                            (bar_rect.height - graph_height) / 2,
                            graph_width + item_padding * 2,
                            graph_height,
                        );
                        
                        // Draw GPU graph
                        if let Some(module) = renderer.module_registry.get("gpu") {
                            if let Some(values) = module.graph_values() {
                                let max_points = (rect.width - item_padding * 2).max(1) as usize;
                                let bars = downsample_values(values, max_points);
                                draw_line_graph(hdc, &bars, &rect, item_padding, theme.text_primary.colorref());

                                unsafe {
                                    let small_font = create_font("Segoe UI Variable Text", scale(9, renderer.dpi), false);
                                    let prev_font = SelectObject(hdc, small_font);
                                    let _ = SetTextColor(hdc, theme.text_primary.colorref());
                                    draw_text(hdc, rect.x + item_padding + 2, rect.y + 2, "GPU");
                                    let _ = SelectObject(hdc, prev_font);
                                    let _ = DeleteObject(small_font);
                                }
                            }
                        }

                        renderer.module_bounds.insert("gpu".to_string(), rect);
                        x -= item_spacing;
                    } else {
                        let gpu_text = renderer
                            .module_registry
                            .get("gpu")
                            .map(|m| m.display_text(config.as_ref()))
                            .unwrap_or_else(|| renderer.icons.get("gpu"));
                        // Fixed width for "GPU 100%" format
                        let min_width = scale(92, renderer.dpi);
                        x -= min_width;

                        // Simple text-only rendering for GPU (percentage text)
                        let gpu_rect = draw_module_text_fixed(
                            hdc,
                            x,
                            bar_rect.height,
                            &gpu_text,
                            item_padding,
                            min_width,
                            theme,
                            renderer.dpi,
                        );
                        renderer.module_bounds.insert("gpu".to_string(), gpu_rect);
                        x -= item_spacing;
                    }
                }

                "keyboard_layout" => {
                    let keyboard_text = renderer
                        .module_registry
                        .get("keyboard_layout")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| "EN".to_string());
                    let (text_width, _) = measure_text(hdc, &keyboard_text);
                    x -= text_width + item_padding * 2;
                    let keyboard_rect = draw_module_text(
                        hdc,
                        x,
                        bar_rect.height,
                        &keyboard_text,
                        item_padding,
                        theme,
                        false,
                        None,
                        renderer.dpi,
                    );
                    renderer.module_bounds
                        .insert("keyboard_layout".to_string(), keyboard_rect);
                    x -= item_spacing;
                }

                "uptime" => {
                    let uptime_text = renderer
                        .module_registry
                        .get("uptime")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| "0d 0h".to_string());
                    let min_width = scale(72, renderer.dpi);
                    x -= min_width;
                    let uptime_rect = draw_module_text_fixed(
                        hdc,
                        x,
                        bar_rect.height,
                        &uptime_text,
                        item_padding,
                        min_width,
                        theme,
                        renderer.dpi,
                    );
                    renderer.module_bounds.insert("uptime".to_string(), uptime_rect);
                    x -= item_spacing;
                }

                "bluetooth" => {
                    // Use Segoe Fluent Icons for the Bluetooth glyph so the E702 codepoint renders correctly
                    let bt_font = create_font("Segoe Fluent Icons", scale(13, renderer.dpi), false);
                    unsafe {
                        let old_font = SelectObject(hdc, bt_font);

                        let bluetooth_text = renderer
                            .module_registry
                            .get("bluetooth")
                            .map(|m| {
                                let t = m.display_text(config.as_ref());
                                if t.trim().is_empty() {
                                    renderer.icons.get("bluetooth")
                                } else {
                                    t
                                }
                            })
                            .unwrap_or_else(|| renderer.icons.get("bluetooth"));

                        let (text_width, _) = measure_text(hdc, &bluetooth_text);
                        x -= text_width + item_padding * 2;
                        let bluetooth_rect = draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &bluetooth_text,
                            item_padding,
                            theme,
                            false,
                            None,
                            renderer.dpi,
                        );
                        renderer.module_bounds
                            .insert("bluetooth".to_string(), bluetooth_rect);
                        x -= item_spacing;

                        let _ = SelectObject(hdc, old_font);
                        let _ = DeleteObject(bt_font);
                    }
                }

                "night_light" => {
                    // Use Segoe UI Symbol for emoji rendering
                    let nl_font = create_font("Segoe UI Symbol", scale(14, renderer.dpi), false);
                    unsafe {
                        let old_font = SelectObject(hdc, nl_font);

                        let night_light_text = renderer
                            .module_registry
                            .get("night_light")
                            .map(|m| m.display_text(config.as_ref()))
                            .unwrap_or_else(|| "NL".to_string());
                        let (text_width, _) = measure_text(hdc, &night_light_text);
                        x -= text_width + item_padding * 2;
                        let night_light_rect = draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &night_light_text,
                            item_padding,
                            theme,
                            false,
                            None,
                            renderer.dpi,
                        );
                        renderer.module_bounds
                            .insert("night_light".to_string(), night_light_rect);
                        x -= item_spacing;

                        let _ = SelectObject(hdc, old_font);
                        let _ = DeleteObject(nl_font);
                    }
                }

                "disk" => {
                    let disk_width = scale(24, renderer.dpi);
                    let disk_height = bar_rect.height - scale(8, renderer.dpi);
                    x -= disk_width + item_padding * 2;

                    let rect = Rect::new(
                        x,
                        (bar_rect.height - disk_height) / 2,
                        disk_width + item_padding * 2,
                        disk_height,
                    );
                    unsafe {
                        // Draw directly on the bar; no background fill so visuals are clean
                        if let Some(module) = renderer.module_registry.get("disk") {
                            if let Some(disk_module) = module.as_any().downcast_ref::<crate::modules::disk::DiskModule>() {
                                let usage_percent = disk_module.primary_usage_percent() as f32 / 100.0;
                                
                                // Draw a very simple pie: a subtle background circle and a filled pie slice for used space
                                let center_x = rect.x + rect.width / 2;
                                let center_y = rect.y + rect.height / 2;
                                let radius = (rect.width.min(rect.height) / 2 - 2) as i32;
                                let left = center_x - radius;
                                let top = center_y - radius;
                                let right = center_x + radius;
                                let bottom = center_y + radius;

                                // Draw background circle (free space) - grey
                                let bg_brush = CreateSolidBrush(theme.text_secondary.colorref());
                                let old_bg_brush = SelectObject(hdc, bg_brush);
                                // No outline - use a transparent/null approach by not drawing a border
                                let _ = Ellipse(hdc, left, top, right, bottom);
                                let _ = SelectObject(hdc, old_bg_brush);
                                let _ = DeleteObject(bg_brush);

                                if usage_percent <= 0.0 {
                                    // nothing else to draw (empty disk - all free/grey)
                                } else if usage_percent >= 1.0 {
                                    // Full disk: draw filled circle using inverted colors (dark/inverted)
                                    let fg_brush = CreateSolidBrush(theme.background.colorref());
                                    let old_brush = SelectObject(hdc, fg_brush);
                                    let _ = Ellipse(hdc, left, top, right, bottom);
                                    let _ = SelectObject(hdc, old_brush);
                                    let _ = DeleteObject(fg_brush);
                                } else {
                                    let start = -std::f32::consts::PI / 2.0;
                                    let end = start + usage_percent * 2.0 * std::f32::consts::PI;
                                    let x1 = center_x + (start.cos() * radius as f32) as i32;
                                    let y1 = center_y + (start.sin() * radius as f32) as i32;
                                    let x2 = center_x + (end.cos() * radius as f32) as i32;
                                    let y2 = center_y + (end.sin() * radius as f32) as i32;

                                    // Draw used slice with inverted colors (dark background for used space)
                                    let fg_brush = CreateSolidBrush(theme.background.colorref());
                                    let old_brush = SelectObject(hdc, fg_brush);
                                    let _ = Pie(hdc, left, top, right, bottom, x1, y1, x2, y2);
                                    let _ = SelectObject(hdc, old_brush);
                                    let _ = DeleteObject(fg_brush);
                                }
                            }
                        }
                    }
                }

                "weather" => {
                    let weather_text = renderer
                        .module_registry
                        .get("weather")
                        .map(|m| m.display_text(config.as_ref()))
                        .unwrap_or_else(|| "🌡️ ...".to_string());
                    if !weather_text.is_empty() {
                        let (text_width, _) = measure_text(hdc, &weather_text);
                        x -= text_width + item_padding * 2;
                        let weather_rect = draw_module_text(
                            hdc,
                            x,
                            bar_rect.height,
                            &weather_text,
                            item_padding,
                            theme,
                            false,
                            None,
                            renderer.dpi,
                        );
                        renderer.module_bounds
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
                let display = renderer
                    .module_registry
                    .get(drag_id)
                    .map(|m| m.display_text(config.as_ref()))
                    .unwrap_or_else(|| drag_id.clone());

                let (text_w, text_h) = measure_text(hdc, &display);
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
                    draw_text(
                        hdc,
                        x_pos + item_padding,
                        (bar_rect.height - text_h) / 2,
                        &display,
                    );

                    // Draw insertion marker
                    let pen = CreatePen(PS_SOLID, 2, theme.accent.colorref());
                    let old_pen = SelectObject(hdc, pen);
                    let top = scale(6, renderer.dpi);
                    let bottom = bar_rect.height - scale(6, renderer.dpi);
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
pub fn draw_module_button(
    hdc: HDC,
    x: i32,
    bar_height: i32,
    text: &str,
    padding: i32,
    theme: &Theme,
    is_hovered: bool,
    dpi: u32,
) -> Rect {
    // Special-case single-glyph icons (menu, search, etc.) to render larger and centered
    let (text_width, text_height) = measure_text(hdc, text);
    let mut width = text_width + padding * 2;
    let mut height = text_height + padding + 4; // Slightly taller for better tap targets
    let y = (bar_height - height) / 2;

    unsafe {
        // If the text is a single glyph (likely an icon), draw it with a larger icon font
        if text.chars().count() == 1 {
            let icon_size = scale(16, dpi);
            let icon_font = create_font("Segoe UI Symbol", icon_size + 2, false);
            let old_font = SelectObject(hdc, icon_font);

            let (iw, ih) = measure_text(hdc, text);
            width = iw + padding * 2;
            height = ih + padding + 8; // a little extra for icons
            let y = (bar_height - height) / 2;

            // Draw subtle rounded background on hover
            if is_hovered {
                let brush = CreateSolidBrush(theme.background_hover.colorref());
                let rect = windows::Win32::Foundation::RECT {
                    left: x + 2,
                    top: y + 1,
                    right: x + width - 2,
                    bottom: y + height - 1,
                };
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(brush);
            }

            // Draw icon centered horizontally within the button area
            SetTextColor(hdc, theme.text_primary.colorref());
            let text_x = x + (width - iw) / 2;
            let text_y = (bar_height - ih) / 2;
            draw_text(hdc, text_x, text_y, text);

            // Restore and cleanup
            let _ = SelectObject(hdc, old_font);
            let _ = DeleteObject(icon_font);
        } else {
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
            draw_text(hdc, x + padding, text_y, text);
        }
    }

    Rect::new(x, y, width, height)
}

/// Draw module text with improved layout
pub fn draw_module_text(
    hdc: HDC,
    x: i32,
    bar_height: i32,
    text: &str,
    padding: i32,
    theme: &Theme,
    _bold: bool,
    icon: Option<HICON>,
    dpi: u32,
) -> Rect {
    let (text_width, text_height) = measure_text(hdc, text);
    let mut width = text_width + padding * 2;
    let icon_size = scale(16, dpi);
    let icon_spacing = scale(6, dpi);

    if icon.is_some() {
        width += icon_size + icon_spacing;
    }

    let height = text_height + padding + 2; // Balanced height
    let y = (bar_height - height) / 2;

    unsafe {
        // Use primary text color for good contrast
        SetTextColor(hdc, theme.text_primary.colorref());
        // Center text vertically with slight adjustment for visual balance
        let text_y = (bar_height - text_height) / 2;

        // Draw icon if provided
        if let Some(hicon) = icon {
            // Draw the icon at padding offset
            let icon_x = x + padding;
            let icon_y = (bar_height - icon_size) / 2;
            let _ = DrawIconEx(hdc, icon_x, icon_y, hicon, icon_size, icon_size, 0, HBRUSH::default(), DI_NORMAL);
            // Draw text after icon + spacing
            draw_text(hdc, x + padding + icon_size + icon_spacing, text_y, text);
        } else {
            draw_text(hdc, x + padding, text_y, text);
        }
    }

    Rect::new(x, y, width, height)
}

/// Compute a sample clock string representing the widest possible time
/// for the current configuration, used to calculate fixed width and prevent layout shifting.
pub fn clock_sample_text(config: &crate::config::Config, dpi: u32) -> String {
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

/// Draw module text with a minimum width to prevent layout shifting
pub fn draw_module_text_fixed(
    hdc: HDC,
    x: i32,
    bar_height: i32,
    text: &str,
    padding: i32,
    min_width: i32,
    theme: &Theme,
    dpi: u32,
) -> Rect {
    let (text_width, text_height) = measure_text(hdc, text);
    let width = (text_width + padding * 2).max(min_width);
    let height = text_height + padding + 2;
    let y = (bar_height - height) / 2;

    unsafe {
        SetTextColor(hdc, theme.text_primary.colorref());
        let text_y = (bar_height - text_height) / 2;
        // Center text within the fixed width
        let text_x = x + (width - text_width) / 2;
        draw_text(hdc, text_x, text_y, text);
    }

    Rect::new(x, y, width, height)
}

/// Try to load a small icon handle (HICON) for an executable path and cache it
pub fn get_small_icon_for_path(renderer: &mut super::renderer::Renderer, path: &str) -> Option<HICON> {
    if path.is_empty() {
        return None;
    }

    if let Some(icon) = renderer.icon_cache.get(path) {
        return Some(*icon);
    }

    unsafe {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut sfi = windows::Win32::UI::Shell::SHFILEINFOW::default();
        let flags = windows::Win32::UI::Shell::SHGFI_ICON | windows::Win32::UI::Shell::SHGFI_SMALLICON;
        let res = windows::Win32::UI::Shell::SHGetFileInfoW(
            windows::core::PCWSTR(wide.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut sfi as *mut windows::Win32::UI::Shell::SHFILEINFOW),
            std::mem::size_of::<windows::Win32::UI::Shell::SHFILEINFOW>() as u32,
            flags,
        );

        if res != 0 && !sfi.hIcon.is_invalid() {
            let icon = sfi.hIcon;
            renderer.icon_cache.insert(path.to_string(), icon);
            return Some(icon);
        }
    }

    None
}