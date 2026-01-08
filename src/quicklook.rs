//! QuickLook - macOS-style file preview with spacebar
//!
//! Press spacebar when a file is selected in Explorer to get a quick preview.

use anyhow::Result;
use log::{debug, info};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT, BOOL};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_SPACE};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::theme::Color;

// Use the `image` crate for decoding common image formats
extern crate image;
use crate::window::state::get_window_state;

const QUICKLOOK_CLASS: &str = "TopBarQuickLookClass";
const MAX_TEXT_PREVIEW_SIZE: u64 = 1024 * 1024; // 1MB max for text preview

static QUICKLOOK_RUNNING: AtomicBool = AtomicBool::new(false);
// Store hook handle as raw isize for thread safety
static HOOK_HANDLE_RAW: AtomicIsize = AtomicIsize::new(0);
// Store preview window handle as raw isize
static PREVIEW_HWND_RAW: AtomicIsize = AtomicIsize::new(0);

/// File preview types
#[derive(Debug, Clone)]
enum PreviewContent {
    Image(PathBuf),
    Text(String),
    Unsupported(String), // Extension name
}

/// QuickLook state
struct QuickLookState {
    file_path: PathBuf,
    content: PreviewContent,
    file_icon: Option<HICON>,
    scroll_offset: i32,
    image_data: Option<ImageData>,
}

/// Loaded image data for rendering
struct ImageData {
    width: i32,
    height: i32,
    bitmap: HBITMAP,
}

/// Start the QuickLook hook system
pub fn start_quicklook_hook() -> Result<()> {
    if QUICKLOOK_RUNNING.load(Ordering::SeqCst) {
        return Ok(());
    }

    info!("Starting QuickLook keyboard hook");

    // Install low-level keyboard hook
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0)?;
        HOOK_HANDLE_RAW.store(hook.0 as isize, Ordering::SeqCst);
    }

    QUICKLOOK_RUNNING.store(true, Ordering::SeqCst);
    info!("QuickLook hook installed successfully");
    Ok(())
}

/// Stop the QuickLook hook system
pub fn stop_quicklook_hook() {
    if !QUICKLOOK_RUNNING.load(Ordering::SeqCst) {
        return;
    }

    info!("Stopping QuickLook keyboard hook");

    let hook_raw = HOOK_HANDLE_RAW.swap(0, Ordering::SeqCst);
    if hook_raw != 0 {
        unsafe {
            let hook = HHOOK(hook_raw as *mut std::ffi::c_void);
            let _ = UnhookWindowsHookEx(hook);
        }
    }

    // Close any open preview window
    close_preview_window();

    QUICKLOOK_RUNNING.store(false, Ordering::SeqCst);
}

/// Low-level keyboard hook procedure
unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && wparam.0 == WM_KEYDOWN as usize {
        let kb_struct = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        
        // Check if spacebar was pressed
        if kb_struct.vkCode == VK_SPACE.0 as u32 {
            // Check if Explorer or Desktop has focus
            if is_explorer_focused() {
                // Get selected file
                if let Some(file_path) = get_selected_file() {
                    debug!("QuickLook: Spacebar pressed on file: {:?}", file_path);

                    // Determine current preview state
                    let current_hwnd_raw = PREVIEW_HWND_RAW.load(Ordering::SeqCst);
                    if current_hwnd_raw != 0 {
                        // There's an existing preview - check its file
                        let hwnd = HWND(current_hwnd_raw as *mut std::ffi::c_void);
                        if let Some(state) = get_preview_state(hwnd) {
                            if state.file_path == file_path {
                                // Same file selected - toggle off
                                close_preview_window();
                                return LRESULT(1); // consume
                            } else {
                                // Different file selected - replace preview with new file
                                close_preview_window();
                                if show_preview(&file_path).is_ok() {
                                    return LRESULT(1); // consume
                                }
                            }
                        } else {
                            // Couldn't read state - just recreate
                            close_preview_window();
                            if show_preview(&file_path).is_ok() {
                                return LRESULT(1);
                            }
                        }
                    } else {
                        // No preview open - open new one
                        if show_preview(&file_path).is_ok() {
                            return LRESULT(1);
                        }
                    }
                }
            }
        }
        
        // Check if Escape was pressed to close preview
        if kb_struct.vkCode == 0x1B { // VK_ESCAPE
            let current_hwnd = PREVIEW_HWND_RAW.load(Ordering::SeqCst);
            if current_hwnd != 0 {
                close_preview_window();
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

/// Check if Windows Explorer or Desktop has focus
fn is_explorer_focused() -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return false;
        }

        // Get the class name of the foreground window
        let mut class_name = [0u16; 256];
        let len = GetClassNameW(foreground, &mut class_name);
        if len == 0 {
            return false;
        }

        let class = String::from_utf16_lossy(&class_name[..len as usize]);
        
        // Check for Explorer window classes
        let explorer_classes = [
            "CabinetWClass",        // Explorer window
            "ExploreWClass",        // Explorer window (older)
            "Progman",              // Desktop
            "WorkerW",              // Desktop worker window
            "SHELLDLL_DefView",     // Shell view
        ];

        for ec in &explorer_classes {
            if class.contains(ec) {
                return true;
            }
        }

        // Also check child windows for SHELLDLL_DefView
        let mut found = false;
        let _ = EnumChildWindows(
            foreground,
            Some(check_shell_view_callback),
            LPARAM(&mut found as *mut bool as isize),
        );

        found
    }
}

unsafe extern "system" fn check_shell_view_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let found = &mut *(lparam.0 as *mut bool);
    
    let mut class_name = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut class_name);
    if len > 0 {
        let class = String::from_utf16_lossy(&class_name[..len as usize]);
        if class.contains("SHELLDLL_DefView") || class.contains("DirectUIHWND") {
            *found = true;
            return BOOL(0); // Stop enumeration
        }
    }
    BOOL(1) // Continue enumeration
}

/// Get the currently selected file in Explorer using UI Automation
fn get_selected_file() -> Option<PathBuf> {
    // Try shell-based Explorer selection first, then desktop listview as a fallback
    if let Some(p) = get_selected_file_via_shell() { return Some(p); }
    get_desktop_selection()
}

/// Get selected file via Shell COM interfaces
fn get_selected_file_via_shell() -> Option<PathBuf> {
    unsafe {
        use windows::Win32::System::Com::*;
        use windows::Win32::UI::Shell::*;
        use windows::core::Interface;

        // Initialize COM if needed
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Get the shell windows object
        let shell_windows: IShellWindows = CoCreateInstance(
            &ShellWindows,
            None,
            CLSCTX_ALL,
        ).ok()?;

        // Get the foreground window
        let foreground = GetForegroundWindow();

        // Iterate through shell windows to find the active one
        let count = shell_windows.Count().ok()?;
        
        for i in 0..count {
            let variant = windows::core::VARIANT::from(i);
            
            if let Ok(disp) = shell_windows.Item(&variant) {
                // Try to get IWebBrowserApp interface
                let browser: IWebBrowserApp = disp.cast().ok()?;
                
                // Check if this is the foreground window
                if let Ok(hwnd_val) = browser.HWND() {
                    // hwnd_val is SHANDLE_PTR - convert to HWND by using its raw value
                    let browser_hwnd = HWND(std::mem::transmute_copy(&hwnd_val));
                    
                    // Check if this browser window is the foreground or its parent
                    if browser_hwnd == foreground || is_ancestor(browser_hwnd, foreground) {
                        // Get the document (folder view)
                        if let Ok(doc_disp) = browser.Document() {
                            // Cast to IShellFolderViewDual
                            let folder_view: IShellFolderViewDual = doc_disp.cast().ok()?;
                            
                            // Get selected items
                            if let Ok(selected_items) = folder_view.SelectedItems() {
                                let item_count = selected_items.Count().ok()?;
                                if item_count > 0 {
                                    // Get first selected item
                                    let variant_zero = windows::core::VARIANT::from(0i32);
                                    // Item expects &VARIANT - pass reference
                                    if let Ok(item) = selected_items.Item(&variant_zero) {
                                        if let Ok(path) = item.Path() {
                                            return Some(PathBuf::from(path.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

/// Get the selected file on the Desktop
fn get_desktop_selection() -> Option<PathBuf> {
    // Disabled for safety: direct ListView text retrieval across processes can
    // crash Explorer when done incorrectly. We'll implement a safe UIA-based
    // method later. For now return None so Desktop preview won't attempt unsafe reads.
    None
}

/// Check if hwnd1 is an ancestor of hwnd2
fn is_ancestor(hwnd1: HWND, hwnd2: HWND) -> bool {
    unsafe {
        let mut current = hwnd2;
        while !current.0.is_null() {
            if current == hwnd1 {
                return true;
            }
            current = GetParent(current).unwrap_or(HWND::default());
        }
        false
    }
}

/// Show the preview window for a file
fn show_preview(file_path: &Path) -> Result<()> {
    // Determine preview type
    let content = determine_preview_content(file_path)?;
    
    // Register window class if needed
    unsafe {
        register_preview_class()?;
    }

    // Calculate window size based on content
    let (win_width, win_height) = calculate_window_size(&content);

    // Create the preview window
    let hwnd = unsafe {
        let class = to_wide(QUICKLOOK_CLASS);
        let hinstance = GetModuleHandleW(None)?;

        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            win_width,
            win_height,
            None,
            None,
            hinstance,
            None,
        )?
    };

    // Store the window handle
    PREVIEW_HWND_RAW.store(hwnd.0 as isize, Ordering::SeqCst);

    // Center on screen
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - win_width) / 2;
        let y = (screen_h - win_height) / 2;
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, win_width, win_height, SWP_SHOWWINDOW).ok();
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
    }

    // Get file icon
    let file_icon = unsafe { get_file_large_icon(file_path) };

    // Load image data if it's an image
    let image_data = if let PreviewContent::Image(ref path) = content {
        load_image_for_preview(path)
    } else {
        None
    };

    // Store state
    let state = Box::new(QuickLookState {
        file_path: file_path.to_path_buf(),
        content,
        file_icon,
        scroll_offset: 0,
        image_data,
    });
    
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }

    // Start a background poller to update preview when selection changes
    {
        let hwnd_poll = hwnd;
        std::thread::spawn(move || {
            // Poll for selection changes while preview window is open
            loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if PREVIEW_HWND_RAW.load(Ordering::SeqCst) == 0 { break; }

                if let Some(current) = get_selected_file() {
                    // Compare to current preview state
                    if PREVIEW_HWND_RAW.load(Ordering::SeqCst) != 0 {
                        let h = HWND(PREVIEW_HWND_RAW.load(Ordering::SeqCst) as *mut std::ffi::c_void);
                        if h.0.is_null() { break; }

                        if let Some(st) = get_preview_state(h) {
                            if st.file_path != current {
                                // Replace preview content for new selection
                                reload_preview_for_hwnd(h, &current);
                            }
                        }
                    }
                }
            }
        });
    }

    Ok(())
}

/// Replace the preview contents in an existing preview window
fn reload_preview_for_hwnd(hwnd: HWND, file_path: &Path) {
    // Load new content and update the window state in-place
    if let Ok(content) = determine_preview_content(file_path) {
        // Load resources before mutating state so we don't lose previous ones on error
        let new_icon = unsafe { get_file_large_icon(file_path) };
        let new_image = if let PreviewContent::Image(ref p) = content {
            load_image_for_preview(p)
        } else { None };

        if let Some(s) = get_preview_state_mut(hwnd) {
            // Free old resources
            if let Some(icon) = s.file_icon {
                unsafe { let _ = DestroyIcon(icon); }
            }
            if let Some(img) = s.image_data.take() {
                unsafe { let _ = DeleteObject(img.bitmap); }
            }

            // Update state
            s.file_path = file_path.to_path_buf();
            s.content = content;
            s.file_icon = new_icon;
            s.image_data = new_image;
            s.scroll_offset = 0;

            // Request redraw
            unsafe { let _ = InvalidateRect(hwnd, None, false); }
        }
    }
}

/// Determine what kind of preview to show
fn determine_preview_content(file_path: &Path) -> Result<PreviewContent> {
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Image extensions
    let image_exts = ["png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "tiff", "tif"];
    if image_exts.contains(&extension.as_str()) {
        return Ok(PreviewContent::Image(file_path.to_path_buf()));
    }

    // Text extensions
    let text_exts = [
        "txt", "md", "rs", "py", "js", "ts", "jsx", "tsx", "json", "xml", "html", "htm",
        "css", "scss", "less", "yaml", "yml", "toml", "ini", "cfg", "conf", "log",
        "sh", "bash", "zsh", "ps1", "bat", "cmd", "c", "cpp", "h", "hpp", "java",
        "go", "rb", "php", "sql", "gitignore", "dockerignore", "env", "vue", "svelte",
    ];
    
    if text_exts.contains(&extension.as_str()) {
        // Read file content
        let metadata = std::fs::metadata(file_path)?;
        if metadata.len() <= MAX_TEXT_PREVIEW_SIZE {
            let content = std::fs::read_to_string(file_path)
                .unwrap_or_else(|_| "[Unable to read file content]".to_string());
            return Ok(PreviewContent::Text(content));
        } else {
            // File too large, read first part
            let content = std::fs::read_to_string(file_path)
                .map(|s| {
                    let chars: String = s.chars().take(50000).collect();
                    format!("{}\n\n[File truncated - too large to preview]", chars)
                })
                .unwrap_or_else(|_| "[Unable to read file content]".to_string());
            return Ok(PreviewContent::Text(content));
        }
    }

    // Unsupported
    Ok(PreviewContent::Unsupported(extension))
}

/// Calculate window size based on content type
fn calculate_window_size(content: &PreviewContent) -> (i32, i32) {
    match content {
        PreviewContent::Image(_) => (800, 600),
        PreviewContent::Text(_) => (700, 500),
        PreviewContent::Unsupported(_) => (400, 200),
    }
}

/// Register the preview window class
unsafe fn register_preview_class() -> Result<()> {
    let class_name = to_wide(QUICKLOOK_CLASS);
    let hinstance = GetModuleHandleW(None)?;

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
        lpfnWndProc: Some(preview_wnd_proc),
        hInstance: hinstance.into(),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hbrBackground: HBRUSH::default(),
        ..Default::default()
    };

    let _ = RegisterClassExW(&wc);
    Ok(())
}

/// Get large icon for a file
unsafe fn get_file_large_icon(path: &Path) -> Option<HICON> {
    let wide_path: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    let mut shfi = SHFILEINFOW::default();

    let result = SHGetFileInfoW(
        PCWSTR(wide_path.as_ptr()),
        windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
        Some(&mut shfi),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_LARGEICON,
    );

    if result != 0 && !shfi.hIcon.is_invalid() {
        Some(shfi.hIcon)
    } else {
        None
    }
}

/// Load image for preview using the `image` crate and create an HBITMAP
fn load_image_for_preview(path: &Path) -> Option<ImageData> {
    // Decode with the image crate (supports PNG/JPEG/GIF/WebP/TIFF/etc.)
    match image::open(path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = (rgba.width() as i32, rgba.height() as i32);

            // Prepare BITMAPINFO for a 32bpp BGRA DIB
            unsafe {
                use windows::Win32::Graphics::Gdi::*;

                let mut bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        biHeight: -height, // top-down
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0 as u32,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [RGBQUAD::default(); 1],
                };

                let hdc = GetDC(HWND::default());
                let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
                let hbitmap_res = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits as *mut _ as *mut _, None, 0);
                let _ = ReleaseDC(HWND::default(), hdc);

                let hbitmap = match hbitmap_res {
                    Ok(b) => b,
                    Err(_) => return None,
                };

                if hbitmap.0.is_null() || bits.is_null() {
                    let _ = DeleteObject(hbitmap);
                    return None;
                }

                // Copy pixels converting from RGBA to BGRA
                let src = rgba.into_raw(); // RGBA
                let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize);
                for i in 0..(width * height) as usize {
                    let si = i * 4;
                    let di = si;
                    // RGBA -> BGRA
                    dst[di + 0] = src[si + 2]; // B
                    dst[di + 1] = src[si + 1]; // G
                    dst[di + 2] = src[si + 0]; // R
                    dst[di + 3] = src[si + 3]; // A
                }

                return Some(ImageData { width, height, bitmap: hbitmap });
            }
        }
        Err(_) => None,
    }
}

/// Preview window procedure
unsafe extern "system" fn preview_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Some(state) = get_preview_state(hwnd) {
                paint_preview(hdc, hwnd, state);
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            match vk {
                0x1B => { // Escape
                    close_preview_window();
                }
                0x20 => { // Space - close preview
                    close_preview_window();
                }
                0x26 => { // Up arrow - scroll up
                    if let Some(state) = get_preview_state_mut(hwnd) {
                        state.scroll_offset = (state.scroll_offset - 30).max(0);
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
                0x28 => { // Down arrow - scroll down
                    if let Some(state) = get_preview_state_mut(hwnd) {
                        state.scroll_offset += 30;
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
                0x0D => { // Enter - open the file
                    if let Some(state) = get_preview_state(hwnd) {
                        let path = state.file_path.clone();
                        close_preview_window();
                        open_file(&path);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            if let Some(state) = get_preview_state_mut(hwnd) {
                if delta > 0 {
                    state.scroll_offset = (state.scroll_offset - 60).max(0);
                } else {
                    state.scroll_offset += 60;
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }

        WM_LBUTTONDBLCLK => {
            // Double-click to open file
            if let Some(state) = get_preview_state(hwnd) {
                let path = state.file_path.clone();
                close_preview_window();
                open_file(&path);
            }
            LRESULT(0)
        }

        WM_KILLFOCUS => {
            // Close when losing focus
            close_preview_window();
            LRESULT(0)
        }

        WM_DESTROY => {
            free_preview_state(hwnd);
            PREVIEW_HWND_RAW.store(0, Ordering::SeqCst);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paint the preview content
unsafe fn paint_preview(hdc: HDC, hwnd: HWND, state: &QuickLookState) {
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    // Get theme colors
    let (bg_color, text_color, accent_color) = if let Some(gs) = get_window_state() {
        let theme = gs.read().theme_manager.theme().clone();
        (
            if theme.is_dark { Color::rgb(30, 30, 32) } else { Color::rgb(250, 250, 252) },
            if theme.is_dark { Color::rgb(240, 240, 242) } else { Color::rgb(30, 30, 32) },
            theme.accent,
        )
    } else {
        (Color::rgb(30, 30, 32), Color::rgb(240, 240, 242), Color::rgb(0, 120, 212))
    };

    // Draw background
    let bg_brush = CreateSolidBrush(bg_color.colorref());
    FillRect(hdc, &rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    // Draw border
    let border_brush = CreateSolidBrush(accent_color.colorref());
    let border_rect = RECT { left: 0, top: 0, right: width, bottom: 3 };
    FillRect(hdc, &border_rect, border_brush);
    let _ = DeleteObject(border_brush);

    SetBkMode(hdc, TRANSPARENT);

    // Header area with file info
    let header_height = 50;
    let header_bg = CreateSolidBrush(Color::rgb(40, 40, 44).colorref());
    let header_rect = RECT { left: 0, top: 3, right: width, bottom: header_height };
    FillRect(hdc, &header_rect, header_bg);
    let _ = DeleteObject(header_bg);

    // Draw file icon
    if let Some(icon) = state.file_icon {
        let _ = DrawIconEx(hdc, 12, 10, icon, 32, 32, 0, None, DI_NORMAL);
    }

    // Draw filename
    let filename = state.file_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");
    
    let title_font = CreateFontW(
        18, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0,
        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
        PCWSTR(to_wide("Segoe UI").as_ptr()),
    );
    let old_font = SelectObject(hdc, title_font);
    SetTextColor(hdc, Color::rgb(255, 255, 255).colorref());
    
    let title_wide: Vec<u16> = filename.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = TextOutW(hdc, 52, 8, &title_wide[..title_wide.len() - 1]);
    
    // Draw file path (smaller)
    let path_str = state.file_path.parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    
    let path_font = CreateFontW(
        12, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
        PCWSTR(to_wide("Segoe UI").as_ptr()),
    );
    let _ = SelectObject(hdc, path_font);
    SetTextColor(hdc, Color::rgb(160, 160, 165).colorref());
    
    let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = TextOutW(hdc, 52, 28, &path_wide[..path_wide.len() - 1]);

    let _ = SelectObject(hdc, old_font);
    let _ = DeleteObject(title_font);
    let _ = DeleteObject(path_font);

    // Content area
    let content_top = header_height + 10;
    let content_rect = RECT {
        left: 16,
        top: content_top,
        right: width - 16,
        bottom: height - 40,
    };

    match &state.content {
        PreviewContent::Image(_) => {
            paint_image_preview(hdc, &content_rect, state);
        }
        PreviewContent::Text(text) => {
            paint_text_preview(hdc, &content_rect, text, state.scroll_offset, text_color);
        }
        PreviewContent::Unsupported(ext) => {
            paint_unsupported(hdc, &content_rect, ext, text_color);
        }
    }

    // Footer with hints
    let footer_font = CreateFontW(
        11, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
        PCWSTR(to_wide("Segoe UI").as_ptr()),
    );
    let _ = SelectObject(hdc, footer_font);
    SetTextColor(hdc, Color::rgb(120, 120, 125).colorref());
    
    let hint = "Press Space/Esc to close • Enter to open • Scroll to navigate";
    let hint_wide: Vec<u16> = hint.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = TextOutW(hdc, 16, height - 24, &hint_wide[..hint_wide.len() - 1]);
    
    let _ = DeleteObject(footer_font);
}

/// Paint image preview
unsafe fn paint_image_preview(hdc: HDC, rect: &RECT, state: &QuickLookState) {
    if let Some(ref img) = state.image_data {
        let content_width = rect.right - rect.left;
        let content_height = rect.bottom - rect.top;

        // Calculate scaled dimensions to fit
        let scale_x = content_width as f64 / img.width as f64;
        let scale_y = content_height as f64 / img.height as f64;
        let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

        let draw_width = (img.width as f64 * scale) as i32;
        let draw_height = (img.height as f64 * scale) as i32;

        // Center the image
        let x = rect.left + (content_width - draw_width) / 2;
        let y = rect.top + (content_height - draw_height) / 2;

        // Create compatible DC and select bitmap
        let mem_dc = CreateCompatibleDC(hdc);
        let old_bmp = SelectObject(mem_dc, img.bitmap);

        // Draw scaled
        let _ = SetStretchBltMode(hdc, HALFTONE);
        let _ = StretchBlt(
            hdc,
            x, y, draw_width, draw_height,
            mem_dc,
            0, 0, img.width, img.height,
            SRCCOPY,
        );

        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteDC(mem_dc);
    } else {
        // Image couldn't be loaded
        let font = CreateFontW(
            14, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
            DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
            PCWSTR(to_wide("Segoe UI").as_ptr()),
        );
        let old_font = SelectObject(hdc, font);
        SetTextColor(hdc, Color::rgb(180, 180, 185).colorref());
        
        let msg = "Unable to load image preview";
        let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let center_x = rect.left + (rect.right - rect.left) / 2 - 80;
        let center_y = rect.top + (rect.bottom - rect.top) / 2;
        let _ = TextOutW(hdc, center_x, center_y, &msg_wide[..msg_wide.len() - 1]);
        
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(font);
    }
}

/// Paint text preview
unsafe fn paint_text_preview(hdc: HDC, rect: &RECT, text: &str, scroll_offset: i32, text_color: Color) {
    let font = CreateFontW(
        13, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
        PCWSTR(to_wide("Consolas").as_ptr()),
    );
    let old_font = SelectObject(hdc, font);
    SetTextColor(hdc, text_color.colorref());

    let line_height = 16;
    let mut y = rect.top - scroll_offset;
    
    // Set clipping region
    let clip_rgn = CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom);
    SelectClipRgn(hdc, clip_rgn);

    for line in text.lines() {
        if y + line_height > rect.top && y < rect.bottom {
            let line_wide: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = TextOutW(hdc, rect.left, y, &line_wide[..line_wide.len() - 1]);
        }
        y += line_height;
        
        // Stop if we're way past the visible area
        if y > rect.bottom + 500 {
            break;
        }
    }

    // Reset clipping
    SelectClipRgn(hdc, None);
    let _ = DeleteObject(clip_rgn);

    let _ = SelectObject(hdc, old_font);
    let _ = DeleteObject(font);
}

/// Paint unsupported file type message
unsafe fn paint_unsupported(hdc: HDC, rect: &RECT, ext: &str, text_color: Color) {
    let font = CreateFontW(
        14, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
        DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY.0 as u32, 0,
        PCWSTR(to_wide("Segoe UI").as_ptr()),
    );
    let old_font = SelectObject(hdc, font);
    SetTextColor(hdc, text_color.colorref());

    let msg = if ext.is_empty() {
        "Preview not available for this file type".to_string()
    } else {
        format!("Preview not available for .{} files", ext)
    };
    
    let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    let center_x = rect.left + (rect.right - rect.left) / 2 - 100;
    let center_y = rect.top + (rect.bottom - rect.top) / 2;
    let _ = TextOutW(hdc, center_x, center_y, &msg_wide[..msg_wide.len() - 1]);

    let hint = "Press Enter to open with default application";
    let hint_wide: Vec<u16> = hint.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = TextOutW(hdc, center_x - 20, center_y + 24, &hint_wide[..hint_wide.len() - 1]);

    let _ = SelectObject(hdc, old_font);
    let _ = DeleteObject(font);
}

/// Open file with default application
fn open_file(path: &Path) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide_path: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(wide_path.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

/// Close the preview window
fn close_preview_window() {
    let hwnd_raw = PREVIEW_HWND_RAW.swap(0, Ordering::SeqCst);
    if hwnd_raw != 0 {
        unsafe {
            let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
            let _ = DestroyWindow(hwnd);
        }
    }
}

fn get_preview_state(hwnd: HWND) -> Option<&'static QuickLookState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut QuickLookState;
        if ptr.is_null() { None } else { Some(&*ptr) }
    }
}

fn get_preview_state_mut(hwnd: HWND) -> Option<&'static mut QuickLookState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut QuickLookState;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

fn free_preview_state(hwnd: HWND) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut QuickLookState;
        if !ptr.is_null() {
            let state = Box::from_raw(ptr);
            // Clean up icon
            if let Some(icon) = state.file_icon {
                let _ = DestroyIcon(icon);
            }
            // Clean up bitmap
            if let Some(img) = state.image_data {
                let _ = DeleteObject(img.bitmap);
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Check if QuickLook is enabled
#[allow(dead_code)]
pub fn is_quicklook_running() -> bool {
    QUICKLOOK_RUNNING.load(Ordering::SeqCst)
}
