//! Window state management for the TopBar application
//!
//! Handles the global window state that needs to be accessed from window procedures.

use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::HWND;

use crate::config::Config;
use crate::theme::ThemeManager;
use crate::utils::Rect;

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
static WINDOW_STATE: once_cell::sync::OnceCell<Arc<RwLock<WindowState>>> =
    once_cell::sync::OnceCell::new();

// Global main window handle (stored as isize for Send + Sync)
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

/// Get the global window state
pub fn get_window_state() -> Option<Arc<RwLock<WindowState>>> {
    WINDOW_STATE.get().cloned()
}

/// Set the global window state (called during window creation)
pub fn set_window_state(state: Arc<RwLock<WindowState>>) {
    let _ = WINDOW_STATE.set(state);
}

/// Store the main window handle for cross-thread access
pub fn set_main_hwnd(hwnd: HWND) {
    MAIN_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
}

/// Get the main window handle (can be called from any thread)
pub fn get_main_hwnd() -> Option<HWND> {
    let val = MAIN_HWND.load(Ordering::SeqCst);
    if val != 0 {
        Some(HWND(val as *mut std::ffi::c_void))
    } else {
        None
    }
}