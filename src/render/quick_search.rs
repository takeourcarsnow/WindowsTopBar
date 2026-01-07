//! Quick search UI - a minimal popup for file search

use anyhow::Result;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::Graphics::Gdi::*;

use crate::window::get_window_state;
use crate::theme::Color;
use crate::search;

const SEARCH_CLASS: &str = "TopBarQuickSearchClass";

struct SearchState {
    input: String,
    results: Vec<String>,
    selected: usize,
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
            0, 0, 600, 300,
            parent,
            None,
            hinstance,
            None,
        )?
    };

    // Center near top of screen
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_w - 600) / 2;
        SetWindowPos(hwnd, HWND_TOPMOST, x, 40, 600, 300, SWP_SHOWWINDOW).ok();
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
    }

    // Store state
    let state = Box::new(SearchState {
        input: String::new(),
        results: Vec::new(),
        selected: 0,
    });
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize); }

    // Start timer to update progress display (every 200ms)
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
        hCursor: LoadCursorW(None, IDC_IBEAM)?,
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

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Some(state) = get_state(hwnd) {
                if let Some(gs) = get_window_state() {
                    let theme = gs.read().theme_manager.theme().clone();

                    // Background
                    let bg = CreateSolidBrush(theme.background.colorref());
                    FillRect(hdc, &ps.rcPaint, bg);
                    let _ = DeleteObject(bg);

                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, theme.text_primary.colorref());

                    // Input text + cursor
                    let display = format!("{}|", state.input);
                    let wide: Vec<u16> = display.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = TextOutW(hdc, 20, 12, &wide[..wide.len() - 1]);

                    // Status or results
                    let mut y = 44;
                    if state.results.is_empty() {
                        let msg = if search::is_index_ready() {
                            if state.input.is_empty() {
                                "Type to search...".to_string()
                            } else {
                                "No results".to_string()
                            }
                        } else if search::is_building() {
                            format!("Building index ({} files)...", search::scanned_count())
                        } else {
                            format!("Scanning: {} files...", search::scanned_count())
                        };
                        SetTextColor(hdc, theme.text_secondary.colorref());
                        let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = TextOutW(hdc, 20, y, &wide[..wide.len() - 1]);
                    } else {
                        for (i, path) in state.results.iter().enumerate().take(10) {
                            // Highlight selected
                            if i == state.selected {
                                let sel = CreateSolidBrush(theme.accent.colorref());
                                let rect = windows::Win32::Foundation::RECT {
                                    left: 12, top: y - 2, right: 588, bottom: y + 18
                                };
                                FillRect(hdc, &rect, sel);
                                let _ = DeleteObject(sel);
                                SetTextColor(hdc, Color::rgb(255, 255, 255).colorref());
                            } else {
                                SetTextColor(hdc, theme.text_primary.colorref());
                            }

                            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = TextOutW(hdc, 20, y, &wide[..wide.len() - 1]);
                            y += 22;
                        }
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
                    '\u{8}' => { state.input.pop(); }  // backspace
                    '\r' | '\n' => {}  // ignore enter here
                    _ if ch.is_ascii_graphic() || ch == ' ' => {
                        state.input.push(ch);
                    }
                    _ => {}
                }
                do_search(state);
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            match vk {
                0x1B => { // ESC - close
                    close_window(hwnd);
                }
                0x26 => { // UP
                    if let Some(state) = get_state_mut(hwnd) {
                        if !state.results.is_empty() {
                            if state.selected == 0 {
                                state.selected = state.results.len().min(10) - 1;
                            } else {
                                state.selected -= 1;
                            }
                            let _ = InvalidateRect(hwnd, None, false);
                        }
                    }
                }
                0x28 => { // DOWN
                    if let Some(state) = get_state_mut(hwnd) {
                        if !state.results.is_empty() {
                            state.selected = (state.selected + 1) % state.results.len().min(10);
                            let _ = InvalidateRect(hwnd, None, false);
                        }
                    }
                }
                0x0D => { // ENTER - open selected
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
            if y >= 44 {
                let idx = ((y - 44) / 22) as usize;
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

        WM_KILLFOCUS => {
            // Close when losing focus
            close_window(hwnd);
            LRESULT(0)
        }

        WM_TIMER => {
            // Refresh display while indexing
            if !search::is_index_ready() {
                let _ = InvalidateRect(hwnd, None, false);
            } else {
                // Stop timer once index is ready
                let _ = KillTimer(hwnd, 1);
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
            state.results = idx.search_prefix(&state.input, 20);
        }
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
