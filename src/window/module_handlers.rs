//! Module interaction handlers for the TopBar application
//!
//! Contains functions for handling module clicks and showing module-specific menus.

use log::{debug, info};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::{ClientToScreen, InvalidateRect};

use crate::utils::open_url;

use super::state::get_window_state;
use super::renderer::with_renderer;
use super::menus::show_popup_menu;

// Menu IDs for clock settings
const CLOCK_24H: u32 = 2001;
const CLOCK_SECONDS: u32 = 2002;
const CLOCK_DATE: u32 = 2003;
const CLOCK_DAY: u32 = 2004;

// Menu IDs for system info
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

// Menu IDs for uptime
// (compact/ShowDays removed - behavior now fixed)

// Menu IDs for bluetooth
const BLUETOOTH_SHOW_COUNT: u32 = 2902;

// Menu IDs for disk
// (Show Percentage and Show Activity removed - percentage always on)
// Disk selection base (dynamic entries)
const DISK_SELECT_BASE: u32 = 3100;

// Clipboard history base (dynamic entries)
const CLIPBOARD_BASE: u32 = 4000;

// Weather menu IDs
const WEATHER_OPEN: u32 = 6001;
const WEATHER_REFRESH: u32 = 6002;

// Clock center toggle
const CLOCK_CENTER: u32 = 2005;

// Menu IDs for app menu
const APP_ABOUT: u32 = 2501;
const APP_SETTINGS: u32 = 2502;
const APP_RELOAD: u32 = 2503;
const APP_RESET: u32 = 2505;
const APP_INSTALL_CURSORS: u32 = 2506;
const APP_EXIT: u32 = 2504;

/// Handle module click actions - show in-app configuration dropdowns
pub fn handle_module_click(hwnd: HWND, module_id: &str, click_x: i32) {
    info!("Module clicked: {}", module_id);

    // Special case: keyboard_layout should switch languages on click, not show menu
    if module_id == "keyboard_layout" {
        with_renderer(|renderer| {
            if let Some(module) = renderer.module_registry.get_mut(module_id) {
                module.on_click();
            }
        });
        // Request redraw to update the display
        if let Some(state) = get_window_state() {
            state.write().needs_redraw = true;
        }
        unsafe {
            let _ = InvalidateRect(hwnd, None, false);
        }
        return;
    }

    // Get screen position for dropdown
    let mut pt = windows::Win32::Foundation::POINT { x: click_x, y: 28 };
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
    }

    show_module_menu(hwnd, module_id, pt.x, pt.y);
}

/// Show module-specific menu
pub fn show_module_menu(hwnd: HWND, module_id: &str, x: i32, y: i32) {
    match module_id {
        "clock" => show_clock_menu(hwnd, x, y),
        "battery" => show_battery_menu(hwnd, x, y),
        "volume" => show_volume_menu(hwnd, x, y),
        "network" => show_network_menu(hwnd, x, y),
        "system_info" => show_sysinfo_menu(hwnd, x, y),
        "gpu" => show_gpu_menu(hwnd, x, y),
        "keyboard_layout" => show_keyboard_menu(hwnd, x, y), // This won't be reached due to early return above
        "uptime" => show_uptime_menu(hwnd, x, y),
        "bluetooth" => show_bluetooth_menu(hwnd, x, y),
        "night_light" => {
            // Toggle night light directly
            with_renderer(|renderer| {
                if let Some(module) = renderer.module_registry.get_mut("night_light") {
                    module.on_click();
                }
            });
            // Request redraw to update the icon
            if let Some(state) = get_window_state() {
                state.write().needs_redraw = true;
            }
            unsafe {
                let _ = InvalidateRect(hwnd, None, false);
            }
        }
        "disk" => show_disk_menu(hwnd, x, y),
        "clipboard" => show_clipboard_menu(hwnd, x, y),
        "app_menu" => show_app_menu(hwnd, x, y),
        "weather" => show_weather_menu(hwnd, x, y),
        "search" => {
            // Open quick search popup
            let _ = crate::render::show_quick_search(hwnd);
        }
        _ => {
            debug!("Unhandled module click: {}", module_id);
        }
    }
}

fn show_clock_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, CLOCK_24H, "24-Hour Format", config.modules.clock.format_24h);
        append_menu_item(menu, CLOCK_SECONDS, "Show Seconds", config.modules.clock.show_seconds);
        append_menu_item(menu, CLOCK_DATE, "Show Date", config.modules.clock.show_date);
        append_menu_item(menu, CLOCK_DAY, "Show Day of Week", config.modules.clock.show_day);
        append_menu_item(menu, CLOCK_CENTER, "Center Clock", config.modules.clock.center);
    });

    if cmd != 0 {
        info!("Clock menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_battery_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, BAT_SHOW_PCT, "Show Percentage", config.modules.battery.show_percentage);
        append_menu_item(menu, BAT_SHOW_TIME, "Show Time Remaining", config.modules.battery.show_time_remaining);
    });

    if cmd != 0 {
        info!("Battery menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_volume_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    // Get actual mute state from volume module
    let mut is_muted = false;
    with_renderer(|renderer| {
        if let Some(module) = renderer.module_registry.get("volume") {
            if let Some(vm) = module.as_any().downcast_ref::<crate::modules::volume::VolumeModule>() {
                is_muted = vm.is_muted();
            }
        }
    });

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, VOL_SHOW_PCT, "Show Percentage", config.modules.volume.show_percentage);
        unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, None).ok(); }
        append_menu_item(menu, VOL_MUTE, "Mute", is_muted);
    });

    if cmd != 0 {
        info!("Volume menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_network_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, NET_SHOW_NAME, "Show Network Name", config.modules.network.show_name);
        append_menu_item(menu, NET_SHOW_SPEED, "Show Speed (MB/s)", config.modules.network.show_speed);
    });

    if cmd != 0 {
        info!("Network menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_disk_menu(hwnd: HWND, x: i32, y: i32) {
    // Get dynamic list of disks
    let mut disks: Vec<(String, String)> = Vec::new();
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

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        for (i, (label, mount)) in disks.iter().enumerate() {
            let id = DISK_SELECT_BASE + i as u32;
            append_menu_item(menu, id, label, mount == &config.modules.disk.primary_disk);
        }
    });

    if cmd != 0 {
        info!("Disk menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_clipboard_menu(hwnd: HWND, x: i32, y: i32) {
    // Gather latest clipboard history from the module
    let mut history: Vec<String> = Vec::new();
    with_renderer(|renderer| {
        if let Some(module) = renderer.module_registry.get("clipboard") {
            if let Some(cm) = module.as_any().downcast_ref::<crate::modules::clipboard::ClipboardModule>() {
                history = cm.get_history();
            }
        }
    });

    // Capture the currently focused window so we can restore it when pasting
    let prev_hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        if history.is_empty() {
            append_menu_item(menu, CLIPBOARD_BASE, "No clipboard history", false);
        } else {
            for (i, entry) in history.iter().take(10).enumerate() {
                let label = crate::utils::truncate_string(entry, 40);
                // No checkmark — top item being in clipboard is implicit
                append_menu_item(menu, CLIPBOARD_BASE + i as u32, &label, false);
            }
        }
    });

    if cmd != 0 {
        let cmd_id = cmd as u32;
        // If a clipboard entry was selected, set clipboard & try to paste into the previous window
        if (CLIPBOARD_BASE..CLIPBOARD_BASE + 100).contains(&cmd_id) {
            let idx = (cmd_id - CLIPBOARD_BASE) as usize;
            if idx < history.len() {
                let text = history[idx].clone();

                // Update the clipboard via the module (so in-memory state is consistent)
                with_renderer(|renderer| {
                    if let Some(module) = renderer.module_registry.get_mut("clipboard") {
                        if let Some(cm) = module.as_any_mut().downcast_mut::<crate::modules::clipboard::ClipboardModule>() {
                            cm.set_clipboard_text(&text);
                        }
                    }
                });

                // Try to restore focus to previous window and send Ctrl+V
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(prev_hwnd);
                    // Small delay to allow focus to settle
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    use windows::Win32::UI::Input::KeyboardAndMouse::{
                        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                        KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
                    };
                    let vk_v = VIRTUAL_KEY(0x56); // 'V'
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
        } else {
            info!("Clipboard menu returned cmd: {}", cmd_id);
            super::menus::handle_menu_command(hwnd, cmd_id);
        }
    }
}

fn show_sysinfo_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        // CPU and Memory are always shown; do not expose toggles to the user.
        append_menu_item(menu, SYSINFO_SHOW_GRAPH, "Show Graph", config.modules.system_info.show_graph);
    });

    if cmd != 0 {
        info!("Sysinfo menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

/// Show weather forecast menu with upcoming days and actions
fn show_weather_menu(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.is_invalid() {
            return;
        }

        // Gather forecast from module
        let mut lines: Vec<String> = Vec::new();
        with_renderer(|renderer| {
            if let Some(module) = renderer.module_registry.get("weather") {
                if let Some(wm) = module
                    .as_any()
                    .downcast_ref::<crate::modules::weather::WeatherModule>()
                {
                    if let Some(data) = wm.weather_data() {
                        if data.forecast.is_empty() {
                            lines.push("No forecast available".to_string());
                        } else {
                            for fc in data.forecast.iter() {
                                // Show raw Celsius values (config unit not accessible here)
                                let max = fc.max;
                                let min = fc.min;
                                let icon = fc.condition.icon();
                                let label = format!(
                                    "{} {} {:.0}°C / {:.0}°C - {}",
                                    crate::modules::weather::WeatherModule::relative_date_label(&fc.date), icon, max, min, fc.description
                                );
                                lines.push(label);
                            }
                        }
                    }
                }
            }
        });

        if lines.is_empty() {
            append_menu_item(menu, WEATHER_REFRESH, "Fetching weather...", false);
        } else {
            for (i, l) in lines.iter().enumerate() {
                // Cap to reasonable number
                if i >= 6 {
                    break;
                }
                append_menu_item(menu, WEATHER_OPEN + i as u32, &l, false);
            }
        }

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

        info!("Weather menu returned cmd: {}", cmd.0);
        if cmd.0 != 0 {
            let cmd_id = cmd.0 as u32;
            match cmd_id {
                id if id >= WEATHER_OPEN && id < WEATHER_OPEN + 10 => {
                    // Clicking a forecast day - open forecast in browser
                    open_url("https://wttr.in/");
                }
                _ => {}
            }
        }
    }
}

fn show_app_menu(hwnd: HWND, x: i32, y: i32) {
    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, APP_ABOUT, "Quickstart / Intro Guide", false);
        append_menu_item(menu, APP_INSTALL_CURSORS, "Install macOS Cursors", false);
        unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, None).ok(); }
        append_menu_item(menu, APP_SETTINGS, "Open Config File", false);
        append_menu_item(menu, APP_RELOAD, "Reload Config", false);
        append_menu_item(menu, APP_RESET, "Reset to Defaults", false);
        unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, None).ok(); }
        append_menu_item(menu, APP_EXIT, "Exit TopBar", false);
    });

    if cmd != 0 {
        info!("App menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_gpu_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        // GPU usage is always shown; do not expose a toggle in the menu.
        append_menu_item(menu, 2604, "Show Graph", config.modules.gpu.show_graph);
    });

    if cmd != 0 {
        info!("GPU menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_keyboard_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, KEYBOARD_SHOW_FULL, "Show Full Language Name", config.modules.keyboard_layout.show_full_name);
    });

    if cmd != 0 {
        info!("Keyboard menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_uptime_menu(hwnd: HWND, x: i32, y: i32) {
    // Currently no settings for uptime module
    let cmd = show_popup_menu(hwnd, x, y, |_menu| {});
    if cmd != 0 {
        info!("Uptime menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
    }
}

fn show_bluetooth_menu(hwnd: HWND, x: i32, y: i32) {
    let config = get_window_state()
        .map(|s| s.read().config.clone())
        .unwrap_or_default();

    let cmd = show_popup_menu(hwnd, x, y, |menu| {
        append_menu_item(menu, BLUETOOTH_SHOW_COUNT, "Show Device Count", config.modules.bluetooth.show_device_count);
    });

    if cmd != 0 {
        info!("Bluetooth menu returned cmd: {}", cmd);
        super::menus::handle_menu_command(hwnd, cmd);
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