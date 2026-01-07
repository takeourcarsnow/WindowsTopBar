//! Bluetooth module - shows Bluetooth status

#![allow(dead_code)]

use std::time::Instant;
use log::debug;

use super::Module;

/// Bluetooth state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BluetoothState {
    Off,
    On,
    Connected,
    Unavailable,
}

/// Bluetooth module
pub struct BluetoothModule {
    cached_text: String,
    state: BluetoothState,
    connected_devices: Vec<String>,
    last_update: Instant,
}

impl BluetoothModule {
    pub fn new() -> Self {
        let mut module = Self {
            cached_text: String::new(),
            state: BluetoothState::Unavailable,
            connected_devices: Vec::new(),
            last_update: Instant::now(),
        };
        module.force_update();
        module
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        self.query_bluetooth_status();
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Query Bluetooth status using Windows APIs
    fn query_bluetooth_status(&mut self) {
        // Check if Bluetooth radio exists and is enabled
        // Using SetupAPI and BluetoothAPIs
        
        use windows::Win32::Devices::Bluetooth::{
            BluetoothFindFirstRadio, BluetoothFindRadioClose,
            BluetoothIsConnectable, BLUETOOTH_FIND_RADIO_PARAMS,
        };
        use windows::Win32::Foundation::HANDLE;

        unsafe {
            let params = BLUETOOTH_FIND_RADIO_PARAMS {
                dwSize: std::mem::size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
            };
            
            let mut radio_handle = HANDLE::default();
            let find_handle = BluetoothFindFirstRadio(&params, &mut radio_handle);
            
            if let Ok(handle) = find_handle {
                // Bluetooth radio found
                if BluetoothIsConnectable(radio_handle).as_bool() {
                    // Check if any devices are connected
                    let connected = self.check_connected_devices();
                    if connected > 0 {
                        self.state = BluetoothState::Connected;
                    } else {
                        self.state = BluetoothState::On;
                    }
                } else {
                    self.state = BluetoothState::Off;
                }
                
                let _ = windows::Win32::Foundation::CloseHandle(radio_handle);
                let _ = BluetoothFindRadioClose(handle);
            } else {
                self.state = BluetoothState::Unavailable;
            }
        }
    }

    /// Check for connected Bluetooth devices
    fn check_connected_devices(&mut self) -> usize {
        self.connected_devices.clear();

        use windows::Win32::Devices::Bluetooth::{
            BluetoothFindFirstDevice, BluetoothFindNextDevice, BluetoothFindDeviceClose,
            BLUETOOTH_DEVICE_SEARCH_PARAMS, BLUETOOTH_DEVICE_INFO,
        };
        use windows::Win32::Foundation::BOOL;

        unsafe {
            // Prepare search params to retrieve connected/remembered/authenticated devices
            let mut search_params: BLUETOOTH_DEVICE_SEARCH_PARAMS = std::mem::zeroed();
            search_params.dwSize = std::mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32;
            search_params.fReturnAuthenticated = BOOL(1);
            search_params.fReturnRemembered = BOOL(1);
            search_params.fReturnUnknown = BOOL(1);
            search_params.fReturnConnected = BOOL(1);
            search_params.fIssueInquiry = BOOL(0);
            search_params.cTimeoutMultiplier = 0;

            let mut device_info: BLUETOOTH_DEVICE_INFO = std::mem::zeroed();
            device_info.dwSize = std::mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;

            if let Ok(handle) = BluetoothFindFirstDevice(&search_params, &mut device_info) {
                let mut current = device_info;
                loop {
                    // fConnected is a flag indicating current connection state
                    let connected = current.fConnected.0 != 0;

                    if connected {
                        // Convert UTF-16 name buffer to Rust String
                        let name = {
                            let raw: &[u16] = &current.szName;
                            let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                            String::from_utf16_lossy(&raw[..len])
                        };
                        self.connected_devices.push(name);
                    }

                    if BluetoothFindNextDevice(handle, &mut current).is_err() {
                        break;
                    }
                }

                let _ = BluetoothFindDeviceClose(handle);
            }
        }

        self.connected_devices.len()
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        match self.state {
            BluetoothState::Off => "󰂲".to_string(),      // Bluetooth off icon
            BluetoothState::On => "󰂯".to_string(),       // Bluetooth on icon
            BluetoothState::Connected => {
                if self.connected_devices.len() == 1 {
                    format!("󰂱 {}", self.connected_devices[0])
                } else if self.connected_devices.len() > 1 {
                    format!("󰂱 {}+", self.connected_devices.len())
                } else {
                    "󰂱".to_string()  // Connected icon
                }
            }
            BluetoothState::Unavailable => String::new(),
        }
    }

    /// Get Bluetooth state
    pub fn state(&self) -> BluetoothState {
        self.state
    }

    /// Get connected device names
    pub fn connected_devices(&self) -> &[String] {
        &self.connected_devices
    }

    /// Toggle Bluetooth
    pub fn toggle(&mut self) {
        // Open Bluetooth settings - actual toggle requires admin privileges
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:bluetooth"])
            .spawn();
    }

    /// Force an immediate refresh (used by device-change notifications)
    pub fn refresh(&mut self) {
        debug!("BluetoothModule: manual refresh triggered");
        self.force_update();
    }
}

impl Default for BluetoothModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BluetoothModule {
    fn id(&self) -> &str {
        "bluetooth"
    }

    fn name(&self) -> &str {
        "Bluetooth"
    }

    fn display_text(&self, config: &crate::config::Config) -> String {
        // Use emoji as fallback for better compatibility
        match self.state {
            BluetoothState::Off => "🔵○".to_string(),
            BluetoothState::On => "🔵".to_string(),
            BluetoothState::Connected => {
                let count = self.connected_devices.len();
                if count > 0 && config.modules.bluetooth.show_device_count {
                    format!("🔵 {}", count)
                } else {
                    "🔵●".to_string()
                }
            }
            BluetoothState::Unavailable => String::new(),
        }
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Update every 10 seconds
        if self.last_update.elapsed().as_secs() >= 10 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        self.toggle();
    }

    fn on_right_click(&mut self) {
        // Open Bluetooth devices
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:connecteddevices"])
            .spawn();
    }

    fn tooltip(&self) -> Option<String> {
        match self.state {
            BluetoothState::Off => Some("Bluetooth: Off\nClick to open settings".to_string()),
            BluetoothState::On => Some("Bluetooth: On\nNo devices connected".to_string()),
            BluetoothState::Connected => {
                let devices = if self.connected_devices.is_empty() {
                    "Unknown device".to_string()
                } else {
                    self.connected_devices.join(", ")
                };
                Some(format!("Bluetooth: Connected\n{}", devices))
            }
            BluetoothState::Unavailable => None,
        }
    }

    fn is_visible(&self) -> bool {
        self.state != BluetoothState::Unavailable
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
