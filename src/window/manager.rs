//! Window manager for creating and managing the TopBar window
//!
//! Handles window creation, positioning, styling, and lifecycle management.

use anyhow::Result;
use log::info;
use parking_lot::RwLock;
use std::sync::Arc;
use windows::core::w;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWM_SYSTEMBACKDROP_TYPE,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::InvalidateRect;

use crate::config::{BarPosition, Config};
use crate::hotkey::HotkeyAction;
use crate::render::Renderer;
use crate::theme::Theme;
use crate::utils::{get_screen_size, scale_by_dpi, to_pcwstr, to_wide_string, Rect};

use super::state::{get_window_state, set_window_state, WindowState};

/// Window class name
const WINDOW_CLASS: &str = "TopBarWindowClass";
const WINDOW_TITLE: &str = "TopBar";

/// Main window manager
pub struct WindowManager {
    hwnd: HWND,
    state: Arc<RwLock<WindowState>>,
    // Keep hotkeys registered for the lifetime of the window manager
    hotkey_manager_owned: bool, // we track ownership so we can unregister named hotkeys on drop
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
        set_window_state(state.clone());

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
        Self::apply_window_style(hwnd, state.read().theme_manager.theme())?;

        // Calculate and set position
        let bar_rect = Self::calculate_bar_rect(&config, dpi);
        {
            let mut state_guard = state.write();
            state_guard.bar_rect = bar_rect;
        }

        Self::position_window(hwnd, &bar_rect, &config)?;

        // Initialize renderer (stored in thread-local)
        super::renderer::set_renderer(Renderer::new(hwnd, dpi)?);

        // Ensure older configs get migrated to enable graph view by default
        {
            let cfg = state.read().config.clone();
            let mut new_cfg = (*cfg).clone();
            if new_cfg.migrate_enable_graphs() {
                state.write().config = Arc::new(new_cfg);
            }
        }

        // Force a weather refresh so icons update immediately after migration
        super::renderer::with_renderer(|renderer| {
            if let Some(module) = renderer.module_registry.get_mut("weather") {
                if let Some(wm) = module
                    .as_any_mut()
                    .downcast_mut::<crate::modules::weather::WeatherModule>() {
                    wm.refresh();
                }
            }
        });

        // Register configured hotkeys and store a simple map for dispatch
        let mut global_map: std::collections::HashMap<i32, HotkeyAction> = std::collections::HashMap::new();

        // Helper to register a single hotkey id for a configured string
        let mut register_k = |id: i32, key_str: Option<String>, action: HotkeyAction| {
            if let Some(s) = key_str {
                if let Some(hk) = crate::hotkey::Hotkey::parse(&s, action) {
                    unsafe {
                        let res = RegisterHotKey(hwnd, id, windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS(hk.modifiers), hk.key);
                        if res.is_ok() {
                            // Record mapping and log success for diagnostics
                            global_map.insert(id, action);
                            info!("Registered hotkey '{}' -> id={} modifiers={} key=0x{:X}", s, id, hk.modifiers, hk.key);
                        } else {
                            let err = unsafe { GetLastError() };
                            log::warn!("Failed to register hotkey {} -> {} (err={})", s, id, err.0);
                        }
                    }
                }
            }
        };


        // Fixed ids for core hotkeys (keeps behavior deterministic)
        const HK_TOGGLE_BAR: i32 = 6000;
        const HK_OPEN_MENU: i32 = 6001;
        const HK_QUICK_SEARCH: i32 = 6002;
        const HK_TOGGLE_THEME: i32 = 6003;

        register_k(HK_TOGGLE_BAR, config.hotkeys.toggle_bar.clone(), HotkeyAction::ToggleBar);
        register_k(HK_OPEN_MENU, config.hotkeys.open_menu.clone(), HotkeyAction::OpenMenu);
        // Only register quick-search hotkey if search is enabled
        if config.search.enabled {
            register_k(HK_QUICK_SEARCH, config.hotkeys.quick_search.clone(), HotkeyAction::QuickSearch);
        }
        register_k(HK_TOGGLE_THEME, config.hotkeys.toggle_theme.clone(), HotkeyAction::ToggleTheme);

        crate::hotkey::set_global_hotkey_map(global_map);

        // Log the final global hotkey map for diagnostics (helpful when registrations fail)
        if let Some(m) = crate::hotkey::global_hotkey_map() {
            let g = m.lock();
            info!("Global hotkey map configured: {:?}", g);
        }

        info!("Window created successfully at {:?}", bar_rect);

        Ok(Self { hwnd, state, hotkey_manager_owned: true })
    }

    /// Register the window class
    fn register_window_class(class_name: &[u16]) -> Result<()> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(super::proc::window_proc),
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
                0,
                0,
                100,
                100, // Temporary size, will be set later
                None,
                None,
                hinstance,
                None,
            )?;

            if hwnd.0.is_null() {
                return Err(anyhow::anyhow!("Failed to create window"));
            }

            // Set layered window attributes for transparency
            let opacity = (config.appearance.opacity * 255.0) as u8;
            SetLayeredWindowAttributes(
                hwnd,
                windows::Win32::Foundation::COLORREF(0),
                opacity,
                LWA_ALPHA,
            )?;

            Ok(hwnd)
        }
    }

    /// Apply Windows 11 styling to the window
    pub fn apply_window_style(hwnd: HWND, theme: &Theme) -> Result<()> {
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
            let backdrop_type: i32 = 3; // Acrylic
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
    pub fn calculate_bar_rect(config: &Config, dpi: u32) -> Rect {
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
        use windows::Win32::UI::Shell::{
            SHAppBarMessage, ABE_BOTTOM, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_SETPOS, APPBARDATA,
        };

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
    pub fn remove_screen_space(hwnd: HWND) {
        use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_REMOVE, APPBARDATA};

        unsafe {
            let mut abd = APPBARDATA {
                cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                hWnd: hwnd,
                uCallbackMessage: 0,
                uEdge: 0,
                rc: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
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
            SetTimer(self.hwnd, 1, 1000, None); // 1 second timer for clock
            SetTimer(self.hwnd, 2, 2000, None); // 2 second timer for system info
            SetTimer(self.hwnd, 3, 100, None); // 100ms timer for animations

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