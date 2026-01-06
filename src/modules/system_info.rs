//! System information module for CPU, memory, and disk usage

use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::{System, CpuRefreshKind, MemoryRefreshKind, RefreshKind};

use super::Module;
use crate::utils::format_bytes;

/// System information module
pub struct SystemInfoModule {
    system: Arc<Mutex<System>>,
    show_cpu: bool,
    show_memory: bool,
    show_disk: bool,
    cached_text: String,
    cpu_usage: f32,
    memory_usage: f32,
    memory_used: u64,
    memory_total: u64,
    last_update: Instant,
    update_interval_ms: u64,
}

impl SystemInfoModule {
    pub fn new() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::everything())
        );

        let mut module = Self {
            system: Arc::new(Mutex::new(system)),
            show_cpu: true,
            show_memory: true,
            show_disk: false,
            cached_text: String::new(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            memory_used: 0,
            memory_total: 0,
            last_update: Instant::now(),
            update_interval_ms: 2000,
        };
        module.force_update();
        module
    }

    /// Set whether to show CPU usage
    pub fn set_show_cpu(&mut self, show: bool) {
        self.show_cpu = show;
    }

    /// Set whether to show memory usage
    pub fn set_show_memory(&mut self, show: bool) {
        self.show_memory = show;
    }

    /// Set update interval
    pub fn set_update_interval(&mut self, interval_ms: u64) {
        self.update_interval_ms = interval_ms;
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        if let Ok(mut sys) = self.system.lock() {
            sys.refresh_cpu_usage();
            sys.refresh_memory();

            // Calculate CPU usage (average across all cores)
            let cpus = sys.cpus();
            if !cpus.is_empty() {
                self.cpu_usage = cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32;
            }

            // Calculate memory usage
            self.memory_total = sys.total_memory();
            self.memory_used = sys.used_memory();
            if self.memory_total > 0 {
                self.memory_usage = (self.memory_used as f64 / self.memory_total as f64 * 100.0) as f32;
            }
        }

        // Build display text
        let mut parts = Vec::new();

        if self.show_cpu {
            parts.push(format!("CPU {:.0}%", self.cpu_usage));
        }

        if self.show_memory {
            parts.push(format!("MEM {:.0}%", self.memory_usage));
        }

        self.cached_text = parts.join("  ");
        self.last_update = Instant::now();
    }

    /// Get CPU usage percentage
    pub fn cpu_usage(&self) -> f32 {
        self.cpu_usage
    }

    /// Get memory usage percentage
    pub fn memory_usage(&self) -> f32 {
        self.memory_usage
    }
}

impl Default for SystemInfoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SystemInfoModule {
    fn id(&self) -> &str {
        "system_info"
    }

    fn name(&self) -> &str {
        "System Info"
    }

    fn display_text(&self) -> String {
        self.cached_text.clone()
    }

    fn update(&mut self) {
        if self.last_update.elapsed().as_millis() >= self.update_interval_ms as u128 {
            self.force_update();
        }
    }

    fn on_click(&mut self) {
        // Open task manager
        let _ = std::process::Command::new("taskmgr.exe").spawn();
    }

    fn tooltip(&self) -> Option<String> {
        Some(format!(
            "CPU Usage: {:.1}%\nMemory: {} / {} ({:.1}%)",
            self.cpu_usage,
            format_bytes(self.memory_used),
            format_bytes(self.memory_total),
            self.memory_usage
        ))
    }
}
