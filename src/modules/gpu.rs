//! GPU module for displaying GPU usage and temperature

use std::time::Instant;

use super::Module;

/// GPU information
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub name: String,
    pub usage: f32,       // 0-100%
    pub memory_used: u64, // bytes
    pub memory_total: u64,// bytes
    pub temperature: Option<f32>, // Celsius
}

/// GPU module
pub struct GpuModule {
    cached_text: String,
    gpu_info: GpuInfo,
    show_usage: bool,
    show_memory: bool,
    show_temp: bool,
    last_update: Instant,
    update_interval_ms: u64,
}

impl GpuModule {
    pub fn new() -> Self {
        let mut module = Self {
            cached_text: String::new(),
            gpu_info: GpuInfo::default(),
            show_usage: true,
            show_memory: false,
            show_temp: false,
            last_update: Instant::now(),
            update_interval_ms: 2000,
        };
        module.force_update();
        module
    }

    /// Set whether to show GPU usage
    pub fn set_show_usage(&mut self, show: bool) {
        self.show_usage = show;
    }

    /// Set whether to show memory usage
    pub fn set_show_memory(&mut self, show: bool) {
        self.show_memory = show;
    }

    /// Set whether to show temperature
    pub fn set_show_temp(&mut self, show: bool) {
        self.show_temp = show;
    }

    /// Set update interval
    pub fn set_update_interval(&mut self, interval_ms: u64) {
        self.update_interval_ms = interval_ms;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        self.query_gpu_info();
        self.cached_text = self.build_display_text();
        self.last_update = Instant::now();
    }

    /// Query GPU information using Windows APIs
    fn query_gpu_info(&mut self) {
        // First try PDH for usage
        if !self.query_d3dkmt_info() {
            // If PDH fails, at least get GPU names via DXGI
            self.query_dxgi_adapter_info();
        }
    }

    /// Query D3DKMT for GPU information
    fn query_d3dkmt_info(&mut self) -> bool {
        // D3DKMT APIs require linking to gdi32.dll dynamically
        // This is a simplified approach using performance counters
        
        use windows::Win32::System::Performance::{
            PdhOpenQueryW, PdhAddEnglishCounterW, PdhCollectQueryData,
            PdhGetFormattedCounterValue, PDH_FMT_DOUBLE, PDH_FMT_COUNTERVALUE,
        };
        use windows::core::PCWSTR;

        unsafe {
            let mut query = 0isize;
            let status = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
            if status != 0 {
                return false;
            }

            // Try multiple GPU Engine utilization counters
            let counter_paths = [
                "\\GPU Engine(*)\\Utilization Percentage",
                "\\GPU Engine(pid_*)\\Utilization Percentage",
                "\\GPU Engine(*)\\Utilization Percentage",
            ];

            for counter_path in &counter_paths {
                let counter_path_wide = crate::utils::to_wide_string(counter_path);
                let mut counter = 0isize;
                let status = PdhAddEnglishCounterW(
                    query,
                    PCWSTR(counter_path_wide.as_ptr()),
                    0,
                    &mut counter,
                );

                if status == 0 {
                    // Collect data
                    let _ = PdhCollectQueryData(query);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let _ = PdhCollectQueryData(query);

                    let mut value = PDH_FMT_COUNTERVALUE::default();
                    if PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, None, &mut value) == 0 {
                        self.gpu_info.usage = value.Anonymous.doubleValue as f32;
                        // Close query
                        let _ = windows::Win32::System::Performance::PdhCloseQuery(query);
                        return true;
                    }
                }
            }

            // If all counters failed, use fallback
            self.gpu_info.usage = self.estimate_gpu_usage();
            
            // Close query
            let _ = windows::Win32::System::Performance::PdhCloseQuery(query);
            false
        }
    }

    /// Estimate GPU usage from system metrics
    fn estimate_gpu_usage(&self) -> f32 {
        // For usage estimation, we can't easily get real-time usage without PDH
        // Return 0 for now
        0.0
    }

    /// Query GPU adapter info using DXGI
    fn query_dxgi_adapter_info(&mut self) {
        use windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIFactory1,
        };

        unsafe {
            let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
                Ok(f) => f,
                Err(_) => return,
            };

            for i in 0.. {
                let adapter = match factory.EnumAdapters1(i) {
                    Ok(a) => a,
                    Err(_) => break,
                };

                if let Ok(desc) = adapter.GetDesc1() {
                    // Convert the description to a string
                    let name = String::from_utf16_lossy(&desc.Description);
                    let name = name.trim_end_matches('\0').to_string();
                    
                    if self.gpu_info.name.is_empty() {
                        self.gpu_info.name = name;
                    }
                    
                    if self.gpu_info.memory_total == 0 {
                        self.gpu_info.memory_total = desc.DedicatedVideoMemory as u64;
                    }
                }
            }
        }
    }

    /// Build the display text
    fn build_display_text(&self) -> String {
        let mut parts = Vec::new();

        if self.show_usage {
            parts.push(format!("GPU {:.0}%", self.gpu_info.usage));
        }

        if self.show_memory && self.gpu_info.memory_total > 0 {
            let mem_percent = (self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64 * 100.0) as u32;
            parts.push(format!("VRAM {}%", mem_percent));
        }

        if self.show_temp {
            if let Some(temp) = self.gpu_info.temperature {
                parts.push(format!("{:.0}°C", temp));
            }
        }

        if parts.is_empty() {
            "GPU".to_string()
        } else {
            parts.join("  ")
        }
    }

    /// Get GPU usage percentage
    pub fn gpu_usage(&self) -> f32 {
        self.gpu_info.usage
    }

    /// Get GPU memory usage percentage
    pub fn memory_usage(&self) -> f32 {
        if self.gpu_info.memory_total > 0 {
            (self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64 * 100.0) as f32
        } else {
            0.0
        }
    }

    /// Get GPU temperature
    pub fn temperature(&self) -> Option<f32> {
        self.gpu_info.temperature
    }
}

impl Default for GpuModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GpuModule {
    fn id(&self) -> &str {
        "gpu"
    }

    fn name(&self) -> &str {
        "GPU"
    }

    fn display_text(&self, config: &crate::config::Config) -> String {
        let mut parts = Vec::new();

        if config.modules.gpu.show_usage {
            parts.push(format!("GPU {:.0}%", self.gpu_info.usage));
        }

        if config.modules.gpu.show_memory && self.gpu_info.memory_total > 0 {
            let mem_percent = (self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64 * 100.0) as u32;
            parts.push(format!("VRAM {}%", mem_percent));
        }

        if config.modules.gpu.show_temperature {
            if let Some(temp) = self.gpu_info.temperature {
                parts.push(format!("{:.0}°C", temp));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join("  ")
        }
    }

    fn update(&mut self) {
        if self.last_update.elapsed().as_millis() >= self.update_interval_ms as u128 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Open task manager Performance tab
        let _ = std::process::Command::new("taskmgr.exe")
            .args(["/7"])
            .spawn();
    }

    fn tooltip(&self) -> Option<String> {
        let mut lines = vec![format!("GPU Usage: {:.1}%", self.gpu_info.usage)];
        
        if self.gpu_info.memory_total > 0 {
            lines.push(format!(
                "VRAM: {} / {}",
                crate::utils::format_bytes(self.gpu_info.memory_used),
                crate::utils::format_bytes(self.gpu_info.memory_total)
            ));
        }

        if let Some(temp) = self.gpu_info.temperature {
            lines.push(format!("Temperature: {:.0}°C", temp));
        }

        if !self.gpu_info.name.is_empty() {
            lines.push(format!("Device: {}", self.gpu_info.name));
        }

        Some(lines.join("\n"))
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
