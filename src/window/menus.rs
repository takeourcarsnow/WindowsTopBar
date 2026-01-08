//! Menu handling for the TopBar application
//!
//! Contains functions for displaying and handling context menus and module-specific menus.

use log::{info, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows::Win32::Graphics::Gdi::InvalidateRect;

use crate::config::Config;

use super::state::get_window_state;
use super::renderer::with_renderer;
use super::config_handlers::{open_config_file, reload_config, reset_config, toggle_config_bool, toggle_module};

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
const MENU_SHOW_NIGHT_LIGHT: u32 = 1014;

// GPU menu items
const GPU_SHOW_USAGE: u32 = 2601;
const GPU_SHOW_GRAPH: u32 = 2604;
const MENU_SETTINGS: u32 = 1200;
const MENU_RELOAD: u32 = 1201;
const MENU_RESET: u32 = 1202;
const MENU_TOGGLE_SEARCH: u32 = 1210;
const MENU_EXIT: u32 = 1999;

/// Helper to display a popup menu and return the selected command ID (or 0 if none)
pub fn show_popup_menu(hwnd: HWND, x: i32, y: i32, build_menu: impl FnOnce(HMENU)) -> u32 {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() {
            return 0;
        }

        build_menu(menu);

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            x,
            y,
            0,
            hwnd,
            None,
        );
        DestroyMenu(menu).ok();
        cmd.0 as u32
    }
}

/// Show the context menu
pub fn show_context_menu(hwnd: HWND, x: i32, y: i32) {
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
        append_menu_item(
            menu,
            MENU_SHOW_CLOCK,
            "Clock",
            right_modules.contains(&"clock".to_string())
                || (config.modules.clock.center && center_modules.contains(&"clock".to_string())),
        );
        append_menu_item(
            menu,
            MENU_SHOW_BATTERY,
            "Battery",
            right_modules.contains(&"battery".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_VOLUME,
            "Volume",
            right_modules.contains(&"volume".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_NETWORK,
            "Network",
            right_modules.contains(&"network".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_SYSINFO,
            "System Info",
            right_modules.contains(&"system_info".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_MEDIA,
            "Media Controls",
            right_modules.contains(&"media".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_CLIPBOARD,
            "Clipboard",
            right_modules.contains(&"clipboard".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_GPU,
            "GPU Usage",
            right_modules.contains(&"gpu".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_KEYBOARD,
            "Keyboard Layout",
            right_modules.contains(&"keyboard_layout".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_UPTIME,
            "System Uptime",
            right_modules.contains(&"uptime".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_BLUETOOTH,
            "Bluetooth",
            right_modules.contains(&"bluetooth".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_NIGHT_LIGHT,
            "Night Light",
            right_modules.contains(&"night_light".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_DISK,
            "Disk Usage",
            right_modules.contains(&"disk".to_string()),
        );
        append_menu_item(
            menu,
            MENU_SHOW_WEATHER,
            "Weather",
            right_modules.contains(&"weather".to_string()),
        );

        // Separator
        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();

        // Settings and exit
        append_menu_item(menu, MENU_TOGGLE_SEARCH, "Enable Quick Search", config.search.enabled);
        append_menu_item(menu, MENU_SETTINGS, "Open Config File", false);
        append_menu_item(menu, MENU_RELOAD, "Reload Config", false);
        append_menu_item(menu, MENU_RESET, "Reset to Defaults", false);

        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();
        append_menu_item(menu, MENU_EXIT, "Exit TopBar", false);

        // Need to set foreground for menu to work properly
        let _ = SetForegroundWindow(hwnd);

        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            x,
            y,
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
        let flags = if checked {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(menu, flags, id as usize, PCWSTR(wide.as_ptr())).ok();
    }
}

/// Handle menu commands
pub fn handle_menu_command(hwnd: HWND, cmd_id: u32) {
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
        MENU_SHOW_NIGHT_LIGHT => toggle_module(hwnd, "night_light"),
        MENU_SHOW_DISK => toggle_module(hwnd, "disk"),
        MENU_SHOW_WEATHER => toggle_module(hwnd, "weather"),
        MENU_SETTINGS => open_config_file(),
        MENU_RELOAD => reload_config(hwnd),
        MENU_RESET => reset_config(hwnd),
        MENU_EXIT => unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        },

        // Clock settings
        2001 => toggle_config_bool(hwnd, |c| &mut c.modules.clock.format_24h),
        2002 => toggle_config_bool(hwnd, |c| &mut c.modules.clock.show_seconds),
        2003 => toggle_config_bool(hwnd, |c| &mut c.modules.clock.show_date),
        2004 => toggle_config_bool(hwnd, |c| &mut c.modules.clock.show_day),

        // Battery settings
        2401 => {
            toggle_config_bool(hwnd, |c| &mut c.modules.battery.show_percentage);
            // Force battery module to rebuild display text so percentage toggles immediately
            if let Some(state) = get_window_state() {
                let config = state.read().config.clone();
                with_renderer(|renderer| {
                    if let Some(module) = renderer.module_registry.get_mut("battery") {
                        if let Some(b) = module
                            .as_any_mut()
                            .downcast_mut::<crate::modules::battery::BatteryModule>()
                        {
                            b.rebuild_cached_text(&config);
                        }
                    }
                });
                unsafe {
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
        },
        2402 => toggle_config_bool(hwnd, |c| &mut c.modules.battery.show_time_remaining),

        // Volume settings
        2201 => {
            toggle_config_bool(hwnd, |c| &mut c.modules.volume.show_percentage);
            if let Some(state) = get_window_state() {
                let config = state.read().config.clone();
                with_renderer(|renderer| {
                    if let Some(module) = renderer.module_registry.get_mut("volume") {
                        if let Some(v) = module
                            .as_any_mut()
                            .downcast_mut::<crate::modules::volume::VolumeModule>()
                        {
                            v.rebuild_cached_text(&config);
                        }
                    }
                });
                unsafe {
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
        },
        2202 => {
            with_renderer(|renderer| {
                if let Some(module) = renderer.module_registry.get_mut("volume") {
                    // Cast to VolumeModule to access toggle_mute
                    if let Some(volume_module) = module
                        .as_any_mut()
                        .downcast_mut::<crate::modules::volume::VolumeModule>() {
                        volume_module.toggle_mute();
                    }
                }
            });
        }

        // Network settings
        2301 => toggle_config_bool(hwnd, |c| &mut c.modules.network.show_name),
        2302 => toggle_config_bool(hwnd, |c| &mut c.modules.network.show_speed),

        // System info settings
        2103 => toggle_config_bool(hwnd, |c| &mut c.modules.system_info.show_graph),

        // GPU settings
        2604 => toggle_config_bool(hwnd, |c| &mut c.modules.gpu.show_graph),

        // Keyboard layout settings
        2701 => {
            toggle_config_bool(hwnd, |c| &mut c.modules.keyboard_layout.show_full_name)
        }

        // Uptime settings
        // (ShowDays and Compact removed - fixed behavior)

        // Bluetooth settings
        2902 => {
            toggle_config_bool(hwnd, |c| &mut c.modules.bluetooth.show_device_count)
        }

        // Disk settings
        // (Percentage and Activity removed - percentage always on)

        // Center clock toggle (moves between right and center sections)
        2005 => {
            if let Some(state) = get_window_state() {
                let config = state.read().config.clone();
                let mut new_config = (*config).clone();
                
                if new_config.modules.center_modules.iter().any(|m| m == "clock") {
                    // Remove from center, add back to right at default position
                    new_config.modules.center_modules.retain(|m| m != "clock");
                    new_config.modules.clock.center = false;

                    if !new_config.modules.right_modules.iter().any(|m| m == "clock") {
                        let insert_pos = find_module_insert_position(&new_config.modules.right_modules, "clock");
                        new_config.modules.right_modules.insert(insert_pos, "clock".to_string());
                    }
                } else {
                    // Add to center and remove from right
                    new_config.modules.center_modules.push("clock".to_string());
                    new_config.modules.right_modules.retain(|m| m != "clock");
                    new_config.modules.clock.center = true;
                }

                if let Err(e) = new_config.save() {
                    warn!("Failed to save config: {}", e);
                }
                state.write().config = std::sync::Arc::new(new_config);
                unsafe {
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
        }

        MENU_TOGGLE_SEARCH => {
            if let Some(state) = get_window_state() {
                let config = state.read().config.clone();
                let mut new_config = (*config).clone();
                // Toggle search enabled
                new_config.search.enabled = !new_config.search.enabled;

                // Save and apply config
                if let Err(e) = new_config.save() {
                    warn!("Failed to save config: {}", e);
                }

                // Update in-memory config
                state.write().config = std::sync::Arc::new(new_config.clone());

                // Hotkey id we use for quick search
                const HK_QUICK_SEARCH: i32 = 6002;

                // If now enabled, kick off a background build and watcher; also register hotkey
                if new_config.search.enabled {
                    // Register hotkey for quick search if configured
                    if let Some(ref s) = new_config.hotkeys.quick_search {
                        if let Some(hk) = crate::hotkey::Hotkey::parse(s, crate::hotkey::HotkeyAction::QuickSearch) {
                            unsafe {
                                let _ = RegisterHotKey(hwnd, HK_QUICK_SEARCH, windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS(hk.modifiers), hk.key);
                            }
                            if let Some(map) = crate::hotkey::global_hotkey_map() {
                                let mut guard = map.lock();
                                guard.insert(HK_QUICK_SEARCH, crate::hotkey::HotkeyAction::QuickSearch);
                            }
                        }
                    }

                    // Build in background and set global index
                    let paths = new_config.search.index_paths.clone();
                    std::thread::spawn(move || {
                        match crate::search::SearchIndex::build(&paths) {
                            Ok(idx) => {
                                if let Some(g) = crate::search::global_index() {
                                    *g.write() = Some(idx);
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to build search index after enabling: {}", e);
                            }
                        }
                    });
                } else {
                    // Disable: clear the in-memory index and unregister the hotkey
                    if let Some(g) = crate::search::global_index() {
                        *g.write() = None;
                    }
                    unsafe {
                        let _ = UnregisterHotKey(hwnd, HK_QUICK_SEARCH);
                    }
                    if let Some(map) = crate::hotkey::global_hotkey_map() {
                        let mut guard = map.lock();
                        guard.remove(&HK_QUICK_SEARCH);
                    }
                }

                unsafe {
                    let _ = InvalidateRect(hwnd, None, true);
                }
            }
        }

        // Disk dynamic selection range
        cmd if (3100..3200).contains(&cmd) => {
            let idx = (cmd - 3100) as usize;
            if let Some(state) = get_window_state() {
                // Get disks from renderer
                let mut selected_mount: Option<String> = None;
                with_renderer(|renderer| {
                    if let Some(module) = renderer.module_registry.get("disk") {
                        if let Some(dm) = module
                            .as_any()
                            .downcast_ref::<crate::modules::disk::DiskModule>(){
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
                    state.write().config = std::sync::Arc::new(new_config);
                    unsafe {
                        let _ = InvalidateRect(hwnd, None, true);
                    }
                }
            }
        }

        // Clipboard history selection range
        cmd if (4000..4100).contains(&cmd) => {
            let idx = (cmd - 4000) as usize;
            let mut selected_text: Option<String> = None;

            // Use renderer to access clipboard module's history and set clipboard
            with_renderer(|renderer| {
                if let Some(module) = renderer.module_registry.get("clipboard") {
                    if let Some(cm) = module
                        .as_any()
                        .downcast_ref::<crate::modules::clipboard::ClipboardModule>(){
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
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
                };
                let vk_v = VIRTUAL_KEY(0x56); // 'V'
                unsafe {
                    let inputs = [
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_CONTROL,
                                    wScan: 0,
                                    dwFlags: KEYBD_EVENT_FLAGS(0),
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: vk_v,
                                    wScan: 0,
                                    dwFlags: KEYBD_EVENT_FLAGS(0),
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: vk_v,
                                    wScan: 0,
                                    dwFlags: KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_CONTROL,
                                    wScan: 0,
                                    dwFlags: KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        },
                    ];
                    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
                }
            }
        }

        // App menu
        2501 => show_about_dialog(),
        2502 => open_config_file(),
        2503 => reload_config(hwnd),
        2505 => reset_config(hwnd),
        2504 => unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        },

        _ => {}
    }
}

/// Show about dialog
fn show_about_dialog() {
    use windows::Win32::UI::WindowsAndMessaging::MessageBoxW;
    unsafe {
        let title: Vec<u16> = "About TopBar"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
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

/// Default order of right-side modules for insertion position calculation
const DEFAULT_RIGHT_MODULE_ORDER: &[&str] = &[
    "weather",
    "media",
    "clipboard",
    "keyboard_layout",
    "gpu",
    "system_info",
    "disk",
    "network",
    "bluetooth",
    "night_light",
    "volume",
    "battery",
    "uptime",
    "clock",
];

/// Find the appropriate insert position for a module based on default order
fn find_module_insert_position(existing_modules: &[String], module_id: &str) -> usize {
    DEFAULT_RIGHT_MODULE_ORDER
        .iter()
        .position(|&m| m == module_id)
        .map(|target_idx| {
            existing_modules
                .iter()
                .position(|m| {
                    DEFAULT_RIGHT_MODULE_ORDER
                        .iter()
                        .position(|&dm| dm == m.as_str())
                        .map(|existing_idx| existing_idx > target_idx)
                        .unwrap_or(false)
                })
                .unwrap_or(existing_modules.len())
        })
        .unwrap_or(existing_modules.len())
}