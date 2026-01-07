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
    signal_strength: u32, // 0-100 for WiFi
    is_connected: bool,
    download_speed: u64, // bytes per second
    upload_speed: u64,   // bytes per second
    prev_total_in: u64,  // cumulative octets seen at last sample
    prev_total_out: u64, // cumulative octets seen at last sample
    last_update: Instant,
    last_speed_update: Instant,
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
            prev_total_in: 0,
            prev_total_out: 0,
            last_update: Instant::now(),
            last_speed_update: Instant::now(),
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

        // Initialize speed sampling to avoid a huge first delta
        if let Some((total_in, total_out)) = self.sample_total_bytes() {
            self.prev_total_in = total_in;
            self.prev_total_out = total_out;
            self.download_speed = 0;
            self.upload_speed = 0;
            self.last_speed_update = Instant::now();
        }

        // Build display text
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();

        // Temporary debug: append current network state to file to help diagnose display issues
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("network_debug.log") {
            use std::io::Write;
            let _ = writeln!(
                f,
                "{:?} connected={} name={:?} signal={} speeds={:?}\n",
                self.network_type,
                self.is_connected,
                self.network_name,
                self.signal_strength,
                (self.download_speed, self.upload_speed)
            );
        }
    }

    /// Try to sample total interface bytes (received, transmitted) across adapters
    fn sample_total_bytes(&self) -> Option<(u64, u64)> {
        unsafe {
            use windows::Win32::NetworkManagement::IpHelper::{
                FreeMibTable, GetIfTable2, MIB_IF_TABLE2,
            };

            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table).0 == 0 && !table.is_null() {
                let tbl = &*table;
                let mut total_in: u64 = 0;
                let mut total_out: u64 = 0;
                for i in 0..(tbl.NumEntries as usize) {
                    let row = &*(&tbl.Table as *const _
                        as *const windows::Win32::NetworkManagement::IpHelper::MIB_IF_ROW2)
                        .add(i);
                    total_in = total_in.saturating_add(row.InOctets);
                    total_out = total_out.saturating_add(row.OutOctets);
                }
                FreeMibTable(table as *mut _);
                return Some((total_in, total_out));
            }
        }
        None
    }

    /// Update upload/download speeds by sampling interface counters and computing deltas
    fn update_speeds(&mut self) {
        if let Some((total_in, total_out)) = self.sample_total_bytes() {
            let elapsed = self.last_speed_update.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let delta_in = total_in.saturating_sub(self.prev_total_in);
                let delta_out = total_out.saturating_sub(self.prev_total_out);
                self.download_speed = (delta_in as f64 / elapsed) as u64; // bytes/sec
                self.upload_speed = (delta_out as f64 / elapsed) as u64;
            }

            self.prev_total_in = total_in;
            self.prev_total_out = total_out;
            self.last_speed_update = Instant::now();
        }
    }

    /// Check network status using Windows API
    fn check_network_status(&mut self) {
        // Reset state before scanning
        self.is_connected = false;
        self.network_type = NetworkType::Unknown;

        // Simple connectivity check using IP helper
        unsafe {
            use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
            use windows::Win32::NetworkManagement::IpHelper::{
                GetAdaptersAddresses, GAA_FLAG_INCLUDE_PREFIX, IP_ADAPTER_ADDRESSES_LH,
            };
            use windows::Win32::Networking::WinSock::AF_UNSPEC;

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

                        // Debug: adapter info
                        log::debug!(
                            "Network adapter found: IfType={}, OperStatus={}, Description={}",
                            adapter.IfType,
                            adapter.OperStatus.0,
                            if adapter.Description.is_null() { "<null>" } else { "<desc>" }
                        );

                        // Check if adapter is up and connected
                        // IfType: 6 = Ethernet, 71 = WiFi
                        if adapter.OperStatus.0 == 1 {
                            // IfOperStatusUp
                            match adapter.IfType {
                                6 => {
                                    self.network_type = NetworkType::Ethernet;
                                    self.is_connected = true;
                                    log::debug!("Adapter is Ethernet and up");
                                    // Don't break - prefer WiFi if available
                                }
                                71 => {
                                    self.network_type = NetworkType::WiFi;
                                    self.is_connected = true;
                                    log::debug!("Adapter is WiFi and up");
                                    break; // WiFi found, stop looking
                                }
                                other => {
                                    log::debug!("Adapter with IfType {} is up (ignored)", other);
                                }
                            }
                        }

                        current = adapter.Next;
                    }
                } else {
                    log::warn!("GetAdaptersAddresses failed with code {}", result);
                }
            } else {
                log::warn!("GetAdaptersAddresses initial call returned {} (expected ERROR_BUFFER_OVERFLOW)", result);
            }
        }

        // If no connected adapter found
        if !self.is_connected {
            self.network_type = NetworkType::Disconnected;
            log::debug!("No connected adapters found; marking as Disconnected");
        }
    }

    /// Get WiFi information using WLAN API
    fn get_wifi_info(&mut self) {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::NetworkManagement::WiFi::{
            wlan_interface_state_connected, wlan_intf_opcode_current_connection, WlanCloseHandle,
            WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WlanQueryInterface,
            WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
        };

        // Clear previous WiFi info by default
        self.network_name = None;
        self.signal_strength = 0;

        unsafe {
            let mut client_handle = HANDLE::default();
            let mut negotiated_version = 0u32;

            // Open WLAN handle
            if WlanOpenHandle(2, None, &mut negotiated_version, &mut client_handle) != 0 {
                log::warn!("WlanOpenHandle failed");
                return;
            }

            // Enumerate interfaces
            let mut interface_list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            if WlanEnumInterfaces(client_handle, None, &mut interface_list) != 0 {
                log::warn!("WlanEnumInterfaces failed");
                let _ = WlanCloseHandle(client_handle, None);
                return;
            }

            if !interface_list.is_null() {
                let list = &*interface_list;

                // Check each interface
                for i in 0..list.dwNumberOfItems {
                    let interface_info = &list.InterfaceInfo[i as usize];

                    log::debug!("WLAN interface {} state={:?} GUID={:?}", i, interface_info.isState, interface_info.InterfaceGuid);

                    if interface_info.isState == wlan_interface_state_connected {
                        log::debug!("WLAN interface {} is connected", i);
                        // Get connection attributes
                        let mut data_size = 0u32;
                        let mut connection_attrs: *mut WLAN_CONNECTION_ATTRIBUTES =
                            std::ptr::null_mut();
                        let mut opcode_value_type = windows::Win32::NetworkManagement::WiFi::WLAN_OPCODE_VALUE_TYPE::default();

                        let res = WlanQueryInterface(
                            client_handle,
                            &interface_info.InterfaceGuid,
                            wlan_intf_opcode_current_connection,
                            None,
                            &mut data_size,
                            &mut connection_attrs as *mut _ as *mut *mut std::ffi::c_void,
                            Some(&mut opcode_value_type),
                        );

                        if res == 0 && !connection_attrs.is_null() {
                            let attrs = &*connection_attrs;

                            // Get SSID
                            let ssid_len =
                                attrs.wlanAssociationAttributes.dot11Ssid.uSSIDLength as usize;
                            log::debug!("WLAN connection SSID length: {}", ssid_len);

                            // Also append to debug file for GUI runs
                            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("network_debug.log") {
                                use std::io::Write;
                                let _ = writeln!(f, "WLAN iface {}: ssid_len={} signal={}", i, ssid_len, attrs.wlanAssociationAttributes.wlanSignalQuality);
                            }

                            if ssid_len > 0 {
                                let ssid_bytes =
                                    &attrs.wlanAssociationAttributes.dot11Ssid.ucSSID[..ssid_len];
                                let ssid = String::from_utf8_lossy(ssid_bytes).to_string();
                                log::debug!("WLAN SSID: {}", ssid);
                                self.network_name = Some(ssid);
                            }

                            // Get signal quality (0-100)
                            self.signal_strength =
                                attrs.wlanAssociationAttributes.wlanSignalQuality;

                            WlanFreeMemory(connection_attrs as *mut std::ffi::c_void);
                        } else {
                            log::debug!("WlanQueryInterface returned error {} or null attrs", res);
                            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("network_debug.log") {
                                use std::io::Write;
                                let _ = writeln!(f, "WlanQueryInterface failed for iface {} with code {}\n", i, res);
                            }
                            // If we receive access denied from WLAN APIs, try a CLI fallback to extract the SSID
                            // This helps when Windows denies access to WLAN APIs for non-elevated apps.
                            const ERROR_ACCESS_DENIED: u32 = 5;
                            if res == ERROR_ACCESS_DENIED {
                                log::debug!("WLAN API access denied; using generic fallback (no netsh).");

                                // Do NOT invoke external CLI tools (netsh) — use a safe generic fallback
                                if self.network_name.is_none() {
                                    self.network_name = Some("Wi-Fi".to_string());
                                }

                                // Ensure a reasonable signal value so the UI shows a connected icon
                                if self.signal_strength == 0 {
                                    self.signal_strength = 50;
                                }
                            }                        }
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
        // Prefer connection status over raw signal when available
        if self.is_connected {
            "📶"
        } else {
            "📵"
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

        // Show speeds in MB/s if enabled
        if config.modules.network.show_speed {
            if !text.is_empty() {
                text.push(' ');
            }
            let down_mb = (self.download_speed as f64) / 1_000_000.0;
            let up_mb = (self.upload_speed as f64) / 1_000_000.0;
            text.push_str(&format!("{:.1}↓/{:.1}↑MB/s", down_mb, up_mb));
        }

        text
    }

    fn update(&mut self, _config: &crate::config::Config) {
        // Update speeds every second
        if self.last_speed_update.elapsed().as_secs() >= 1 {
            self.update_speeds();
        }

        // Full refresh every 10 seconds
        if self.last_update.elapsed().as_secs() >= 10 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Open network settings
        crate::utils::open_url("ms-settings:network");
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

        // Show speeds in tooltip when we have samples
        if self.download_speed > 0 || self.upload_speed > 0 {
            // Use values already sampled; convert to MB/s
            let down_mb = (self.download_speed as f64) / 1_000_000.0;
            let up_mb = (self.upload_speed as f64) / 1_000_000.0;
            tooltip.push_str(&format!(
                "\nSpeed: {down:.2} MB/s down / {up:.2} MB/s up",
                down = down_mb,
                up = up_mb
            ));
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
