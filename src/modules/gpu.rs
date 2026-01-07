//! GPU module for displaying GPU usage and temperature

#![allow(unused_unsafe)]

use std::collections::VecDeque;
use std::time::Instant;

use super::Module;
use windows::core::Interface;

/// GPU information
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub name: String,
    pub usage: f32,               // 0-100%
    pub memory_used: u64,         // bytes
    pub memory_total: u64,        // bytes
    pub temperature: Option<f32>, // Celsius
}

/// GPU module
pub struct GpuModule {
    cached_text: String,
    gpu_info: GpuInfo,
    // histories for moving graphs (percentages or scaled values)
    usage_history: VecDeque<f32>,
    memory_history: VecDeque<f32>,
    history_len: usize,
    last_update: Instant,
    update_interval_ms: u64,
}

impl GpuModule {
    pub fn new() -> Self {
        let mut s = Self {
            cached_text: String::new(),
            gpu_info: GpuInfo::default(),
            usage_history: VecDeque::with_capacity(60),
            memory_history: VecDeque::with_capacity(60),
            history_len: 60,
            last_update: Instant::now(),
            update_interval_ms: 2000,
        };

        // Query once at startup so graphs have an initial meaningful value
        s.query_gpu_info();
        let usage_val = s.gpu_info.usage;
        s.usage_history = VecDeque::from(vec![usage_val; s.history_len]);

        if s.gpu_info.memory_total > 0 {
            let mem_pct = s.memory_usage_percent().unwrap_or(0.0);
            s.memory_history = VecDeque::from(vec![mem_pct; s.history_len]);
        }

        s.cached_text = s.build_display_text(&crate::config::Config::default());

        s
    }

    /// Force an immediate update
    fn force_update(&mut self, config: &crate::config::Config) {
        self.query_gpu_info();

        // Update histories
        self.usage_history.push_back(self.gpu_info.usage);
        if self.usage_history.len() > self.history_len {
            self.usage_history.pop_front();
        }

        if self.gpu_info.memory_total > 0 {
            let mem_pct = (self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64
                * 100.0) as f32;
            self.memory_history.push_back(mem_pct);
            if self.memory_history.len() > self.history_len {
                self.memory_history.pop_front();
            }
        }

        self.cached_text = self.build_display_text(config);
        self.last_update = Instant::now();
    }

    /// Get usage history (oldest to newest)
    pub fn usage_history(&self) -> Vec<f32> {
        self.usage_history.iter().copied().collect()
    }

    /// Get memory history (oldest to newest, percent)
    pub fn memory_history(&self) -> Vec<f32> {
        self.memory_history.iter().copied().collect()
    }

    /// Get current VRAM usage percent if available
    pub fn memory_usage_percent(&self) -> Option<f32> {
        if self.gpu_info.memory_total > 0 {
            Some((self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64 * 100.0) as f32)
        } else {
            None
        }
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

        use windows::core::PCWSTR;
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterW, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
            PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        };

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

                    // Try to get per-instance values via PdhGetFormattedCounterArrayW and sum them
                    use windows::Win32::System::Performance::{
                        PdhGetFormattedCounterArrayW, PDH_FMT_COUNTERVALUE_ITEM_W,
                    };
                    unsafe {
                        let mut buf_size: u32 = 0;
                        let mut item_count: u32 = 0;
                        // First call to get required buffer size
                        let status_array = PdhGetFormattedCounterArrayW(
                            counter,
                            PDH_FMT_DOUBLE,
                            &mut buf_size,
                            &mut item_count,
                            Some(std::ptr::null_mut()),
                        );
                        if status_array == 0 && item_count > 0 {
                            // Shouldn't happen since buffer is null, but handle anyway
                        }

                        if buf_size > 0 {
                            // Allocate buffer
                            let mut buffer: Vec<u8> = vec![0u8; buf_size as usize];
                            let ptr = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                            let status_array2 = PdhGetFormattedCounterArrayW(
                                counter,
                                PDH_FMT_DOUBLE,
                                &mut buf_size,
                                &mut item_count,
                                Some(ptr),
                            );
                            if status_array2 == 0 && item_count > 0 {
                                let mut sum = 0.0f64;
                                for i in 0..item_count as isize {
                                    let item = ptr.offset(i);
                                    let val = (*item).FmtValue.Anonymous.doubleValue;
                                    sum += val;
                                }
                                // Average or clamp to 100
                                let usage = sum.min(100.0) as f32;
                                self.gpu_info.usage = usage;
                                let _ = windows::Win32::System::Performance::PdhCloseQuery(query);
                                return true;
                            }
                        }

                        // Fallback to formatted counter value
                        let mut value = PDH_FMT_COUNTERVALUE::default();
                        if PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, None, &mut value)
                            == 0
                        {
                            self.gpu_info.usage = value.Anonymous.doubleValue as f32;
                            let _ = windows::Win32::System::Performance::PdhCloseQuery(query);
                            return true;
                        }
                    }
                }
            }

            // If all counters failed, use fallback
            self.gpu_info.usage = self.estimate_gpu_usage();

            // Try GPU adapter memory counter as fallback for memory used
            let mem_path = crate::utils::to_wide_string("\\GPU Adapter Memory(*)\\Dedicated Bytes");
            let mut mem_counter = 0isize;
            if PdhAddEnglishCounterW(query, PCWSTR(mem_path.as_ptr()), 0, &mut mem_counter) == 0 {
                let _ = PdhCollectQueryData(query);
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = PdhCollectQueryData(query);
                let mut mem_value = PDH_FMT_COUNTERVALUE::default();
                if PdhGetFormattedCounterValue(mem_counter, PDH_FMT_DOUBLE, None, &mut mem_value)
                    == 0
                {
                    // mem_value is in bytes
                    self.gpu_info.memory_used = mem_value.Anonymous.doubleValue as u64;
                }
            }

            // Try GPU temperature counter if available
            let temp_path = crate::utils::to_wide_string("\\GPU Temperature(*)\\Temperature");
            let mut temp_counter = 0isize;
            if PdhAddEnglishCounterW(query, PCWSTR(temp_path.as_ptr()), 0, &mut temp_counter) == 0 {
                let _ = PdhCollectQueryData(query);
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = PdhCollectQueryData(query);
                let mut temp_value = PDH_FMT_COUNTERVALUE::default();
                if PdhGetFormattedCounterValue(temp_counter, PDH_FMT_DOUBLE, None, &mut temp_value)
                    == 0
                {
                    self.gpu_info.temperature = Some(temp_value.Anonymous.doubleValue as f32);
                }
            }

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
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

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

                // Try to query current video memory usage via IDXGIAdapter3 if available
                if let Ok(adapter3) =
                    adapter.cast::<windows::Win32::Graphics::Dxgi::IDXGIAdapter3>()
                {
                    use windows::Win32::Graphics::Dxgi::{
                        DXGI_MEMORY_SEGMENT_GROUP, DXGI_QUERY_VIDEO_MEMORY_INFO,
                    };
                    unsafe {
                        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                        if adapter3
                            .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP(0), &mut info)
                            .is_ok()
                        {
                            // CurrentUsage is the number of bytes currently used
                            self.gpu_info.memory_used = info.CurrentUsage;
                            if self.gpu_info.memory_total == 0 {
                                // If we didn't get dedicated memory earlier, set from budget
                                self.gpu_info.memory_total = info.Budget;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Build the display text
    fn build_display_text(&self, config: &crate::config::Config) -> String {
        let mut parts = Vec::new();

        // Usage remains configurable
        if config.modules.gpu.show_usage {
            parts.push(format!("GPU {:.0}%", self.gpu_info.usage));
        }

        // Always show VRAM percent if available
        if self.gpu_info.memory_total > 0 {
            let mem_percent = (self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64
                * 100.0) as u32;
            parts.push(format!("VRAM {}%", mem_percent));
        }

        // Always show temperature if available
        if let Some(temp) = self.gpu_info.temperature {
            parts.push(format!("{:.0}°C", temp));
        }

        if parts.is_empty() {
            "GPU".to_string()
        } else {
            parts.join("  ")
        }
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

        // Usage remains configurable
        if config.modules.gpu.show_usage {
            parts.push(format!("GPU {:.0}%", self.gpu_info.usage));
        }

        // Always show VRAM percent if available
        if self.gpu_info.memory_total > 0 {
            let mem_percent = (self.gpu_info.memory_used as f64 / self.gpu_info.memory_total as f64
                * 100.0) as u32;
            parts.push(format!("VRAM {}%", mem_percent));
        }

        // Always show temperature if available
        if let Some(temp) = self.gpu_info.temperature {
            parts.push(format!("{:.0}°C", temp));
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join("  ")
        }
    }

    fn update(&mut self, config: &crate::config::Config) {
        if self.last_update.elapsed().as_millis() >= self.update_interval_ms as u128 {
            self.force_update(config);
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

    fn graph_values(&self) -> Option<Vec<f32>> {
        // Return GPU usage history (oldest to newest) so the renderer can draw a historical graph
        Some(self.usage_history.iter().copied().collect())
    }
}
