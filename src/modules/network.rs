//! Network module for displaying network status with real WiFi info

#![allow(dead_code)]

use std::time::Instant;

use super::Module;

/// Network connection type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkType {
    Disconnected,
    Ethernet,
    WiFi,
    Cellular,
    Unknown,
}

/// Network module
pub struct NetworkModule {
    show_icon: bool,
    show_name: bool,
    show_speed: bool,
    cached_text: String,
    network_type: NetworkType,
    network_name: Option<String>,
    signal_strength: u32,  // 0-100 for WiFi
    is_connected: bool,
    download_speed: u64,   // bytes per second
    upload_speed: u64,     // bytes per second
    last_update: Instant,
}

impl NetworkModule {
    pub fn new() -> Self {
        let mut module = Self {
            show_icon: true,
            show_name: false,
            show_speed: false,
            cached_text: String::new(),
            network_type: NetworkType::Unknown,
            network_name: None,
            signal_strength: 0,
            is_connected: false,
            download_speed: 0,
            upload_speed: 0,
            last_update: Instant::now(),
        };
        module.force_update();
        module
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        // Check network connectivity
        self.check_network_status();
        
        // Try to get WiFi info if connected via WiFi
        if self.network_type == NetworkType::WiFi {
            self.get_wifi_info();
        }
        
        // Build display text
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Check network status using Windows API
    fn check_network_status(&mut self) {
        // Simple connectivity check using IP helper
        unsafe {
            use windows::Win32::NetworkManagement::IpHelper::{
                GetAdaptersAddresses, GAA_FLAG_INCLUDE_PREFIX,
                IP_ADAPTER_ADDRESSES_LH,
            };
            use windows::Win32::Networking::WinSock::AF_UNSPEC;
            use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;

            // First call to get required buffer size
            let mut size: u32 = 0;
            let result = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_INCLUDE_PREFIX,
                None,
                None,
                &mut size,
            );

            if result == ERROR_BUFFER_OVERFLOW.0 {
                let mut buffer = vec![0u8; size as usize];
                let addresses = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;

                let result = GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    GAA_FLAG_INCLUDE_PREFIX,
                    None,
                    Some(addresses),
                    &mut size,
                );

                if result == 0 {
                    let mut current = addresses;
                    while !current.is_null() {
                        let adapter = &*current;
                        
                        // Check if adapter is up and connected
                        // IfType: 6 = Ethernet, 71 = WiFi
                        if adapter.OperStatus.0 == 1 {  // IfOperStatusUp
                            match adapter.IfType {
                                6 => {
                                    self.network_type = NetworkType::Ethernet;
                                    self.is_connected = true;
                                    // Don't break - prefer WiFi if available
                                }
                                71 => {
                                    self.network_type = NetworkType::WiFi;
                                    self.is_connected = true;
                                    break; // WiFi found, stop looking
                                }
                                _ => {}
                            }
                        }
                        
                        current = adapter.Next;
                    }
                }
            }
        }

        // If no connected adapter found
        if !self.is_connected {
            self.network_type = NetworkType::Disconnected;
        }
    }

    /// Get WiFi information using WLAN API
    fn get_wifi_info(&mut self) {
        use windows::Win32::NetworkManagement::WiFi::{
            WlanOpenHandle, WlanCloseHandle, WlanEnumInterfaces, WlanQueryInterface,
            WlanFreeMemory, WLAN_INTERFACE_INFO_LIST, WLAN_CONNECTION_ATTRIBUTES,
            wlan_intf_opcode_current_connection, wlan_interface_state_connected,
        };
        use windows::Win32::Foundation::HANDLE;
        
        unsafe {
            let mut client_handle = HANDLE::default();
            let mut negotiated_version = 0u32;
            
            // Open WLAN handle
            if WlanOpenHandle(2, None, &mut negotiated_version, &mut client_handle) != 0 {
                return;
            }
            
            // Enumerate interfaces
            let mut interface_list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            if WlanEnumInterfaces(client_handle, None, &mut interface_list) != 0 {
                let _ = WlanCloseHandle(client_handle, None);
                return;
            }
            
            if !interface_list.is_null() {
                let list = &*interface_list;
                
                // Check each interface
                for i in 0..list.dwNumberOfItems {
                    let interface_info = &list.InterfaceInfo[i as usize];
                    
                    if interface_info.isState == wlan_interface_state_connected {
                        // Get connection attributes
                        let mut data_size = 0u32;
                        let mut connection_attrs: *mut WLAN_CONNECTION_ATTRIBUTES = std::ptr::null_mut();
                        let mut opcode_value_type = windows::Win32::NetworkManagement::WiFi::WLAN_OPCODE_VALUE_TYPE::default();
                        
                        if WlanQueryInterface(
                            client_handle,
                            &interface_info.InterfaceGuid,
                            wlan_intf_opcode_current_connection,
                            None,
                            &mut data_size,
                            &mut connection_attrs as *mut _ as *mut *mut std::ffi::c_void,
                            Some(&mut opcode_value_type),
                        ) == 0 && !connection_attrs.is_null() {
                            let attrs = &*connection_attrs;
                            
                            // Get SSID
                            let ssid_len = attrs.wlanAssociationAttributes.dot11Ssid.uSSIDLength as usize;
                            if ssid_len > 0 {
                                let ssid_bytes = &attrs.wlanAssociationAttributes.dot11Ssid.ucSSID[..ssid_len];
                                self.network_name = Some(String::from_utf8_lossy(ssid_bytes).to_string());
                            }
                            
                            // Get signal quality (0-100)
                            self.signal_strength = attrs.wlanAssociationAttributes.wlanSignalQuality as u32;
                            
                            WlanFreeMemory(connection_attrs as *mut std::ffi::c_void);
                        }
                    }
                }
                
                WlanFreeMemory(interface_list as *mut std::ffi::c_void);
            }
            
            let _ = WlanCloseHandle(client_handle, None);
        }
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        let mut text = String::new();

        if self.show_icon {
            let icon = match self.network_type {
                NetworkType::Disconnected => "📵",
                NetworkType::Ethernet => "🔗",
                NetworkType::WiFi => self.get_wifi_icon(),
                NetworkType::Cellular => "📶",
                NetworkType::Unknown => "🌐",
            };
            text.push_str(icon);
        }

        if self.show_name {
            if let Some(ref name) = self.network_name {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(name);
            }
        }

        text
    }

    /// Get WiFi icon based on signal strength
    fn get_wifi_icon(&self) -> &'static str {
        if self.signal_strength >= 20 {
            "📶"  // Connected
        } else {
            "📵"  // Very weak / disconnected
        }
    }

    /// Get network type
    pub fn network_type(&self) -> NetworkType {
        self.network_type
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    /// Get signal strength (0-100)
    pub fn signal_strength(&self) -> u32 {
        self.signal_strength
    }

    /// Get network name (SSID for WiFi)
    pub fn network_name(&self) -> Option<&str> {
        self.network_name.as_deref()
    }
}

impl Default for NetworkModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for NetworkModule {
    fn id(&self) -> &str {
        "network"
    }

    fn name(&self) -> &str {
        "Network"
    }

    fn display_text(&self, config: &crate::config::Config) -> String {
        let mut text = String::new();

        if self.show_icon {
            let icon = match self.network_type {
                NetworkType::Disconnected => "📵",
                NetworkType::Ethernet => "🔗",
                NetworkType::WiFi => self.get_wifi_icon(),
                NetworkType::Cellular => "📶",
                NetworkType::Unknown => "🌐",
            };
            text.push_str(icon);
        }

        if config.modules.network.show_name {
            if let Some(ref name) = self.network_name {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(name);
            }
        }

        text
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Update every 10 seconds
        if self.last_update.elapsed().as_secs() >= 10 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Open network settings
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:network"])
            .spawn();
    }

    fn tooltip(&self) -> Option<String> {
        let type_str = match self.network_type {
            NetworkType::Disconnected => "Not connected",
            NetworkType::Ethernet => "Ethernet",
            NetworkType::WiFi => "Wi-Fi",
            NetworkType::Cellular => "Cellular",
            NetworkType::Unknown => "Unknown",
        };

        let mut tooltip = format!("Network: {}", type_str);

        if self.network_type == NetworkType::WiFi {
            tooltip.push_str(&format!("\nSignal Strength: {}%", self.signal_strength));
            
            // Signal quality description
            let quality = if self.signal_strength >= 80 {
                "Excellent"
            } else if self.signal_strength >= 60 {
                "Good"
            } else if self.signal_strength >= 40 {
                "Fair"
            } else {
                "Weak"
            };
            tooltip.push_str(&format!(" ({})", quality));
        }

        if let Some(ref name) = self.network_name {
            tooltip.push_str(&format!("\nNetwork: {}", name));
        }

        Some(tooltip)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
