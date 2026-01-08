//! Window procedure for handling Windows messages
//!
//! Contains the main window message handler and related message processing logic.

use log::{debug, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::ClientToScreen;

use crate::render;
use crate::utils::Rect;

use super::state::get_window_state;
use super::renderer::with_renderer;
use super::menus::{show_context_menu, handle_menu_command};
use super::module_handlers::handle_module_click;

/// WM_MOUSELEAVE message constant
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Custom window messages
pub const WM_TOPBAR_UPDATE: u32 = WM_USER + 1;
pub const WM_TOPBAR_THEME_CHANGED: u32 = WM_USER + 2;
pub const WM_TOPBAR_TRAY: u32 = WM_USER + 3;
pub const WM_TOPBAR_MODULE_CLICK: u32 = WM_USER + 4;
pub const WM_TOPBAR_NIGHTLIGHT_TOGGLED: u32 = WM_USER + 5;

/// Window procedure for handling Windows messages
pub unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            debug!("Window created");
            LRESULT(0)
        }

        WM_PAINT => {
            if let Some(state) = get_window_state() {
                let state_guard = state.read();

                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let bar_rect = state_guard.bar_rect;
                let theme = state_guard.theme_manager.theme().clone();
                drop(state_guard);

                with_renderer(|renderer| {
                    renderer.paint(hdc, &bar_rect, &theme);
                });

                let _ = EndPaint(hwnd, &ps);

                if let Some(state) = get_window_state() {
                    state.write().needs_redraw = false;
                }
            }
            LRESULT(0)
        }

        WM_TIMER => {
            let timer_id = wparam.0;
            match timer_id {
                1 => {
                    // Clock update (1 second)
                    let _ = InvalidateRect(hwnd, None, false);
                }
                2 => {
                    // System info update (2 seconds)
                    let _ = InvalidateRect(hwnd, None, false);
                }
                3 => {
                    // Fast update for active window and animations (100ms)
                    // Always invalidate to keep active window responsive
                    let _ = InvalidateRect(hwnd, None, false);
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_HOTKEY => {
            // Global hotkeys (registered during window creation)
            if let Some(map) = crate::hotkey::global_hotkey_map() {
                let guard = map.lock();
                let id = wparam.0 as i32;
                if let Some(action) = guard.get(&id) {
                    match action {
                        crate::hotkey::HotkeyAction::QuickSearch => {
                            // Show quick search popup centered under the bar
                            let _ = render::show_quick_search(hwnd);
                        }
                        crate::hotkey::HotkeyAction::ToggleBar => {
                            // Toggle visibility via WindowManager post message
                            unsafe { let _ = PostMessageW(hwnd, WM_USER + 99, WPARAM(0), LPARAM(0)); }
                        }
                        _ => {}
                    }
                }
            }

            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            const DRAG_THRESHOLD: i32 = 6;

            if let Some(state) = get_window_state() {
                let mut state_guard = state.write();
                if !state_guard.is_hovered {
                    state_guard.is_hovered = true;
                    state_guard.needs_redraw = true;

                    // Track mouse leave
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }

                // If we have a clicked module and movement exceeds threshold, start a drag
                if state_guard.dragging_module.is_none() {
                    if let (Some(click_id), Some((cx, _cy))) =
                        (state_guard.clicked_module.clone(), state_guard.clicked_pos)
                    {
                        if (x - cx).abs() > DRAG_THRESHOLD {
                            debug!("Starting drag for module: {}", click_id);
                            state_guard.dragging_module = Some(click_id.clone());
                            state_guard.drag_start_x = cx;
                            state_guard.drag_current_x = x;
                            state_guard.hover_module = None;
                            state_guard.needs_redraw = true;
                        }
                    }
                } else {
                    // Update dragging position
                    state_guard.drag_current_x = x;
                    state_guard.needs_redraw = true;
                }

                // Only update hover when not dragging
                let currently_dragging = state_guard.dragging_module.clone();
                let current_hover = state_guard.hover_module.clone();
                drop(state_guard);

                if currently_dragging.is_none() {
                    // Update hover module based on position
                    let new_hover = with_renderer(|renderer| renderer.hit_test(x, y)).flatten();
                    if new_hover != current_hover {
                        if let Some(state) = get_window_state() {
                            let mut state_guard = state.write();
                            state_guard.hover_module = new_hover;
                            state_guard.needs_redraw = true;
                        }
                    }
                }
            }
            LRESULT(0)
        }

        WM_MOUSELEAVE => {
            if let Some(state) = get_window_state() {
                let mut state_guard = state.write();
                state_guard.is_hovered = false;
                state_guard.hover_module = None;
                state_guard.needs_redraw = true;
            }
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            let module_id = with_renderer(|renderer| renderer.hit_test(x, y)).flatten();
            if let Some(module_id) = module_id {
                debug!("Mouse down on module: {}", module_id);
                // Store the clicked module and preparatory drag state; do NOT trigger click yet
                if let Some(state) = get_window_state() {
                    let mut s = state.write();
                    s.clicked_module = Some(module_id.clone());
                    s.clicked_pos = Some((x, y));
                    s.dragging_module = None;
                    s.drag_start_x = x;
                    s.drag_current_x = x;

                    // Record origin (left/right and index) for later reordering
                    let cfg = (*s.config).clone();
                    if let Some(idx) = cfg
                        .modules
                        .left_modules
                        .iter()
                        .position(|m| m == &module_id)
                    {
                        s.drag_origin_side = Some("left".to_string());
                        s.drag_orig_index = Some(idx);
                    } else if let Some(idx) = cfg
                        .modules
                        .right_modules
                        .iter()
                        .position(|m| m == &module_id)
                    {
                        s.drag_origin_side = Some("right".to_string());
                        s.drag_orig_index = Some(idx);
                    } else {
                        s.drag_origin_side = None;
                        s.drag_orig_index = None;
                    }
                }

                // Capture mouse so we receive move/up events
                unsafe {
                    let _ = SetCapture(hwnd);
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let _y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Some(state) = get_window_state() {
                let mut s = state.write();

                // If a drag was in progress, finalize reorder
                if let Some(drag_id) = s.dragging_module.clone() {
                    // Use renderer bounds to determine insertion point
                    with_renderer(|renderer| {
                        let bounds = renderer.module_bounds().clone();

                        // Determine visual order for the origin side
                        let visual_list = if let Some(side) = &s.drag_origin_side {
                            if side == "left" {
                                s.config.modules.left_modules.clone()
                            } else {
                                s.config.modules.right_modules.clone()
                            }
                        } else {
                            vec![]
                        };

                        // Build visual vector of (id, rect) in left-to-right order
                        let mut visual: Vec<(String, crate::utils::Rect)> = Vec::new();
                        for id in visual_list.iter() {
                            if let Some(r) = bounds.get(id) {
                                visual.push((id.clone(), *r));
                            }
                        }

                        // Compute insertion index based on cursor x
                        let mut insert_idx = visual.len();
                        for (i, (_id, rect)) in visual.iter().enumerate() {
                            let mid = rect.x + rect.width / 2;
                            if s.drag_current_x < mid {
                                insert_idx = i;
                                break;
                            }
                        }

                        // Apply to config: remove original and insert at new index
                        let mut new_cfg = (*s.config).clone();
                        let vec_ref = if s.drag_origin_side.as_deref() == Some("left") {
                            &mut new_cfg.modules.left_modules
                        } else {
                            &mut new_cfg.modules.right_modules
                        };

                        if let Some(pos) = vec_ref.iter().position(|m| m == &drag_id) {
                            vec_ref.remove(pos);
                            let mut final_idx = insert_idx;
                            if final_idx > pos {
                                final_idx = final_idx.saturating_sub(1);
                            }
                            vec_ref.insert(final_idx, drag_id.clone());
                        }

                        // Save and apply config
                        if let Err(e) = new_cfg.save() {
                            warn!("Failed to save config after reorder: {}", e);
                        } else {
                            s.config = std::sync::Arc::new(new_cfg);
                        }
                    });

                    // Clear drag state
                    s.dragging_module = None;
                    s.clicked_module = None;
                    s.clicked_pos = None;
                    s.drag_origin_side = None;
                    s.drag_orig_index = None;
                    s.needs_redraw = true;
                    // Force redraw to reflect new ordering
                    unsafe {
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                } else if let Some(click_id) = s.clicked_module.clone() {
                    // No drag - treat as click
                    drop(s); // unlock briefly for handler
                    handle_module_click(hwnd, &click_id, x);
                    if let Some(state) = get_window_state() {
                        let mut s2 = state.write();
                        s2.clicked_module = None;
                        s2.clicked_pos = None;
                        s2.needs_redraw = true;
                    }
                }

                // Release mouse capture
                unsafe {
                    let _ = ReleaseCapture();
                }
            }

            LRESULT(0)
        }

        WM_RBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            debug!("Right click at ({}, {})", x, y);

            // Get screen coordinates
            let mut pt = windows::Win32::Foundation::POINT { x, y };
            let _ = ClientToScreen(hwnd, &mut pt);

            // Show context menu
            show_context_menu(hwnd, pt.x, pt.y);
            LRESULT(0)
        }

        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            debug!("Mouse wheel delta: {}", delta);

            // Get cursor position from lparam (client coords) and dispatch to the module under cursor
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            // Forward the scroll to the module under the cursor (if any)
            with_renderer(|renderer| {
                if let Some(module_id) = renderer.hit_test(x, y) {
                    if let Some(module) = renderer.module_registry.get_mut(&module_id) {
                        module.on_scroll(delta as i32);
                    }
                }
            });

            // Request redraw to reflect changed volume/tooltip immediately
            if let Some(state) = get_window_state() {
                state.write().needs_redraw = true;
            }
            let _ = InvalidateRect(hwnd, None, false);

            LRESULT(0)
        }

        WM_DISPLAYCHANGE => {
            // Monitor resolution changed
            if let Some(state) = get_window_state() {
                let mut state_guard = state.write();
                let dpi = state_guard.dpi;
                let config = state_guard.config.clone();
                state_guard.bar_rect = super::manager::WindowManager::calculate_bar_rect(&config, dpi);

                let rect = state_guard.bar_rect;
                drop(state_guard);

                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    SWP_NOACTIVATE,
                );
            }
            LRESULT(0)
        }

        WM_DPICHANGED => {
            let new_dpi = (wparam.0 & 0xFFFF) as u32;
            if let Some(state) = get_window_state() {
                let mut state_guard = state.write();
                state_guard.dpi = new_dpi;
                let config = state_guard.config.clone();
                state_guard.bar_rect = super::manager::WindowManager::calculate_bar_rect(&config, new_dpi);
            }
            LRESULT(0)
        }

        WM_SETTINGCHANGE => {
            // System settings changed (including theme)
            if let Some(state) = get_window_state() {
                let mut state_guard = state.write();
                if state_guard.theme_manager.check_system_theme() {
                    let theme = state_guard.theme_manager.theme().clone();
                    drop(state_guard);
                    let _ = super::manager::WindowManager::apply_window_style(hwnd, &theme);
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
            LRESULT(0)
        }

        WM_DEVICECHANGE => {
            // Handle device arrival/removal and trigger immediate Bluetooth refresh
            match wparam.0 as u32 {
                DBT_DEVICEARRIVAL | DBT_DEVICEREMOVECOMPLETE | DBT_DEVNODES_CHANGED => {
                    debug!("Device change event: {}", wparam.0 as u32);

                    // Some device-change events include a pointer to a DEV_BROADCAST_HDR in lparam
                    // Only refresh Bluetooth module when the change concerns a device interface
                    let mut should_refresh = true;
                    if lparam.0 != 0 {
                        unsafe {
                            let hdr = &*(lparam.0 as *const DEV_BROADCAST_HDR);
                            if hdr.dbch_devicetype != DBT_DEVTYP_DEVICEINTERFACE {
                                // Not a device interface change - skip unless it's a general devnode change
                                if wparam.0 as u32 != DBT_DEVNODES_CHANGED {
                                    debug!("Ignoring device change (not device interface): devtype={:?} wparam={}", hdr.dbch_devicetype, wparam.0 as u32);
                                    should_refresh = false;
                                }
                            }
                        }
                    }

                    if should_refresh {
                        // Trigger bluetooth module refresh immediately
                        with_renderer(|renderer| {
                            if let Some(module) = renderer.module_registry.get_mut("bluetooth") {
                                if let Some(bm) = module
                                    .as_any_mut()
                                    .downcast_mut::<crate::modules::bluetooth::BluetoothModule>() {
                                    bm.refresh();
                                }
                            }
                        });

                        // Request a redraw to update the UI
                        unsafe {
                            let _ = InvalidateRect(hwnd, None, false);
                        }
                    }
                }
                _ => {}
            }

            LRESULT(0)
        }

        WM_TOPBAR_UPDATE => {
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }

        WM_TOPBAR_THEME_CHANGED => {
            if let Some(state) = get_window_state() {
                let state_guard = state.read();
                let theme = state_guard.theme_manager.theme().clone();
                drop(state_guard);
                let _ = super::manager::WindowManager::apply_window_style(hwnd, &theme);
                let _ = InvalidateRect(hwnd, None, true);
            }
            LRESULT(0)
        }

        WM_TOPBAR_NIGHTLIGHT_TOGGLED => {
            // Refresh night_light module state and request a redraw
            with_renderer(|renderer| {
                if let Some(module) = renderer.module_registry.get_mut("night_light") {
                    if let Some(nm) = module.as_any_mut().downcast_mut::<crate::modules::night_light::NightLightModule>() {
                        nm.refresh();
                    }
                }
            });

            if let Some(state) = get_window_state() {
                state.write().needs_redraw = true;
            }

            unsafe {
                let _ = InvalidateRect(hwnd, None, false);
            }

            LRESULT(0)
        }

        WM_DESTROY => {
            info!("Window destroyed, quitting application");
            super::manager::WindowManager::remove_screen_space(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_CLOSE => {
            DestroyWindow(hwnd).ok();
            LRESULT(0)
        }

        WM_COMMAND => {
            let cmd_id = (wparam.0 & 0xFFFF) as u32;
            handle_menu_command(hwnd, cmd_id);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}