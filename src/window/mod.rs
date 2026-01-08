//! Window management module for the TopBar application
//!
//! This module contains all window-related functionality, split into logical submodules.

pub mod state;
pub mod manager;
pub mod renderer;
pub mod proc;
pub mod menus;
pub mod module_handlers;
pub mod config_handlers;

// Re-export main types for convenience
pub use manager::WindowManager;
pub use proc::{window_proc, WM_TOPBAR_UPDATE, WM_TOPBAR_THEME_CHANGED, WM_TOPBAR_TRAY, WM_TOPBAR_MODULE_CLICK, WM_TOPBAR_NIGHTLIGHT_TOGGLED};
pub use state::get_main_hwnd;