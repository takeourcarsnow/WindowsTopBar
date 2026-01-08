//! Quick search UI - Spotlight-style popup for file search

use anyhow::Result;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::{ShellExecuteW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::Graphics::Gdi::*;

use crate::window::state::get_window_state;
use crate::theme::Color;
use crate::search; 
use std::path::Path;
use std::collections::HashMap;

const SEARCH_CLASS: &str = "TopBarQuickSearchClass";
const WIN_WIDTH: i32 = 620;
const WIN_HEIGHT: i32 = 420;
const ROW_HEIGHT: i32 = 56;
const RESULTS_START_Y: i32 = 72;
const MAX_RESULTS: usize = 6;
const INPUT_HEIGHT: i32 = 52;
const PADDING: i32 = 16;

struct SearchState {
    input: String,
    results: Vec<String>,
    selected: usize,
    focused: bool,
    icon_cache: HashMap<String, HICON>,
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
        icon_cache: HashMap::new(),
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

/// Get parent directory path (shortened for display)
fn get_parent_path(path: &str) -> String {
    let parent = Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    
    // Shorten the path if it's too long
    if parent.len() > 50 {
        let parts: Vec<&str> = parent.split('\\').collect();
        if parts.len() > 3 {
            format!("{}\\...\\{}", parts[0], parts[parts.len()-1])
        } else {
            parent.to_string()
        }
    } else {
        parent.to_string()
    }
}

/// Get the system icon for a file
unsafe fn get_file_icon(path: &str, cache: &mut HashMap<String, HICON>) -> Option<HICON> {
    // Check cache first
    if let Some(&icon) = cache.get(path) {
        return Some(icon);
    }
    
    let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut shfi = SHFILEINFOW::default();
    
    let result = SHGetFileInfoW(
        PCWSTR(wide_path.as_ptr()),
        windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
        Some(&mut shfi),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_SMALLICON,
    );
    
    if result != 0 && !shfi.hIcon.is_invalid() {
        cache.insert(path.to_string(), shfi.hIcon);
        Some(shfi.hIcon)
    } else {
        None
    }
}

/// Draw a rounded rectangle
unsafe fn draw_rounded_rect(hdc: HDC, rect: &RECT, radius: i32, brush: HBRUSH) {
    let rgn = CreateRoundRectRgn(rect.left, rect.top, rect.right, rect.bottom, radius, radius);
    let _ = FillRgn(hdc, rgn, brush);
    let _ = DeleteObject(rgn);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Some(state) = get_state(hwnd) {
                if let Some(gs) = get_window_state() {
                    let theme: crate::theme::Theme = gs.read().theme_manager.theme().clone();

                    // Main background - dark glass effect
                    let bg = CreateSolidBrush(Color::rgb(22, 22, 24).colorref());
                    FillRect(hdc, &ps.rcPaint, bg);
                    let _ = DeleteObject(bg);

                    SetBkMode(hdc, TRANSPARENT);

                    // ===== SEARCH INPUT AREA =====
                    // Input background (slightly lighter)
                    let input_bg = CreateSolidBrush(Color::rgb(38, 38, 42).colorref());
                    let input_rect = RECT {
                        left: PADDING,
                        top: PADDING,
                        right: WIN_WIDTH - PADDING,
                        bottom: PADDING + INPUT_HEIGHT,
                    };
                    draw_rounded_rect(hdc, &input_rect, 10, input_bg);
                    let _ = DeleteObject(input_bg);

                    // Search icon (magnifying glass)
                    let icon_font = CreateFontW(
                        20, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI Symbol").as_ptr())
                    );
                    let old_font = SelectObject(hdc, icon_font);
                    SetTextColor(hdc, Color::rgb(120, 120, 125).colorref());
                    let search_icon = "🔍";
                    let icon_wide: Vec<u16> = search_icon.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = TextOutW(hdc, PADDING + 14, PADDING + 14, &icon_wide[..icon_wide.len() - 1]);
                    let _ = SelectObject(hdc, old_font);
                    let _ = DeleteObject(icon_font);

                    // Input text
                    let input_font = CreateFontW(
                        18, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI").as_ptr())
                    );
                    let old_font = SelectObject(hdc, input_font);

                    let display = if state.input.is_empty() && search::is_index_ready() {
                        SetTextColor(hdc, Color::rgb(100, 100, 105).colorref());
                        "Search apps and files...".to_string()
                    } else if state.input.is_empty() {
                        SetTextColor(hdc, Color::rgb(100, 100, 105).colorref());
                        let scanned = search::scanned_count();
                        let est = search::estimated_total();
                        if est > 0 {
                            let pct = ((scanned * 100) / est).min(100);
                            format!("Indexing {} files... (~{}%)", scanned, pct)
                        } else {
                            format!("Indexing {} files...", scanned)
                        }
                    } else {
                        SetTextColor(hdc, Color::rgb(245, 245, 245).colorref());
                        state.input.clone()
                    };
                    let wide: Vec<u16> = display.encode_utf16().chain(std::iter::once(0)).collect();
                    let text_x = PADDING + 48;
                    let _ = TextOutW(hdc, text_x, PADDING + 16, &wide[..wide.len() - 1]);

                    // Cursor
                    if state.focused && !state.input.is_empty() {
                        let mut size = windows::Win32::Foundation::SIZE { cx: 0, cy: 0 };
                        let _ = GetTextExtentPoint32W(hdc, &wide[..wide.len() - 1], &mut size);
                        let cursor_x = text_x + size.cx + 2;
                        let cursor_brush = CreateSolidBrush(theme.accent.colorref());
                        let cursor_rect = RECT {
                            left: cursor_x, top: PADDING + 14, right: cursor_x + 2, bottom: PADDING + 38
                        };
                        FillRect(hdc, &cursor_rect, cursor_brush);
                        let _ = DeleteObject(cursor_brush);
                    }

                    let _ = SelectObject(hdc, old_font);
                    let _ = DeleteObject(input_font);

                    // ===== SEPARATOR LINE =====
                    let sep_brush = CreateSolidBrush(Color::rgb(50, 50, 55).colorref());
                    let sep_rect = RECT {
                        left: PADDING,
                        top: PADDING + INPUT_HEIGHT + 8,
                        right: WIN_WIDTH - PADDING,
                        bottom: PADDING + INPUT_HEIGHT + 9,
                    };
                    FillRect(hdc, &sep_rect, sep_brush);
                    let _ = DeleteObject(sep_brush);

                    // ===== RESULTS AREA =====
                    let mut y = RESULTS_START_Y;

                    // Fonts for results
                    let name_font = CreateFontW(
                        16, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI").as_ptr())
                    );
                    let path_font = CreateFontW(
                        12, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
                        PCWSTR(to_wide("Segoe UI").as_ptr())
                    );

                    if state.results.is_empty() {
                        let _ = SelectObject(hdc, name_font);
                        if search::is_index_ready() && state.input.is_empty() {
                            // Empty state with hint
                            SetTextColor(hdc, Color::rgb(100, 100, 105).colorref());
                            let msg = "Type to search for apps, files, and more";
                            let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, PADDING + 8, y + 16, &wide[..wide.len() - 1]);
                            
                            // Keyboard shortcut hint
                            let _ = SelectObject(hdc, path_font);
                            SetTextColor(hdc, Color::rgb(80, 80, 85).colorref());
                            let hint = "Press Enter to open • Esc to close";
                            let hint_wide: Vec<u16> = hint.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, PADDING + 8, y + 40, &hint_wide[..hint_wide.len() - 1]);
                        } else if !state.input.is_empty() {
                            // No results found
                            SetTextColor(hdc, Color::rgb(120, 120, 125).colorref());
                            let msg = format!("No results for \"{}\"", state.input);
                            let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, PADDING + 8, y + 16, &wide[..wide.len() - 1]);
                        }
                    } else {
                        for (i, path) in state.results.iter().enumerate().take(MAX_RESULTS) {
                            let is_selected = i == state.selected;
                            let row_rect = RECT {
                                left: PADDING - 4,
                                top: y,
                                right: WIN_WIDTH - PADDING + 4,
                                bottom: y + ROW_HEIGHT - 4,
                            };

                            // Selection background with rounded corners
                            if is_selected {
                                let sel = CreateSolidBrush(theme.accent.colorref());
                                draw_rounded_rect(hdc, &row_rect, 8, sel);
                                let _ = DeleteObject(sel);
                            } else {
                                // Subtle hover hint on alternate rows
                                if i % 2 == 1 {
                                    let alt_bg = CreateSolidBrush(Color::rgb(26, 26, 28).colorref());
                                    draw_rounded_rect(hdc, &row_rect, 8, alt_bg);
                                    let _ = DeleteObject(alt_bg);
                                }
                            }

                            // File icon - get actual system icon
                            if let Some(state_mut) = get_state_mut(hwnd) {
                                if let Some(icon) = get_file_icon(path, &mut state_mut.icon_cache) {
                                    let _ = DrawIconEx(
                                        hdc,
                                        PADDING + 8,
                                        y + 12,
                                        icon,
                                        24,  // width
                                        24,  // height
                                        0,
                                        None,
                                        DI_NORMAL,
                                    );
                                }
                            }

                            // Filename (bold)
                            let _ = SelectObject(hdc, name_font);
                            SetTextColor(hdc, if is_selected {
                                Color::rgb(255, 255, 255).colorref()
                            } else {
                                Color::rgb(240, 240, 242).colorref()
                            });
                            let filename = get_filename(path);
                            let name_wide: Vec<u16> = filename.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, PADDING + 48, y + 10, &name_wide[..name_wide.len() - 1]);

                            // Path (smaller, muted)
                            let _ = SelectObject(hdc, path_font);
                            SetTextColor(hdc, if is_selected {
                                Color::rgb(220, 220, 225).colorref()
                            } else {
                                Color::rgb(110, 110, 115).colorref()
                            });
                            let parent = get_parent_path(path);
                            let path_wide: Vec<u16> = parent.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, PADDING + 48, y + 30, &path_wide[..path_wide.len() - 1]);

                            y += ROW_HEIGHT;
                        }

                        // Result count indicator
                        let _ = SelectObject(hdc, path_font);
                        SetTextColor(hdc, Color::rgb(80, 80, 85).colorref());
                        let count_str = if state.results.len() > MAX_RESULTS {
                            format!("Showing {} of {} results", MAX_RESULTS, state.results.len())
                        } else {
                            format!("{} result{}", state.results.len(), if state.results.len() == 1 { "" } else { "s" })
                        };
                        let count_wide: Vec<u16> = count_str.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = TextOutW(hdc, PADDING + 8, WIN_HEIGHT - 28, &count_wide[..count_wide.len() - 1]);
                    }

                    let _ = DeleteObject(name_font);
                    let _ = DeleteObject(path_font);
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
                // Debounce searching: set a short timer and perform the search on timer to avoid blocking on every keystroke
                let _ = KillTimer(hwnd, 2);
                let _ = SetTimer(hwnd, 2, 120, None);
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
            let id = wparam.0 as u32;
            if id == 1 {
                if !search::is_index_ready() {
                    // Update without erasing background to avoid flicker
                    let _ = InvalidateRect(hwnd, None, false);
                }
            } else if id == 2 {
                // Debounced search timer fired
                if let Some(state) = get_state_mut(hwnd) {
                    do_search(state);
                    let _ = KillTimer(hwnd, 2);
                    let _ = InvalidateRect(hwnd, None, false);
                }
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
                // Use simpler contains-based search to find installed apps better
                state.results = idx.search_query(&state.input, 200);
            }
        }
    }
}

fn invalidate_result_row(hwnd: HWND, idx: usize) {
    unsafe {
        if idx >= MAX_RESULTS { return; }
        let top = RESULTS_START_Y + (idx as i32) * ROW_HEIGHT;
        let rect = RECT {
            left: PADDING - 4, top, right: WIN_WIDTH - PADDING + 4, bottom: top + ROW_HEIGHT
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
            let state = Box::from_raw(ptr);
            // Clean up cached icons
            for (_, icon) in state.icon_cache.iter() {
                let _ = DestroyIcon(*icon);
            }
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
