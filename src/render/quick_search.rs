//! Quick search UI - Spotlight-style popup for file search

use anyhow::Result;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::{ShellExecuteW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;

use crate::window::state::get_window_state;
use crate::theme::Color;
use crate::search;
use crate::effects::EffectsManager;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn, DeleteObject};
use std::path::Path;

const SEARCH_CLASS: &str = "TopBarQuickSearchClass";
const WIN_WIDTH: i32 = 680;
const WIN_HEIGHT: i32 = 420;
const ROW_HEIGHT: i32 = 44;
const RESULTS_START_Y: i32 = 60;
const MAX_RESULTS: usize = 8;

struct SearchState {
    input: String,
    results: Vec<String>,
    selected: usize,
    focused: bool,
}

pub fn show_quick_search(parent: HWND) -> Result<()> {
    unsafe { register_class()?; }

    let hwnd = unsafe {
        let class = to_wide(SEARCH_CLASS);
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0, 0, WIN_WIDTH, WIN_HEIGHT,
            parent,
            None,
            hinstance,
            None,
        )?
    };

    // Center near top of screen
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_w - WIN_WIDTH) / 2;
        SetWindowPos(hwnd, HWND_TOPMOST, x, 80, WIN_WIDTH, WIN_HEIGHT, SWP_SHOWWINDOW).ok();
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);

        // Simplified window chrome: no acrylic/frosted glass or rounded corners to avoid rendering glitches.
        // The background is painted as a solid color in WM_PAINT.
    }

    // Store state
    let state = Box::new(SearchState {
        input: String::new(),
        results: Vec::new(),
        selected: 0,
        focused: true,
    });
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize); }

    // Timer for progress updates
    unsafe { SetTimer(hwnd, 1, 200, None); }

    Ok(())
}

unsafe fn register_class() -> Result<()> {
    let class_name = to_wide(SEARCH_CLASS);
    let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance.into(),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hbrBackground: HBRUSH::default(),
        ..Default::default()
    };
    let _ = RegisterClassExW(&wc);
    Ok(())
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Extract filename from full path
fn get_filename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

/// Get parent directory path
fn get_parent_path(path: &str) -> &str {
    Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Some(state) = get_state(hwnd) {
                if let Some(gs) = get_window_state() {
                    let theme: crate::theme::Theme = gs.read().theme_manager.theme().clone();

                    // Dark solid background (dark grey)
                    let bg = CreateSolidBrush(Color::rgb(28, 28, 30).colorref());
                    FillRect(hdc, &ps.rcPaint, bg);
                    let _ = DeleteObject(bg);

                    // No rectangular backdrop for the input — keep text background transparent over the solid fill
                    SetBkMode(hdc, TRANSPARENT);

                    // Create slightly smaller font for search input for a simpler, more stable render
                    let input_font = CreateFontW(
                        18, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI").as_ptr())
                    );
                    let old_font = SelectObject(hdc, input_font);

                    // Simplified input: no decorative glyph to reduce rendering complexity and glitches. Left padding preserved.
                    // (Previously a large glyph was drawn here; removed for simplicity.)

                    // Input text (shifted further right to account for larger icon)
                    SetTextColor(hdc, Color::rgb(245, 245, 245).colorref());
                    let display = if state.input.is_empty() && search::is_index_ready() {
                        "Search files...".to_string()
                    } else if state.input.is_empty() {
                        // Show scanned count with percent if we have an estimate
                        let scanned = search::scanned_count();
                        let est = search::estimated_total();
                        if est > 0 {
                            let pct = ((scanned * 100) / est).min(100);
                            format!("Indexing {} files... (~{}%)", scanned, pct)
                        } else {
                            format!("Indexing {} files...", scanned)
                        }
                    } else {
                        state.input.clone()
                    };
                    let wide: Vec<u16> = display.encode_utf16().chain(std::iter::once(0)).collect();
                    // Draw input text starting closer to the left edge for a simpler layout
                    let _ = TextOutW(hdc, 32, 20, &wide[..wide.len() - 1]);

                    // Draw cursor at end of visible input text using measured width to avoid mispositioning
                    if state.focused && !state.input.is_empty() {
                        let mut size = windows::Win32::Foundation::SIZE { cx: 0, cy: 0 };
                        let _ = GetTextExtentPoint32W(hdc, &wide[..wide.len() - 1], &mut size);
                        let cursor_x = 32 + size.cx; // match input left offset
                        let cursor_brush = CreateSolidBrush(Color::rgb(245, 245, 245).colorref());
                        let cursor_rect = windows::Win32::Foundation::RECT {
                            left: cursor_x, top: 20, right: cursor_x + 2, bottom: 42
                        };
                        FillRect(hdc, &cursor_rect, cursor_brush);
                        let _ = DeleteObject(cursor_brush);
                    }

                    let _ = SelectObject(hdc, old_font);
                    let _ = DeleteObject(input_font);

                    // Results area
                    let mut y = RESULTS_START_Y;
                    
                    // Create fonts for results
                    let name_font = CreateFontW(
                        16, 0, 0, 0, FW_MEDIUM.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI").as_ptr())
                    );
                    let path_font = CreateFontW(
                        13, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI").as_ptr())
                    );

                    if state.results.is_empty() {
                        let _ = SelectObject(hdc, path_font);
                        // Avoid duplicating the indexing message (already shown in the input area).
                        // Only show hints when the index is ready or there are no files found.
                        let msg = if !search::is_index_ready() {
                            String::new()
                        } else if state.input.is_empty() {
                            "Start typing to search".to_string()
                        } else {
                            "No files found".to_string()
                        };

                        if !msg.is_empty() {
                            SetTextColor(hdc, Color::rgb(140, 140, 140).colorref());
                            let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, 24, y + 10, &wide[..wide.len() - 1]);
                        }
                    } else {
                        for (i, path) in state.results.iter().enumerate().take(MAX_RESULTS) {
                            let is_selected = i == state.selected;

                            // Selection background
                            if is_selected {
                                let sel = CreateSolidBrush(theme.accent.colorref());
                                let rect = windows::Win32::Foundation::RECT {
                                    left: 8, top: y, right: WIN_WIDTH - 8, bottom: y + ROW_HEIGHT
                                };
                                FillRect(hdc, &rect, sel);
                                let _ = DeleteObject(sel);
                            }

                            // File icon
                            let mut sfi = SHFILEINFOW::default();
                            let widepath: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = SHGetFileInfoW(
                                PCWSTR(widepath.as_ptr()),
                                FILE_FLAGS_AND_ATTRIBUTES(0),
                                Some(&mut sfi),
                                std::mem::size_of::<SHFILEINFOW>() as u32,
                                SHGFI_ICON | SHGFI_SMALLICON
                            );
                            if !sfi.hIcon.is_invalid() {
                                let _ = DrawIconEx(hdc, 20, y + 6, sfi.hIcon, 32, 32, 0, HBRUSH::default(), DI_NORMAL);
                                let _ = DestroyIcon(sfi.hIcon);
                            }

                            // Filename (bold/larger)
                            let _ = SelectObject(hdc, name_font);
                            SetTextColor(hdc, if is_selected {
                                Color::rgb(255, 255, 255).colorref()
                            } else {
                                Color::rgb(240, 240, 240).colorref()
                            });
                            let filename = get_filename(path);
                            let name_wide: Vec<u16> = filename.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, 62, y + 6, &name_wide[..name_wide.len() - 1]);

                            // Path (smaller, dimmer)
                            let _ = SelectObject(hdc, path_font);
                            SetTextColor(hdc, if is_selected {
                                Color::rgb(220, 220, 220).colorref()
                            } else {
                                Color::rgb(120, 120, 120).colorref()
                            });
                            let parent = get_parent_path(path);
                            let path_wide: Vec<u16> = parent.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, 62, y + 24, &path_wide[..path_wide.len() - 1]);

                            y += ROW_HEIGHT;
                        }
                    }

                    let _ = DeleteObject(name_font);
                    let _ = DeleteObject(path_font);

                    // Bottom hint
                    if !state.results.is_empty() {
                        let hint_font = CreateFontW(
                            12, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                            DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                            PCWSTR(to_wide("Segoe UI").as_ptr())
                        );
                        let _ = SelectObject(hdc, hint_font);
                        SetTextColor(hdc, Color::rgb(100, 100, 100).colorref());
                        let hint = "↑↓ Navigate  ⏎ Open  Esc Close";
                        let hint_wide: Vec<u16> = hint.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = TextOutW(hdc, 24, WIN_HEIGHT - 28, &hint_wide[..hint_wide.len() - 1]);
                        let _ = DeleteObject(hint_font);
                    }
                }
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_CHAR => {
            if let Some(state) = get_state_mut(hwnd) {
                let ch = (wparam.0 & 0xFF) as u8 as char;
                match ch {
                    '\u{8}' => { state.input.pop(); }
                    '\r' | '\n' => {}
                    _ if ch.is_ascii_graphic() || ch == ' ' => {
                        state.input.push(ch);
                    }
                    _ => {}
                }
                do_search(state);
                // Invalidate without erasing background to avoid flicker
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            match vk {
                0x1B => close_window(hwnd),
                0x26 => { // UP
                    if let Some(state) = get_state_mut(hwnd) {
                        if !state.results.is_empty() {
                            let max = state.results.len().min(MAX_RESULTS);
                            let old = state.selected;
                            state.selected = if state.selected == 0 { max - 1 } else { state.selected - 1 };
                            // Only redraw the previously selected and newly selected rows to avoid flashing
                            invalidate_result_row(hwnd, old);
                            invalidate_result_row(hwnd, state.selected);
                        }
                    }
                }
                0x28 => { // DOWN
                    if let Some(state) = get_state_mut(hwnd) {
                        if !state.results.is_empty() {
                            let max = state.results.len().min(MAX_RESULTS);
                            let old = state.selected;
                            state.selected = (state.selected + 1) % max;
                            invalidate_result_row(hwnd, old);
                            invalidate_result_row(hwnd, state.selected);
                        }
                    }
                }
                0x0D => { // ENTER
                    if let Some(state) = get_state(hwnd) {
                        if let Some(path) = state.results.get(state.selected) {
                            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                            ShellExecuteW(None, PCWSTR::null(), PCWSTR(wide.as_ptr()), None, None, SW_SHOWNORMAL);
                            close_window(hwnd);
                        }
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let y = (lparam.0 >> 16) as i16 as i32;
            if y >= RESULTS_START_Y {
                let idx = ((y - RESULTS_START_Y) / ROW_HEIGHT) as usize;
                if let Some(state) = get_state(hwnd) {
                    if let Some(path) = state.results.get(idx) {
                        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                        ShellExecuteW(None, PCWSTR::null(), PCWSTR(wide.as_ptr()), None, None, SW_SHOWNORMAL);
                        close_window(hwnd);
                    }
                }
            }
            LRESULT(0)
        }

        WM_SETFOCUS => {
            if let Some(state) = get_state_mut(hwnd) {
                state.focused = true;
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }

        WM_KILLFOCUS => {
            if let Some(state) = get_state_mut(hwnd) {
                state.focused = false;
            }
            close_window(hwnd);
            LRESULT(0)
        }

        WM_TIMER => {
            if !search::is_index_ready() {
                    // Update without erasing background to avoid flicker
                    let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            let _ = KillTimer(hwnd, 1);
            free_state(hwnd);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn do_search(state: &mut SearchState) {
    state.results.clear();
    state.selected = 0;

    if state.input.is_empty() {
        return;
    }

    if let Some(index) = search::global_index() {
        if let Some(ref idx) = *index.read() {
            // If input starts with '.', treat as extension search
            if state.input.starts_with('.') {
                state.results = idx.search_by_extension(&state.input, 200);
            } else {
                state.results = idx.search_prefix(&state.input, 200);
            }
        }
    }
}

fn invalidate_result_row(hwnd: HWND, idx: usize) {
    unsafe {
        if idx >= MAX_RESULTS { return; }
        let top = RESULTS_START_Y + (idx as i32) * ROW_HEIGHT;
        let rect = windows::Win32::Foundation::RECT {
            left: 8, top, right: WIN_WIDTH - 8, bottom: top + ROW_HEIGHT
        };
        let _ = InvalidateRect(hwnd, Some(&rect), false);
    }
}

fn get_state(hwnd: HWND) -> Option<&'static SearchState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SearchState;
        if ptr.is_null() { None } else { Some(&*ptr) }
    }
}

fn get_state_mut(hwnd: HWND) -> Option<&'static mut SearchState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SearchState;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

fn free_state(hwnd: HWND) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SearchState;
        if !ptr.is_null() {
            let _ = Box::from_raw(ptr);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
    }
}

fn close_window(hwnd: HWND) {
    unsafe {
        free_state(hwnd);
        let _ = DestroyWindow(hwnd);
    }
}
