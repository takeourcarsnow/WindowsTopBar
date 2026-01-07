//! System information module for CPU, memory, and disk usage

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

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
    // Histories for sparkline graphs
    cpu_history: VecDeque<f32>,
    memory_history: VecDeque<f32>,
    history_len: usize,
    last_update: Instant,
    update_interval_ms: u64,
}

impl SystemInfoModule {
    pub fn new() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::everything()),
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
            // history length for graph samples
            cpu_history: VecDeque::with_capacity(60),
            memory_history: VecDeque::with_capacity(60),
            history_len: 60,
            last_update: Instant::now(),
            update_interval_ms: 2000,
        };
        module.force_update();
        module
    }

    /// Force an immediate update
    fn force_update(&mut self) {
        if let Ok(mut sys) = self.system.lock() {
            sys.refresh_cpu_usage();
            sys.refresh_memory();

            // Calculate CPU usage (average across all cores)
            let cpus = sys.cpus();
            if !cpus.is_empty() {
                self.cpu_usage =
                    cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32;
            }

            // Calculate memory usage
            self.memory_total = sys.total_memory();
            self.memory_used = sys.used_memory();
            if self.memory_total > 0 {
                self.memory_usage =
                    (self.memory_used as f64 / self.memory_total as f64 * 100.0) as f32;
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

        // Update histories for graphs
        self.cpu_history.push_back(self.cpu_usage);
        if self.cpu_history.len() > self.history_len {
            self.cpu_history.pop_front();
        }

        self.memory_history.push_back(self.memory_usage);
        if self.memory_history.len() > self.history_len {
            self.memory_history.pop_front();
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

    /// Get CPU history for graph (oldest to newest)
    pub fn cpu_history(&self) -> Vec<f32> {
        self.cpu_history.iter().copied().collect()
    }

    /// Get memory history for graph (oldest to newest)
    pub fn memory_history(&self) -> Vec<f32> {
        self.memory_history.iter().copied().collect()
    }

    /// Get battery-aware update multiplier (2x on battery, 1x on AC)
    fn get_battery_multiplier(&self) -> u64 {
        // Simple battery detection using Windows API
        unsafe {
            use windows::Win32::System::Power::GetSystemPowerStatus;
            let mut status = windows::Win32::System::Power::SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut status).is_ok() {
                // ACLineStatus: 0 = offline, 1 = online, 255 = unknown
                if status.ACLineStatus == 0 {
                    return 2; // On battery, update less frequently
                }
            }
        }
        1 // On AC power or unknown, use normal interval
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

    fn display_text(&self, _config: &crate::config::Config) -> String {
        // Return cached text to avoid rebuilding strings unnecessarily
        self.cached_text.clone()
    }

    fn update(&mut self, config: &crate::config::Config) {
        // Use configurable update interval from config, with battery optimization
        let base_interval = config.modules.system_info.update_interval_ms;
        let battery_multiplier = self.get_battery_multiplier();
        let effective_interval = base_interval * battery_multiplier;
        
        if self.last_update.elapsed().as_millis() >= effective_interval as u128 {
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn graph_values(&self) -> Option<Vec<f32>> {
        // Return CPU usage history (oldest to newest) so the renderer can draw a historical graph
        Some(self.cpu_history.iter().copied().collect())
    }
}
