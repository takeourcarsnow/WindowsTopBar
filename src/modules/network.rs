//! Network module for displaying network status

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
            last_update: Instant::now(),
        };
        module.force_update();
        module
    }

    /// Set whether to show icon
    pub fn set_show_icon(&mut self, show: bool) {
        self.show_icon = show;
    }

    /// Set whether to show network name
    pub fn set_show_name(&mut self, show: bool) {
        self.show_name = show;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        // Check network connectivity
        self.check_network_status();
        
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
                                }
                                71 => {
                                    self.network_type = NetworkType::WiFi;
                                    self.is_connected = true;
                                    self.signal_strength = 75;  // Default, would need WiFi API for actual
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
        if self.signal_strength >= 75 {
            "📶"
        } else if self.signal_strength >= 50 {
            "📶"
        } else if self.signal_strength >= 25 {
            "📶"
        } else {
            "📶"
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

    fn update(&mut self) {
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
            tooltip.push_str(&format!("\nSignal: {}%", self.signal_strength));
        }

        if let Some(ref name) = self.network_name {
            tooltip.push_str(&format!("\nConnected to: {}", name));
        }

        Some(tooltip)
    }
}
