//! Module system for TopBar
//! 
//! Modules are individual components that display information or provide
//! functionality in the topbar.

pub mod clock;
pub mod system_info;
pub mod battery;
pub mod network;
pub mod volume;
pub mod app_menu;
pub mod active_window;
pub mod media;
pub mod weather;
pub mod gpu;
pub mod keyboard_layout;
pub mod uptime;
pub mod bluetooth;
pub mod disk;

use std::any::Any;
use std::collections::HashMap;
use windows::Win32::Graphics::Gdi::HDC;

use crate::theme::Theme;

/// Trait for all topbar modules
pub trait Module: Send + Sync {
    /// Unique identifier for the module
    fn id(&self) -> &str;

    /// Display name for the module
    fn name(&self) -> &str;

    /// Get the current display text
    fn display_text(&self, config: &crate::config::Config) -> String;

    /// Update module state (called periodically)
    fn update(&mut self);

    /// Handle click event
    fn on_click(&mut self) {}

    /// Handle right-click event  
    fn on_right_click(&mut self) {}

    /// Handle scroll event
    fn on_scroll(&mut self, _delta: i32) {}

    /// Get tooltip text
    fn tooltip(&self) -> Option<String> {
        None
    }

    /// Whether the module should be visible
    fn is_visible(&self) -> bool {
        true
    }

    /// Get preferred width (0 for auto)
    fn preferred_width(&self) -> i32 {
        0
    }

    /// Cast to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Cast to Any mutably for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Render context for modules
#[derive(Clone)]
pub struct ModuleRenderContext {
    pub hdc: HDC,
    pub theme: Theme,
    pub dpi: u32,
}

/// Registry for managing modules
pub struct ModuleRegistry {
    modules: HashMap<String, Box<dyn Module>>,
    order_left: Vec<String>,
    order_center: Vec<String>,
    order_right: Vec<String>,
}

impl ModuleRegistry {
    /// Create a new module registry with default modules
    pub fn new() -> Self {
        let mut registry = Self {
            modules: HashMap::new(),
            order_left: vec!["app_menu".to_string(), "active_window".to_string()],
            order_center: vec![],
            order_right: vec![
                "media".to_string(),
                "keyboard_layout".to_string(),
                "gpu".to_string(),
                "system_info".to_string(),
                "disk".to_string(),
                "network".to_string(),
                "bluetooth".to_string(),
                "volume".to_string(),
                "battery".to_string(),
                "uptime".to_string(),
                "clock".to_string(),
            ],
        };

        // Register default modules
        registry.register(Box::new(clock::ClockModule::new()));
        registry.register(Box::new(system_info::SystemInfoModule::new()));
        registry.register(Box::new(battery::BatteryModule::new()));
        registry.register(Box::new(network::NetworkModule::new()));
        registry.register(Box::new(volume::VolumeModule::new()));
        registry.register(Box::new(app_menu::AppMenuModule::new()));
        registry.register(Box::new(active_window::ActiveWindowModule::new()));
        registry.register(Box::new(media::MediaModule::new()));
        
        // Register new modules
        registry.register(Box::new(gpu::GpuModule::new()));
        registry.register(Box::new(keyboard_layout::KeyboardLayoutModule::new()));
        registry.register(Box::new(uptime::UptimeModule::new()));
        registry.register(Box::new(bluetooth::BluetoothModule::new()));
        registry.register(Box::new(disk::DiskModule::new()));

        registry
    }

    /// Register a module
    pub fn register(&mut self, module: Box<dyn Module>) {
        let id = module.id().to_string();
        self.modules.insert(id, module);
    }

    /// Get a module by ID
    pub fn get(&self, id: &str) -> Option<&Box<dyn Module>> {
        self.modules.get(id)
    }

    /// Get a mutable module by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Box<dyn Module>> {
        self.modules.get_mut(id)
    }

    /// Update all modules
    pub fn update_all(&mut self) {
        for module in self.modules.values_mut() {
            module.update();
        }
    }

    /// Get left-side modules in order
    pub fn left_modules(&self) -> Vec<&Box<dyn Module>> {
        self.order_left
            .iter()
            .filter_map(|id| self.modules.get(id))
            .collect()
    }

    /// Get center modules in order
    pub fn center_modules(&self) -> Vec<&Box<dyn Module>> {
        self.order_center
            .iter()
            .filter_map(|id| self.modules.get(id))
            .collect()
    }

    /// Get right-side modules in order
    pub fn right_modules(&self) -> Vec<&Box<dyn Module>> {
        self.order_right
            .iter()
            .filter_map(|id| self.modules.get(id))
            .collect()
    }

    /// Set module order for left side
    pub fn set_left_order(&mut self, order: Vec<String>) {
        self.order_left = order;
    }

    /// Set module order for center
    pub fn set_center_order(&mut self, order: Vec<String>) {
        self.order_center = order;
    }

    /// Set module order for right side
    pub fn set_right_order(&mut self, order: Vec<String>) {
        self.order_right = order;
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
