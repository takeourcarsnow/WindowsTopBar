//! Disk I/O module - shows disk read/write activity

use std::time::Instant;
use sysinfo::Disks;

use super::Module;
use crate::utils::format_bytes;

/// Disk usage information
#[derive(Debug, Clone, Default)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub used_space: u64,
}

/// Disk I/O module
pub struct DiskModule {
    cached_text: String,
    disks: Vec<DiskInfo>,
    primary_disk_index: usize,
    last_update: Instant,
    update_interval_ms: u64,
}

impl DiskModule {
    pub fn new() -> Self {
        let module = Self {
            cached_text: String::new(),
            disks: Vec::new(),
            primary_disk_index: 0,
            last_update: Instant::now(),
            update_interval_ms: 5000,
        };
        module
    }

    /// Force an immediate update
    fn force_update(&mut self, config: &crate::config::Config) {
        self.query_disk_info();

        // Respect configured primary disk if present (match by mount point or name)
        if !config.modules.disk.primary_disk.is_empty() {
            if let Some(pos) = self.disks.iter().position(|d| {
                d.mount_point.starts_with(&config.modules.disk.primary_disk)
                    || d.name == config.modules.disk.primary_disk
            }) {
                self.primary_disk_index = pos;
            }
        }

        self.cached_text = self.build_display_text(config);
        self.last_update = Instant::now();
    }

    /// Query disk information using sysinfo
    fn query_disk_info(&mut self) {
        let disks = Disks::new_with_refreshed_list();

        self.disks.clear();
        for disk in disks.list() {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);

            let mount = disk.mount_point().to_string_lossy().to_string();

            self.disks.push(DiskInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: mount,
                total_space: total,
                available_space: available,
                used_space: used,
            });
        }

        // Find primary disk (usually C:)
        self.primary_disk_index = self
            .disks
            .iter()
            .position(|d| d.mount_point.starts_with("C:") || d.mount_point == "/")
            .unwrap_or(0);
    }

    /// Build the display text
    fn build_display_text(&self, _config: &crate::config::Config) -> String {
        if self.disks.is_empty() {
            return "💾 --".to_string();
        }

        let primary = &self.disks[self.primary_disk_index];
        let usage_percent = if primary.total_space > 0 {
            (primary.used_space as f64 / primary.total_space as f64 * 100.0) as u32
        } else {
            0
        };

        // Always show percentage
        format!("💾 {}%", usage_percent)
    }

    /// Get primary disk usage percentage
    pub fn primary_usage_percent(&self) -> u32 {
        if self.disks.is_empty() {
            return 0;
        }

        let primary = &self.disks[self.primary_disk_index];
        if primary.total_space > 0 {
            (primary.used_space as f64 / primary.total_space as f64 * 100.0) as u32
        } else {
            0
        }
    }

    /// Get all disk information
    pub fn get_disks(&self) -> &[DiskInfo] {
        &self.disks
    }
}

impl Default for DiskModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for DiskModule {
    fn id(&self) -> &str {
        "disk"
    }

    fn name(&self) -> &str {
        "Disk Usage"
    }

    fn display_text(&self, _config: &crate::config::Config) -> String {
        if self.disks.is_empty() {
            return String::new();
        }

        // Always show percentage
        let primary = &self.disks[self.primary_disk_index];
        let usage_percent = if primary.total_space > 0 {
            (primary.used_space as f64 / primary.total_space as f64 * 100.0) as u32
        } else {
            0
        };
        format!("💾 {}%", usage_percent)
    }

    fn update(&mut self, config: &crate::config::Config) {
        if self.last_update.elapsed().as_millis() >= self.update_interval_ms as u128 {
            self.force_update(config);
        }
    }

    fn on_click(&mut self) {
        // Open File Explorer to This PC
        let _ = std::process::Command::new("explorer.exe")
            .arg("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}")
            .spawn();
    }

    fn on_right_click(&mut self) {
        // Open Disk Management
        let _ = std::process::Command::new("diskmgmt.msc").spawn();
    }

    fn tooltip(&self) -> Option<String> {
        if self.disks.is_empty() {
            return None;
        }

        let mut lines: Vec<String> = vec!["Disk Usage:".to_string()];

        for disk in &self.disks {
            let usage_percent = if disk.total_space > 0 {
                (disk.used_space as f64 / disk.total_space as f64 * 100.0) as u32
            } else {
                0
            };

            lines.push(format!(
                "{} {} / {} ({:.0}%)",
                if disk.mount_point.is_empty() {
                    &disk.name
                } else {
                    &disk.mount_point
                },
                format_bytes(disk.used_space),
                format_bytes(disk.total_space),
                usage_percent
            ));
        }

        Some(lines.join("\n"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
