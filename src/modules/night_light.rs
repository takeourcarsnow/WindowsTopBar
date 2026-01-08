//! Night Light module - toggle Windows Night Light feature

use std::time::Instant;

use super::Module;
use windows::Win32::System::Registry::{
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, RegCloseKey,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_BINARY, REG_VALUE_TYPE,
};
use windows::core::PCWSTR;

/// Registry path for Night Light state
const NIGHT_LIGHT_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current\default$windows.data.bluelightreduction.bluelightreductionstate\windows.data.bluelightreduction.bluelightreductionstate";

/// Night Light state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NightLightState {
    On,
    Off,
    Unknown,
}

/// Night Light module
pub struct NightLightModule {
    state: NightLightState,
    last_update: Instant,
}

impl NightLightModule {
    pub fn new() -> Self {
        let mut module = Self {
            state: NightLightState::Unknown,
            last_update: Instant::now(),
        };
        module.refresh_state();
        module
    }

    /// Read current Night Light state from registry
    fn refresh_state(&mut self) {
        self.state = Self::read_night_light_state().unwrap_or(NightLightState::Unknown);
        self.last_update = Instant::now();
    }

    /// Read the Night Light state from Windows registry
    fn read_night_light_state() -> Option<NightLightState> {
        unsafe {
            let key_path: Vec<u16> = NIGHT_LIGHT_KEY.encode_utf16().chain(std::iter::once(0)).collect();
            let value_name: Vec<u16> = "Data".encode_utf16().chain(std::iter::once(0)).collect();
            
            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key_path.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            );
            
            if result.is_err() {
                log::debug!("NightLight: failed to open registry key: {:?}", result);
                return None;
            }

            // First, get the size of the data
            let mut data_size: u32 = 0;
            let mut data_type = REG_VALUE_TYPE::default();
            let rc = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );

            if rc.is_err() || data_size == 0 {
                log::debug!("NightLight: failed to query value size or empty (rc={:?}, size={})", rc, data_size);
                let _ = RegCloseKey(hkey);
                return None;
            }

            // Read the data
            let mut data = vec![0u8; data_size as usize];
            let rc2 = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                Some(data.as_mut_ptr()),
                Some(&mut data_size),
            );

            let _ = RegCloseKey(hkey);

            if rc2.is_err() {
                log::debug!("NightLight: failed to read value (rc={:?})", rc2);
                return None;
            }

            // Debug: log a short hex dump of the first 32 bytes
            let sample_len = data.len().min(32);
            let hex: String = data.iter().take(sample_len).map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            log::debug!("NightLight: data_size={}, sample=[{}]", data.len(), hex);

            // The Night Light state is indicated by the presence of certain bytes
            // When enabled, byte at offset 18 is typically 0x15, when disabled it's 0x13
            // Or we look for the pattern - if data length > 24 and byte[23] == 0x10, it's ON
            if data.len() > 23 {
                // Check for the enable flag - byte at position 18 indicates state
                if data.len() > 18 && (data[18] == 0x15 || (data.len() > 23 && data[23] == 0x10)) {
                    log::debug!("NightLight: detected state=On (byte18={:02X}, byte23={})", data[18], if data.len() > 23 { data[23] } else { 0 });
                    return Some(NightLightState::On);
                }
            }
            log::debug!("NightLight: detected state=Off");
            Some(NightLightState::Off)
        }
    }

    /// Toggle Night Light by modifying the registry; fall back to PowerShell UI automation
    pub fn toggle(&mut self) {
        // For UI thread safety, perform the system toggle in a background thread and
        // notify the UI thread to refresh the module state when complete.
        log::info!("NightLight: user clicked toggle; scheduling background toggle");

        std::thread::spawn(|| {
            let ok = Self::toggle_system_native();
            log::info!("NightLight: background toggle completed -> {}", ok);

            // Notify main window to refresh and redraw (asynchronous)
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    windows::Win32::UI::WindowsAndMessaging::HWND_BROADCAST,
                    crate::window::WM_TOPBAR_NIGHTLIGHT_TOGGLED,
                    windows::Win32::Foundation::WPARAM(if ok { 1 } else { 0 }),
                    windows::Win32::Foundation::LPARAM(0),
                );
            }
        });

        self.last_update = Instant::now();
    }

    /// Set Night Light enabled/disabled via registry
    fn set_night_light_enabled(enable: bool) -> bool {
        unsafe {
            let key_path: Vec<u16> = NIGHT_LIGHT_KEY.encode_utf16().chain(std::iter::once(0)).collect();
            let value_name: Vec<u16> = "Data".encode_utf16().chain(std::iter::once(0)).collect();
            
            let mut hkey = windows::Win32::System::Registry::HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key_path.as_ptr()),
                0,
                KEY_READ | KEY_WRITE,
                &mut hkey,
            );
            
            if result.is_err() {
                log::warn!("NightLight: failed to open registry key: {:?}", result);
                return false;
            }

            // Get current data
            let mut data_size: u32 = 0;
            let mut data_type = REG_VALUE_TYPE::default();
            let rc = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );

            if rc.is_err() || data_size == 0 {
                log::warn!("NightLight: no registry data or failed to query (rc={:?}, size={})", rc, data_size);
                let _ = RegCloseKey(hkey);
                return Self::create_night_light_data(enable);
            }

            let mut data = vec![0u8; data_size as usize];
            let rc2 = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                Some(data.as_mut_ptr()),
                Some(&mut data_size),
            );

            if rc2.is_err() {
                let _ = RegCloseKey(hkey);
                log::warn!("NightLight: failed to read registry value (rc={:?})", rc2);
                return false;
            }

            log::debug!("NightLight: read {} bytes from registry", data.len());

            // Modify the data to toggle the state
            // The Night Light data structure has the enable flag around byte 18
            // We need to modify it carefully
            if data.len() >= 43 {
                if enable {
                    // Set byte 18 to 0x15 (might be enabled with schedule) 
                    // and ensure byte 23 is 0x10 (enabled)
                    if data.len() > 18 {
                        data[18] = 0x15;
                    }
                    // Insert or modify the enabled marker
                    if data.len() > 23 {
                        data[23] = 0x10;
                    }
                } else {
                    // Set byte 18 to 0x13 (disabled)
                    if data.len() > 18 {
                        data[18] = 0x13;
                    }
                    // Remove enabled marker
                    if data.len() > 23 {
                        data[23] = 0x00;
                    }
                }

                // Update timestamp bytes (bytes 10-17 are timestamp)
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // Windows uses FILETIME (100-nanosecond intervals since 1601)
                let filetime = (now + 11644473600) * 10000000;
                if data.len() >= 18 {
                    data[10..18].copy_from_slice(&filetime.to_le_bytes());
                }
            }

            // Write modified data back
            log::debug!("NightLight: writing {} bytes back to registry", data.len());
            let result = RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                0,
                REG_BINARY,
                Some(&data),
            );

            let _ = RegCloseKey(hkey);

            if result.is_err() {
                log::warn!("NightLight: failed to write registry data (rc={:?})", result);
                return false;
            }

            log::info!("NightLight: registry updated (enabled={})", enable);

            // Broadcast settings change to apply immediately
            Self::broadcast_settings_change();

            true
        }
    }

    /// Create initial Night Light registry data
    fn create_night_light_data(_enable: bool) -> bool {
        // This creates a minimal Night Light data structure
        // The full implementation would require understanding the complete binary format
        // For now, we fall back to opening settings if we can't modify the registry
        log::info!("Night Light registry key doesn't exist, opening settings");
        crate::utils::open_url("ms-settings:nightlight");
        false
    }

    /// Broadcast a settings change message to make Windows apply the change immediately
    fn broadcast_settings_change() {
        use windows::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, WM_SETTINGCHANGE,
        };
        use windows::Win32::Foundation::WPARAM;
        
        unsafe {
            // Post the WM_SETTINGCHANGE asynchronously to avoid re-entrancy while we hold renderer borrows
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    }

    /// Get the current state
    pub fn state(&self) -> NightLightState {
        self.state
    }

    /// Force refresh the state
    pub fn refresh(&mut self) {
        self.refresh_state();
    }

    /// Toggle Night Light using system methods (registry + PowerShell fallback)
    /// This is a static helper that can be called from a worker thread.
    pub fn toggle_system_native() -> bool {
        // Read current state
        let current = Self::read_night_light_state().unwrap_or(NightLightState::Unknown);
        let target = match current {
            NightLightState::On => false,
            NightLightState::Off => true,
            NightLightState::Unknown => true,
        };

        log::info!("NightLight.system: current={:?}, target={}", current, target);

        let mut applied = false;

        if Self::set_night_light_enabled(target) {
            std::thread::sleep(std::time::Duration::from_millis(400));
            let after = Self::read_night_light_state().unwrap_or(NightLightState::Unknown);
            if (target && after == NightLightState::On) || (!target && after == NightLightState::Off) {
                applied = true;
                log::info!("NightLight.system: registry write applied -> {:?}", after);
            }
        }

        if !applied {
            log::info!("NightLight.system: trying PowerShell fallback");
            let ps_ok = crate::utils::toggle_night_light_via_powershell();
            if ps_ok {
                std::thread::sleep(std::time::Duration::from_millis(700));
                let after = Self::read_night_light_state().unwrap_or(NightLightState::Unknown);
                if (target && after == NightLightState::On) || (!target && after == NightLightState::Off) {
                    applied = true;
                    log::info!("NightLight.system: PowerShell applied -> {:?}", after);
                } else {
                    log::info!("NightLight.system: PowerShell reported success but state unchanged -> {:?}", after);
                }
            }
        }

        if !applied {
            log::info!("NightLight.system: falling back to opening Settings UI");
            crate::utils::open_url("ms-settings:nightlight");
        }

        applied
    }
}

impl Default for NightLightModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for NightLightModule {
    fn id(&self) -> &str {
        "night_light"
    }

    fn name(&self) -> &str {
        "Night Light"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        match self.state {
            NightLightState::On => "🌙".to_string(),  // Moon when night light is ON
            NightLightState::Off => "☀".to_string(), // Sun when night light is OFF  
            NightLightState::Unknown => "🌓".to_string(), // Half moon when unknown
        }
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Refresh state periodically (every 5 seconds)
        if self.last_update.elapsed().as_secs() > 5 {
            self.refresh_state();
        }
    }

    fn on_click(&mut self) {
        self.toggle();
    }

    fn on_right_click(&mut self) {
        // Right-click opens settings for fine control
        crate::utils::open_url("ms-settings:nightlight");
    }

    fn tooltip(&self) -> Option<String> {
        let state_text = match self.state {
            NightLightState::On => "ON",
            NightLightState::Off => "OFF",
            NightLightState::Unknown => "Unknown",
        };
        Some(format!("Night Light: {}\nClick to toggle\nRight-click for settings", state_text))
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
