//! Window management for the TopBar application
//! 
//! Handles window creation, positioning, and Windows API interactions.

#![allow(dead_code)]

use anyhow::Result;
use log::{debug, info, warn};
use std::sync::Arc;
use parking_lot::RwLock;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWM_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWA_SYSTEMBACKDROP_TYPE,
    DWM_SYSTEMBACKDROP_TYPE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{TRACKMOUSEEVENT, TME_LEAVE, TrackMouseEvent, SetCapture, ReleaseCapture};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::core::w;

use crate::config::{Config, BarPosition};
use crate::render::Renderer;
use crate::theme::{Theme, ThemeManager};
use crate::utils::{to_wide_string, to_pcwstr, Rect, get_screen_size, scale_by_dpi};

/// Window class name
const WINDOW_CLASS: &str = "TopBarWindowClass";
const WINDOW_TITLE: &str = "TopBar";

/// WM_MOUSELEAVE message constant
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Custom window messages
pub const WM_TOPBAR_UPDATE: u32 = WM_USER + 1;
pub const WM_TOPBAR_THEME_CHANGED: u32 = WM_USER + 2;
pub const WM_TOPBAR_TRAY: u32 = WM_USER + 3;
pub const WM_TOPBAR_MODULE_CLICK: u32 = WM_USER + 4;

/// Window state for storing data accessible from window proc (thread-safe parts only)
pub struct WindowState {
    pub config: Arc<Config>,
    pub theme_manager: ThemeManager,
    pub bar_rect: Rect,
    pub dpi: u32,
    pub is_visible: bool,
    pub is_hovered: bool,
    pub hover_module: Option<String>,
    pub active_menu: Option<String>,
    pub needs_redraw: bool,
    pub clicked_module: Option<String>,

    // Drag-and-drop state for rearranging modules
    pub clicked_pos: Option<(i32, i32)>,
    pub dragging_module: Option<String>,
    pub drag_start_x: i32,
    pub drag_current_x: i32,
    pub drag_origin_side: Option<String>, // "left" or "right"
    pub drag_orig_index: Option<usize>,
}

impl WindowState {
    pub fn new(config: Arc<Config>) -> Self {
        let theme_manager = ThemeManager::new(config.appearance.theme_mode);
        
        Self {
            config,
            theme_manager,
            bar_rect: Rect::default(),
            dpi: 96,
            is_visible: true,
            is_hovered: false,
            hover_module: None,
            active_menu: None,
            needs_redraw: true,
            clicked_module: None,

            // Drag state defaults
            clicked_pos: None,
            dragging_module: None,
            drag_start_x: 0,
            drag_current_x: 0,
            drag_origin_side: None,
            drag_orig_index: None,
        }
    }
}

// Global window state (thread-safe)
static WINDOW_STATE: once_cell::sync::OnceCell<Arc<RwLock<WindowState>>> = once_cell::sync::OnceCell::new();

// Thread-local storage for the renderer (contains non-Send HWND)
thread_local! {
    static RENDERER: std::cell::RefCell<Option<Renderer>> = const { std::cell::RefCell::new(None) };
}

/// Set the renderer
fn set_renderer(renderer: Renderer) {
    RENDERER.with(|r| {
        *r.borrow_mut() = Some(renderer);
    });
}

/// Access the renderer
fn with_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Renderer) -> R,
{
    RENDERER.with(|r| {
        r.borrow_mut().as_mut().map(f)
    })
}

/// Get the global window state
pub fn get_window_state() -> Option<Arc<RwLock<WindowState>>> {
    WINDOW_STATE.get().cloned()
}

/// Main window manager
pub struct WindowManager {
    hwnd: HWND,
    state: Arc<RwLock<WindowState>>,
}

impl WindowManager {
    /// Create a new window manager and topbar window
    pub fn new(config: Arc<Config>) -> Result<Self> {
        // Set DPI awareness
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        // Create window state
        let state = Arc::new(RwLock::new(WindowState::new(config.clone())));
        let _ = WINDOW_STATE.set(state.clone());

        // Register window class
        let class_name = to_wide_string(WINDOW_CLASS);
        Self::register_window_class(&class_name)?;

        // Create the window
        let hwnd = Self::create_window(&class_name, &config)?;

        // Get DPI
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        {
            let mut state_guard = state.write();
            state_guard.dpi = dpi;
        }

        // Apply window styling
        Self::apply_window_style(hwnd, &state.read().theme_manager.theme())?;

        // Calculate and set position
        let bar_rect = Self::calculate_bar_rect(&config, dpi);
        {
            let mut state_guard = state.write();
            state_guard.bar_rect = bar_rect;
        }
        
        Self::position_window(hwnd, &bar_rect, &config)?;

        // Initialize renderer (stored in thread-local)
        set_renderer(Renderer::new(hwnd, dpi)?);

        info!("Window created successfully at {:?}", bar_rect);

        Ok(Self { hwnd, state })
    }

    /// Register the window class
    fn register_window_class(class_name: &[u16]) -> Result<()> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(window_proc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: to_pcwstr(class_name),
                hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
                ..Default::default()
            };

            let atom = RegisterClassExW(&wc);
            if atom == 0 {
                return Err(anyhow::anyhow!("Failed to register window class"));
            }
        }
        Ok(())
    }

    /// Create the topbar window
    fn create_window(class_name: &[u16], config: &Config) -> Result<HWND> {
        let title = to_wide_string(WINDOW_TITLE);
        
        // Extended style for topmost, layered, tool window
        let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE;
        
        // Window style - popup with no border
        let style = WS_POPUP;

        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            
            let hwnd = CreateWindowExW(
                ex_style,
                to_pcwstr(class_name),
                to_pcwstr(&title),
                style,
                0, 0, 100, 100,  // Temporary size, will be set later
                None,
                None,
                hinstance,
                None,
            )?;

            if hwnd.0 == std::ptr::null_mut() {
                return Err(anyhow::anyhow!("Failed to create window"));
            }

            // Set layered window attributes for transparency
            let opacity = (config.appearance.opacity * 255.0) as u8;
            SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), opacity, LWA_ALPHA)?;

            Ok(hwnd)
        }
    }

    /// Apply Windows 11 styling to the window
    fn apply_window_style(hwnd: HWND, theme: &Theme) -> Result<()> {
        unsafe {
            // Enable dark mode title bar if using dark theme
            let use_dark_mode: i32 = if theme.is_dark { 1 } else { 0 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &use_dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            // Set rounded corners (Windows 11)
            let corner_preference = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_preference as *const _ as *const _,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );

            // Try to enable Mica/Acrylic backdrop (Windows 11 22H2+)
            // 2 = Mica, 3 = Acrylic, 4 = Mica Alt
            let backdrop_type: i32 = 3;  // Acrylic
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop_type as *const _ as *const _,
                std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
            );
        }
        Ok(())
    }

    /// Calculate the bar rectangle based on config and screen
    fn calculate_bar_rect(config: &Config, dpi: u32) -> Rect {
        let screen = get_screen_size();
        let height = scale_by_dpi(config.appearance.bar_height as i32, dpi);
        
        let y = match config.appearance.position {
            BarPosition::Top => 0,
            BarPosition::Bottom => screen.height - height,
        };

        Rect {
            x: 0,
            y,
            width: screen.width,
            height,
        }
    }

    /// Position the window
    fn position_window(hwnd: HWND, rect: &Rect, config: &Config) -> Result<()> {
        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;

            // Reserve screen space if configured
            if config.behavior.reserve_space {
                Self::reserve_screen_space(hwnd, rect, config)?;
            }
        }
        Ok(())
    }

    /// Reserve screen space (like a taskbar)
    fn reserve_screen_space(hwnd: HWND, rect: &Rect, config: &Config) -> Result<()> {
        use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_NEW, ABM_QUERYPOS, ABM_SETPOS, APPBARDATA, ABE_TOP, ABE_BOTTOM};

        unsafe {
            let mut abd = APPBARDATA {
                cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                hWnd: hwnd,
                uCallbackMessage: 0,
                uEdge: match config.appearance.position {
                    BarPosition::Top => ABE_TOP,
                    BarPosition::Bottom => ABE_BOTTOM,
                },
                rc: RECT {
                    left: rect.x,
                    top: rect.y,
                    right: rect.right(),
                    bottom: rect.bottom(),
                },
                lParam: LPARAM(0),
            };

            // Register as an AppBar with the Shell
            SHAppBarMessage(ABM_NEW, &mut abd);
            // Let the shell adjust the requested rectangle to avoid overlaps
            SHAppBarMessage(ABM_QUERYPOS, &mut abd);
            // Apply the final position and reserve the space
            SHAppBarMessage(ABM_SETPOS, &mut abd);
        }
        Ok(())
    }

    /// Remove any AppBar reservation for this window (called on destroy)
    fn remove_screen_space(hwnd: HWND) {
        use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_REMOVE, APPBARDATA};

        unsafe {
            let mut abd = APPBARDATA {
                cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                hWnd: hwnd,
                uCallbackMessage: 0,
                uEdge: 0,
                rc: RECT { left: 0, top: 0, right: 0, bottom: 0 },
                lParam: LPARAM(0),
            };

            let _ = SHAppBarMessage(ABM_REMOVE, &mut abd);
        }
    }

    /// Show the window
    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            self.state.write().is_visible = true;
        }

        // If configured, register/reserve the screen space when showing
        let state_guard = self.state.read();
        if state_guard.config.behavior.reserve_space {
            let rect = state_guard.bar_rect;
            let cfg = state_guard.config.clone();
            drop(state_guard);
            let _ = Self::reserve_screen_space(self.hwnd, &rect, &cfg);
        }
    }

    /// Hide the window
    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            self.state.write().is_visible = false;
        }

        // If configured, remove the reserved space so other apps can use full screen
        let state_guard = self.state.read();
        if state_guard.config.behavior.reserve_space {
            drop(state_guard);
            Self::remove_screen_space(self.hwnd);
        }
    }

    /// Toggle window visibility
    pub fn toggle_visibility(&self) {
        let is_visible = self.state.read().is_visible;
        if is_visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Request a redraw
    pub fn request_redraw(&self) {
        self.state.write().needs_redraw = true;
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, false);
        }
    }

    /// Update theme
    pub fn update_theme(&self) {
        let mut state = self.state.write();
        state.theme_manager.check_system_theme();
        let theme = state.theme_manager.theme();
        let _ = Self::apply_window_style(self.hwnd, theme);
        state.needs_redraw = true;
        drop(state);
        
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, true);
        }
    }

    /// Get window handle
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Get window state
    pub fn state(&self) -> Arc<RwLock<WindowState>> {
        self.state.clone()
    }

    /// Run the message loop
    pub fn run_message_loop(&self) -> Result<()> {
        unsafe {
            let mut msg = MSG::default();
            
            // Create timer for periodic updates
            SetTimer(self.hwnd, 1, 1000, None);  // 1 second timer for clock
            SetTimer(self.hwnd, 2, 2000, None);  // 2 second timer for system info
            SetTimer(self.hwnd, 3, 100, None);   // 100ms timer for animations

            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }
}

impl Drop for WindowManager {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Window procedure for handling Windows messages
unsafe extern "system" fn window_proc(
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
                    if let (Some(click_id), Some((cx, _cy))) = (state_guard.clicked_module.clone(), state_guard.clicked_pos) {
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
                    if let Some(idx) = cfg.modules.left_modules.iter().position(|m| m == &module_id) {
                        s.drag_origin_side = Some("left".to_string());
                        s.drag_orig_index = Some(idx);
                    } else if let Some(idx) = cfg.modules.right_modules.iter().position(|m| m == &module_id) {
                        s.drag_origin_side = Some("right".to_string());
                        s.drag_orig_index = Some(idx);
                    } else {
                        s.drag_origin_side = None;
                        s.drag_orig_index = None;
                    }
                }

                // Capture mouse so we receive move/up events
                unsafe { let _ = SetCapture(hwnd); }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Some(state) = get_window_state() {
                let mut s = state.write();

                // If a drag was in progress, finalize reorder
                if let Some(drag_id) = s.dragging_module.clone() {
                    // Use renderer bounds to determine insertion point
                    with_renderer(|renderer| {
                        let bounds = renderer.module_bounds().clone();

                        // Determine visual order for the origin side
                        let (visual_list, mut target_vec) = if let Some(side) = &s.drag_origin_side {
                            if side == "left" {
                                (s.config.modules.left_modules.clone(), "left")
                            } else {
                                (s.config.modules.right_modules.clone(), "right")
                            }
                        } else {
                            (vec![], "left")
                        };

                        // Build visual vector of (id, rect) in left-to-right order
                        let mut visual: Vec<(String, crate::utils::Rect)> = Vec::new();
                        for id in visual_list.iter() {
                            if let Some(r) = bounds.get(id) {
                                visual.push((id.clone(), r.clone()));
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
                            if final_idx > pos { final_idx = final_idx.saturating_sub(1); }
                            vec_ref.insert(final_idx, drag_id.clone());
                        }

                        // Save and apply config
                        if let Err(e) = new_cfg.save() {
                            warn!("Failed to save config after reorder: {}", e);
                        } else {
                            s.config = Arc::new(new_cfg);
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
                    unsafe { let _ = InvalidateRect(hwnd, None, false); }
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
                unsafe { let _ = ReleaseCapture(); }
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
                state_guard.bar_rect = WindowManager::calculate_bar_rect(&config, dpi);
                
                let rect = state_guard.bar_rect;
                drop(state_guard);
                
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    rect.x, rect.y, rect.width, rect.height,
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
                state_guard.bar_rect = WindowManager::calculate_bar_rect(&config, new_dpi);
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
                    let _ = WindowManager::apply_window_style(hwnd, &theme);
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
                                if let Some(bm) = module.as_any_mut().downcast_mut::<crate::modules::bluetooth::BluetoothModule>() {
                                    bm.refresh();
                                }
                            }
                        });

                        // Request a redraw to update the UI
                        unsafe { let _ = InvalidateRect(hwnd, None, false); }
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
                let _ = WindowManager::apply_window_style(hwnd, &theme);
                let _ = InvalidateRect(hwnd, None, true);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            info!("Window destroyed, quitting application");
            WindowManager::remove_screen_space(hwnd);
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

// Menu item IDs
const MENU_SHOW_CLOCK: u32 = 1001;
const MENU_SHOW_BATTERY: u32 = 1002;
const MENU_SHOW_VOLUME: u32 = 1003;
const MENU_SHOW_NETWORK: u32 = 1004;
const MENU_SHOW_SYSINFO: u32 = 1005;
const MENU_SHOW_MEDIA: u32 = 1006;
const MENU_SHOW_GPU: u32 = 1007;
const MENU_SHOW_KEYBOARD: u32 = 1008;
const MENU_SHOW_UPTIME: u32 = 1009;
const MENU_SHOW_BLUETOOTH: u32 = 1010;
const MENU_SHOW_DISK: u32 = 1011;
const MENU_SHOW_CLIPBOARD: u32 = 1012;
const MENU_SHOW_WEATHER: u32 = 1013;

// GPU menu items
const GPU_SHOW_USAGE: u32 = 2601;
const GPU_SHOW_MEMORY: u32 = 2602;
const GPU_SHOW_TEMP: u32 = 2603;
const GPU_SHOW_GRAPH: u32 = 2604;
const MENU_SETTINGS: u32 = 1200;
const MENU_RELOAD: u32 = 1201;
const MENU_EXIT: u32 = 1999;

/// Show the context menu
fn show_context_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() {
            return;
        }

        // Get current config to show checkmarks
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        let right_modules = &config.modules.right_modules;
        let center_modules = &config.modules.center_modules;

        // Module toggles with checkmarks
        append_menu_item(menu, MENU_SHOW_CLOCK, "Clock", right_modules.contains(&"clock".to_string()) || (config.modules.clock.center && center_modules.contains(&"clock".to_string())));
        append_menu_item(menu, MENU_SHOW_BATTERY, "Battery", right_modules.contains(&"battery".to_string()));
        append_menu_item(menu, MENU_SHOW_VOLUME, "Volume", right_modules.contains(&"volume".to_string()));
        append_menu_item(menu, MENU_SHOW_NETWORK, "Network", right_modules.contains(&"network".to_string()));
        append_menu_item(menu, MENU_SHOW_SYSINFO, "System Info", right_modules.contains(&"system_info".to_string()));
        append_menu_item(menu, MENU_SHOW_MEDIA, "Media Controls", right_modules.contains(&"media".to_string()));
        append_menu_item(menu, MENU_SHOW_CLIPBOARD, "Clipboard", right_modules.contains(&"clipboard".to_string()));
        append_menu_item(menu, MENU_SHOW_GPU, "GPU Usage", right_modules.contains(&"gpu".to_string()));
        append_menu_item(menu, MENU_SHOW_KEYBOARD, "Keyboard Layout", right_modules.contains(&"keyboard_layout".to_string()));
        append_menu_item(menu, MENU_SHOW_UPTIME, "System Uptime", right_modules.contains(&"uptime".to_string()));
        append_menu_item(menu, MENU_SHOW_BLUETOOTH, "Bluetooth", right_modules.contains(&"bluetooth".to_string()));
        append_menu_item(menu, MENU_SHOW_DISK, "Disk Usage", right_modules.contains(&"disk".to_string()));
        append_menu_item(menu, MENU_SHOW_WEATHER, "Weather", right_modules.contains(&"weather".to_string()));
        
        // Separator
        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        
        // Settings and exit
        append_menu_item(menu, MENU_SETTINGS, "Open Config File", false);
        append_menu_item(menu, MENU_RELOAD, "Reload Config", false);
        
        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        append_menu_item(menu, MENU_EXIT, "Exit TopBar", false);

        // Need to set foreground for menu to work properly
        let _ = SetForegroundWindow(hwnd);
        
        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            x, y,
            0,
            hwnd,
            None,
        );
        
        DestroyMenu(menu).ok();
        
        info!("Context menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

/// Helper to append a menu item
fn append_menu_item(menu: HMENU, id: u32, text: &str, checked: bool) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let flags = if checked { MF_STRING | MF_CHECKED } else { MF_STRING };
        AppendMenuW(menu, flags, id as usize, PCWSTR(wide.as_ptr())).ok();
    }
}

/// Handle menu commands
fn handle_menu_command(hwnd: HWND, cmd_id: u32) {
    match cmd_id {
        // Main context menu
        MENU_SHOW_CLOCK => toggle_module(hwnd, "clock"),
        MENU_SHOW_BATTERY => toggle_module(hwnd, "battery"),
        MENU_SHOW_VOLUME => toggle_module(hwnd, "volume"),
        MENU_SHOW_NETWORK => toggle_module(hwnd, "network"),
        MENU_SHOW_SYSINFO => toggle_module(hwnd, "system_info"),
        MENU_SHOW_MEDIA => toggle_module(hwnd, "media"),
        MENU_SHOW_CLIPBOARD => toggle_module(hwnd, "clipboard"),
        MENU_SHOW_GPU => toggle_module(hwnd, "gpu"),
        MENU_SHOW_KEYBOARD => toggle_module(hwnd, "keyboard_layout"),
        MENU_SHOW_UPTIME => toggle_module(hwnd, "uptime"),
        MENU_SHOW_BLUETOOTH => toggle_module(hwnd, "bluetooth"),
        MENU_SHOW_DISK => toggle_module(hwnd, "disk"),
        MENU_SHOW_WEATHER => toggle_module(hwnd, "weather"),
        MENU_SETTINGS => open_config_file(),
        MENU_RELOAD => reload_config(hwnd),
        MENU_EXIT => {
            unsafe {
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        
        // Clock settings
        CLOCK_24H => toggle_config_bool(hwnd, |c| &mut c.modules.clock.format_24h),
        CLOCK_SECONDS => toggle_config_bool(hwnd, |c| &mut c.modules.clock.show_seconds),
        CLOCK_DATE => toggle_config_bool(hwnd, |c| &mut c.modules.clock.show_date),
        CLOCK_DAY => toggle_config_bool(hwnd, |c| &mut c.modules.clock.show_day),
        
        // Battery settings
        BAT_SHOW_PCT => toggle_config_bool(hwnd, |c| &mut c.modules.battery.show_percentage),
        BAT_SHOW_TIME => toggle_config_bool(hwnd, |c| &mut c.modules.battery.show_time_remaining),
        
        // Volume settings
        VOL_SHOW_PCT => toggle_config_bool(hwnd, |c| &mut c.modules.volume.show_percentage),
        VOL_MUTE => {
            with_renderer(|renderer| {
                if let Some(module) = renderer.module_registry.get_mut("volume") {
                    // Cast to VolumeModule to access toggle_mute
                    if let Some(volume_module) = module.as_any_mut().downcast_mut::<crate::modules::volume::VolumeModule>() {
                        volume_module.toggle_mute();
                    }
                }
            });
        }
        
        // Network settings
        NET_SHOW_NAME => toggle_config_bool(hwnd, |c| &mut c.modules.network.show_name),
        NET_SHOW_SPEED => toggle_config_bool(hwnd, |c| &mut c.modules.network.show_speed),
        
        // System info settings
        SYSINFO_CPU => toggle_config_bool(hwnd, |c| &mut c.modules.system_info.show_cpu),
        SYSINFO_MEM => toggle_config_bool(hwnd, |c| &mut c.modules.system_info.show_memory),
        SYSINFO_SHOW_GRAPH => toggle_config_bool(hwnd, |c| &mut c.modules.system_info.show_graph),
        
        // GPU settings
        GPU_SHOW_USAGE => toggle_config_bool(hwnd, |c| &mut c.modules.gpu.show_usage),
        GPU_SHOW_MEMORY => toggle_config_bool(hwnd, |c| &mut c.modules.gpu.show_memory),
        GPU_SHOW_TEMP => toggle_config_bool(hwnd, |c| &mut c.modules.gpu.show_temperature),
        GPU_SHOW_GRAPH => toggle_config_bool(hwnd, |c| &mut c.modules.gpu.show_graph),
        
        // Keyboard layout settings
        KEYBOARD_SHOW_FULL => toggle_config_bool(hwnd, |c| &mut c.modules.keyboard_layout.show_full_name),
        KEYBOARD_AUTO_SWITCH => toggle_config_bool(hwnd, |c| &mut c.modules.keyboard_layout.auto_switch),
        
        // Uptime settings
        UPTIME_SHOW_DAYS => toggle_config_bool(hwnd, |c| &mut c.modules.uptime.show_days),
        UPTIME_COMPACT => toggle_config_bool(hwnd, |c| &mut c.modules.uptime.compact_format),
        
        // Bluetooth settings
        BLUETOOTH_ENABLED => toggle_config_bool(hwnd, |c| &mut c.modules.bluetooth.enabled),
        BLUETOOTH_SHOW_COUNT => toggle_config_bool(hwnd, |c| &mut c.modules.bluetooth.show_device_count),
        
        // Disk settings
        DISK_SHOW_PERCENTAGE => toggle_config_bool(hwnd, |c| &mut c.modules.disk.show_percentage),
        DISK_SHOW_ACTIVITY => toggle_config_bool(hwnd, |c| &mut c.modules.disk.show_activity),

        // Center clock toggle (moves between right and center sections)
        CLOCK_CENTER => {
            if let Some(state) = get_window_state() {
                let config = state.read().config.clone();
                let mut new_config = (*config).clone();
                if new_config.modules.center_modules.iter().any(|m| m == "clock") {
                    // Remove from center, add back to right at default position
                    new_config.modules.center_modules.retain(|m| m != "clock");
                    // Ensure the boolean reflects removal from center
                    new_config.modules.clock.center = false;

                    if !new_config.modules.right_modules.iter().any(|m| m == "clock") {
                        let default_order = vec!["media", "keyboard_layout", "gpu", "system_info", "disk", "network", "bluetooth", "volume", "battery", "uptime", "clock"];
                        let insert_pos = default_order.iter()
                            .position(|&m| m == "clock")
                            .map(|target_idx| {
                                new_config.modules.right_modules.iter()
                                    .position(|m| {
                                        default_order.iter()
                                            .position(|&dm| dm == m.as_str())
                                            .map(|existing_idx| existing_idx > target_idx)
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(new_config.modules.right_modules.len())
                            })
                            .unwrap_or(new_config.modules.right_modules.len());
                        new_config.modules.right_modules.insert(insert_pos, "clock".to_string());
                    }
                } else {
                    // Add to center and remove from right
                    new_config.modules.center_modules.push("clock".to_string());
                    new_config.modules.right_modules.retain(|m| m != "clock");
                    // Ensure the boolean reflects that clock is centered
                    new_config.modules.clock.center = true;
                }

                if let Err(e) = new_config.save() {
                    warn!("Failed to save config: {}", e);
                }
                state.write().config = Arc::new(new_config);
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
            }
        },

        // Disk dynamic selection range
        cmd if (cmd >= DISK_SELECT_BASE && cmd < DISK_SELECT_BASE + 100) => {
            let idx = (cmd - DISK_SELECT_BASE) as usize;
            if let Some(state) = get_window_state() {
                // Get disks from renderer
                let mut selected_mount: Option<String> = None;
                with_renderer(|renderer| {
                    if let Some(module) = renderer.module_registry.get("disk") {
                        if let Some(dm) = module.as_any().downcast_ref::<crate::modules::disk::DiskModule>() {
                            if idx < dm.get_disks().len() {
                                let d = &dm.get_disks()[idx];
                                selected_mount = Some(d.mount_point.clone());
                            }
                        }
                    }
                });

                if let Some(mount) = selected_mount {
                    let config = state.read().config.clone();
                    let mut new_config = (*config).clone();
                    new_config.modules.disk.primary_disk = mount;
                    if let Err(e) = new_config.save() {
                        warn!("Failed to save config: {}", e);
                    }
                    state.write().config = Arc::new(new_config);
                    unsafe { let _ = InvalidateRect(hwnd, None, true); }
                }
            }
        },

        // Clipboard history selection range
        cmd if (cmd >= CLIPBOARD_BASE && cmd < CLIPBOARD_BASE + 100) => {
            let idx = (cmd - CLIPBOARD_BASE) as usize;
            let mut selected_text: Option<String> = None;

            // Use renderer to access clipboard module's history and set clipboard
            with_renderer(|renderer| {
                if let Some(module) = renderer.module_registry.get("clipboard") {
                    if let Some(cm) = module.as_any().downcast_ref::<crate::modules::clipboard::ClipboardModule>() {
                        let hist = cm.get_history();
                        if idx < hist.len() {
                            let text = hist[idx].clone();
                            // Set clipboard using module helper
                            cm.set_clipboard_text(&text);
                            selected_text = Some(text);
                        }
                    }
                }
            });

            // If we set clipboard, simulate Ctrl+V to paste
            if selected_text.is_some() {
                use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, VIRTUAL_KEY, VK_CONTROL, KEYEVENTF_KEYUP};
                let vk_v = VIRTUAL_KEY(0x56); // 'V'
                unsafe {
                    let mut inputs = [
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL, wScan: 0, dwFlags: KEYBD_EVENT_FLAGS(0), time: 0, dwExtraInfo: 0 } },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki: KEYBDINPUT { wVk: vk_v, wScan: 0, dwFlags: KEYBD_EVENT_FLAGS(0), time: 0, dwExtraInfo: 0 } },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki: KEYBDINPUT { wVk: vk_v, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki: KEYBDINPUT { wVk: VK_CONTROL, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
                        },
                    ];
                    SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
                }
            }
        },
        
        // App menu
        APP_ABOUT => show_about_dialog(),
        APP_SETTINGS => open_config_file(),
        APP_RELOAD => reload_config(hwnd),
        APP_EXIT => {
            unsafe {
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        
        _ => {}
    }
}

/// Toggle a boolean config value
fn toggle_config_bool<F>(hwnd: HWND, getter: F) 
where 
    F: FnOnce(&mut crate::config::Config) -> &mut bool
{
    if let Some(state) = get_window_state() {
        let config = state.read().config.clone();
        let mut new_config = (*config).clone();
        
        let value = getter(&mut new_config);
        *value = !*value;
        
        if let Err(e) = new_config.save() {
            warn!("Failed to save config: {}", e);
        }
        
        state.write().config = Arc::new(new_config);
        unsafe {
            let _ = InvalidateRect(hwnd, None, true);
        }
    }
}

/// Show about dialog
fn show_about_dialog() {
    use windows::Win32::UI::WindowsAndMessaging::MessageBoxW;
    unsafe {
        let title: Vec<u16> = "About TopBar".encode_utf16().chain(std::iter::once(0)).collect();
        let msg: Vec<u16> = format!(
            "TopBar v{}\n\nA native Windows 11 topbar inspired by macOS.\n\nRight-click modules to configure them.",
            env!("CARGO_PKG_VERSION")
        ).encode_utf16().chain(std::iter::once(0)).collect();
        
        MessageBoxW(
            None,
            PCWSTR(msg.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Toggle a module on/off
fn toggle_module(hwnd: HWND, module_id: &str) {
    if let Some(state) = get_window_state() {
        let config = state.read().config.clone();
        let mut new_config = (*config).clone();

        // Special handling for clock when it's centered
        if module_id == "clock" && new_config.modules.center_modules.iter().any(|m| m == "clock") {
            // Clock is centered, remove it from center and disable
            new_config.modules.center_modules.retain(|m| m != "clock");
            new_config.modules.clock.center = false;
            info!("Disabled centered clock: {}", module_id);
        }
        // Check if module exists in right_modules
        else if let Some(pos) = new_config.modules.right_modules.iter().position(|m| m == module_id) {
            // Remove it
            new_config.modules.right_modules.remove(pos);
            info!("Disabled module: {}", module_id);
        } else {
            // Add it back at the appropriate position
            let default_order = vec![
                "media", "clipboard", "keyboard_layout", "gpu", "system_info", "disk",
                "network", "bluetooth", "volume", "battery", "uptime", "clock"
            ];
            let insert_pos = default_order.iter()
                .position(|&m| m == module_id)
                .map(|target_idx| {
                    // Find where to insert based on existing modules
                    new_config.modules.right_modules.iter()
                        .position(|m| {
                            default_order.iter()
                                .position(|&dm| dm == m.as_str())
                                .map(|existing_idx| existing_idx > target_idx)
                                .unwrap_or(false)
                        })
                        .unwrap_or(new_config.modules.right_modules.len())
                })
                .unwrap_or(new_config.modules.right_modules.len());

            new_config.modules.right_modules.insert(insert_pos, module_id.to_string());
            info!("Enabled module: {}", module_id);
        }
        
        // Save config
        if let Err(e) = new_config.save() {
            warn!("Failed to save config: {}", e);
        }
        
        // Update the state with new config
        state.write().config = Arc::new(new_config);

        // Force a redraw so changes take effect immediately
        unsafe {
            let _ = InvalidateRect(hwnd, None, true);
        }
    }
}

/// Open config file in default editor
fn open_config_file() {
    use crate::config::Config;
    let path = Config::config_path();
    
    // Create config if it doesn't exist
    if !path.exists() {
        if let Ok(config) = Config::load_or_default() {
            let _ = config.save();
        }
    }
    
    unsafe {
        let path_wide: Vec<u16> = path.to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path_wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
    info!("Opening config file: {:?}", path);
}

/// Reload configuration
fn reload_config(hwnd: HWND) {
    use crate::config::Config;
    
    match Config::load_or_default() {
        Ok(config) => {
            if let Some(state) = get_window_state() {
                state.write().config = Arc::new(config);
                info!("Configuration reloaded");
                unsafe {
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
        }
        Err(e) => {
            warn!("Failed to reload config: {}", e);
        }
    }
}

/// Handle module click actions - show in-app configuration dropdowns
fn handle_module_click(hwnd: HWND, module_id: &str, click_x: i32) {
    info!("Module clicked: {}", module_id);
    
    // Get screen position for dropdown
    let mut pt = windows::Win32::Foundation::POINT { x: click_x, y: 28 };
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
    }
    
    match module_id {
        "clock" => show_clock_menu(hwnd, pt.x, pt.y),
        "battery" => show_battery_menu(hwnd, pt.x, pt.y),
        "volume" => show_volume_menu(hwnd, pt.x, pt.y),
        "network" => show_network_menu(hwnd, pt.x, pt.y),
        "system_info" => show_sysinfo_menu(hwnd, pt.x, pt.y),
        "gpu" => show_gpu_menu(hwnd, pt.x, pt.y),
        "keyboard_layout" => show_keyboard_menu(hwnd, pt.x, pt.y),
        "uptime" => show_uptime_menu(hwnd, pt.x, pt.y),
        "bluetooth" => show_bluetooth_menu(hwnd, pt.x, pt.y),
        "disk" => show_disk_menu(hwnd, pt.x, pt.y),
        "clipboard" => show_clipboard_menu(hwnd, pt.x, pt.y),
        "app_menu" => show_app_menu(hwnd, pt.x, pt.y),
        _ => {
            debug!("Unhandled module click: {}", module_id);
        }
    }
}

// Menu IDs for clock settings
const CLOCK_24H: u32 = 2001;
const CLOCK_SECONDS: u32 = 2002;
const CLOCK_DATE: u32 = 2003;
const CLOCK_DAY: u32 = 2004;

// Menu IDs for system info
const SYSINFO_CPU: u32 = 2101;
const SYSINFO_MEM: u32 = 2102;
const SYSINFO_SHOW_GRAPH: u32 = 2103; // show as moving graph

// Menu IDs for volume
const VOL_SHOW_PCT: u32 = 2201;
const VOL_MUTE: u32 = 2202;

// Menu IDs for network  
const NET_SHOW_NAME: u32 = 2301;
const NET_SHOW_SPEED: u32 = 2302;

// Menu IDs for battery
const BAT_SHOW_PCT: u32 = 2401;
const BAT_SHOW_TIME: u32 = 2402;

// Menu IDs for keyboard layout
const KEYBOARD_SHOW_FULL: u32 = 2701;
const KEYBOARD_AUTO_SWITCH: u32 = 2702;

// Menu IDs for uptime
const UPTIME_SHOW_DAYS: u32 = 2801;
const UPTIME_COMPACT: u32 = 2802;

// Menu IDs for bluetooth
const BLUETOOTH_ENABLED: u32 = 2901;
const BLUETOOTH_SHOW_COUNT: u32 = 2902;

// Menu IDs for disk
const DISK_SHOW_PERCENTAGE: u32 = 3001;
const DISK_SHOW_ACTIVITY: u32 = 3002;
// Disk selection base (dynamic entries)
const DISK_SELECT_BASE: u32 = 3100;

// Clipboard history base (dynamic entries)
const CLIPBOARD_BASE: u32 = 4000;

// Clock center toggle
const CLOCK_CENTER: u32 = 2005; 

// Menu IDs for app menu
const APP_ABOUT: u32 = 2501;
const APP_SETTINGS: u32 = 2502;
const APP_RELOAD: u32 = 2503;
const APP_EXIT: u32 = 2504;

fn show_clock_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, CLOCK_24H, "24-Hour Format", config.modules.clock.format_24h);
        append_menu_item(menu, CLOCK_SECONDS, "Show Seconds", config.modules.clock.show_seconds);
        append_menu_item(menu, CLOCK_DATE, "Show Date", config.modules.clock.show_date);
        append_menu_item(menu, CLOCK_DAY, "Show Day of Week", config.modules.clock.show_day);
        append_menu_item(menu, CLOCK_CENTER, "Center Clock", config.modules.clock.center);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Clock menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_battery_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, BAT_SHOW_PCT, "Show Percentage", config.modules.battery.show_percentage);
        append_menu_item(menu, BAT_SHOW_TIME, "Show Time Remaining", config.modules.battery.show_time_remaining);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Battery menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_volume_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, VOL_SHOW_PCT, "Show Percentage", config.modules.volume.show_percentage);
        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        append_menu_item(menu, VOL_MUTE, "Mute", false);  // TODO: Get actual mute state
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Volume menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_network_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, NET_SHOW_NAME, "Show Network Name", config.modules.network.show_name);
        append_menu_item(menu, NET_SHOW_SPEED, "Show Speed (MB/s)", config.modules.network.show_speed);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Network menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_disk_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }

        // Add dynamic list of disks
        let mut disks: Vec<(String,String)> = Vec::new(); // (display, mount)
        with_renderer(|renderer| {
            if let Some(module) = renderer.module_registry.get("disk") {
                if let Some(dm) = module.as_any().downcast_ref::<crate::modules::disk::DiskModule>() {
                    for d in dm.get_disks() {
                        let label = if d.mount_point.is_empty() { d.name.clone() } else { d.mount_point.clone() };
                        disks.push((label, d.mount_point.clone()));
                    }
                }
            }
        });

        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();

        for (i, (label, mount)) in disks.iter().enumerate() {
            let id = DISK_SELECT_BASE + i as u32;
            append_menu_item(menu, id, label, mount == &config.modules.disk.primary_disk);
        }

        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        append_menu_item(menu, DISK_SHOW_PERCENTAGE, "Show Percentage", config.modules.disk.show_percentage);
        append_menu_item(menu, DISK_SHOW_ACTIVITY, "Show Activity", config.modules.disk.show_activity);

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();

        info!("Disk menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_clipboard_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }

        // Gather latest clipboard history from the module
        let mut history: Vec<String> = Vec::new();
        with_renderer(|renderer| {
            if let Some(module) = renderer.module_registry.get("clipboard") {
                if let Some(cm) = module.as_any().downcast_ref::<crate::modules::clipboard::ClipboardModule>() {
                    history = cm.get_history();
                }
            }
        });

        if history.is_empty() {
            append_menu_item(menu, CLIPBOARD_BASE, "No clipboard history", false);
        } else {
            for (i, entry) in history.iter().enumerate() {
                if i >= 10 { break; }
                let label = crate::utils::truncate_string(entry, 40);
                let id = CLIPBOARD_BASE + i as u32;
                append_menu_item(menu, id, &label, i == 0);
            }
        }

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();

        info!("Clipboard menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_sysinfo_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, SYSINFO_CPU, "Show CPU Usage", config.modules.system_info.show_cpu);
        append_menu_item(menu, SYSINFO_MEM, "Show Memory Usage", config.modules.system_info.show_memory);
        append_menu_item(menu, SYSINFO_SHOW_GRAPH, "Show Graph", config.modules.system_info.show_graph);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Sysinfo menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_app_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        append_menu_item(menu, APP_ABOUT, "About TopBar", false);
        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        append_menu_item(menu, APP_SETTINGS, "Open Config File", false);
        append_menu_item(menu, APP_RELOAD, "Reload Config", false);
        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        append_menu_item(menu, APP_EXIT, "Exit TopBar", false);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("App menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_gpu_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
            append_menu_item(menu, GPU_SHOW_USAGE, "Show GPU Usage", config.modules.gpu.show_usage);
        append_menu_item(menu, GPU_SHOW_MEMORY, "Show Memory Usage", config.modules.gpu.show_memory);
        append_menu_item(menu, GPU_SHOW_TEMP, "Show Temperature", config.modules.gpu.show_temperature);
        append_menu_item(menu, GPU_SHOW_GRAPH, "Show Graph", config.modules.gpu.show_graph);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("GPU menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_keyboard_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, KEYBOARD_SHOW_FULL, "Show Full Language Name", config.modules.keyboard_layout.show_full_name);
        append_menu_item(menu, KEYBOARD_AUTO_SWITCH, "Auto-switch on Window Focus", config.modules.keyboard_layout.auto_switch);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Keyboard menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_uptime_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, UPTIME_SHOW_DAYS, "Show Days in Uptime", config.modules.uptime.show_days);
        append_menu_item(menu, UPTIME_COMPACT, "Compact Format", config.modules.uptime.compact_format);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Uptime menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}

fn show_bluetooth_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() { return; }
        
        let config = get_window_state()
            .map(|s| s.read().config.clone())
            .unwrap_or_default();
        
        append_menu_item(menu, BLUETOOTH_ENABLED, "Enable Bluetooth Module", config.modules.bluetooth.enabled);
        append_menu_item(menu, BLUETOOTH_SHOW_COUNT, "Show Device Count", config.modules.bluetooth.show_device_count);
        
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD, x, y, 0, hwnd, None);
        DestroyMenu(menu).ok();
        
        info!("Bluetooth menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            handle_menu_command(hwnd, cmd.0 as u32);
        }
    }
}


