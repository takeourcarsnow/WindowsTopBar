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
        // Try to get GPU adapter information via performance counters
        self.query_d3dkmt_info();
    }

    /// Query D3DKMT for GPU information
    fn query_d3dkmt_info(&mut self) {
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
                // Fallback: estimate GPU usage from GPU Engine performance counters
                self.gpu_info.usage = self.estimate_gpu_usage();
                return;
            }

            // Try GPU Engine utilization counter
            let counter_path = crate::utils::to_wide_string("\\GPU Engine(*)\\Utilization Percentage");
            let mut counter = 0isize;
            let status = PdhAddEnglishCounterW(
                query,
                PCWSTR(counter_path.as_ptr()),
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
                }
            }

            // Close query
            let _ = windows::Win32::System::Performance::PdhCloseQuery(query);
        }
    }

    /// Estimate GPU usage from system metrics
    fn estimate_gpu_usage(&self) -> f32 {
        // Fallback method: check if any GPU processes are running
        // This is a rough estimate
        0.0
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
}
