//! Main application logic for TopBar

use anyhow::Result;
use log::{debug, info, warn};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::config::Config;
use crate::tray::TrayIcon;
use crate::window::WindowManager;
use crate::modules::ModuleRegistry;

/// Main application state
pub struct Application {
    config: Arc<Config>,
    window_manager: WindowManager,
    tray_icon: Option<TrayIcon>,
    is_running: bool,
}

impl Application {
    /// Create a new application instance
    pub fn new(config: Arc<Config>) -> Result<Self> {
        info!("Initializing TopBar application");

        // Create the main window
        let window_manager = WindowManager::new(config.clone())?;

        // Create tray icon (optional, might fail)
        let tray_icon = match TrayIcon::new(window_manager.hwnd()) {
            Ok(tray) => {
                info!("Tray icon created successfully");
                Some(tray)
            }
            Err(e) => {
                warn!("Failed to create tray icon: {}", e);
                None
            }
        };

        Ok(Self {
            config,
            window_manager,
            tray_icon,
            is_running: false,
        })
    }

    /// Run the application
    pub fn run(&mut self) -> Result<()> {
        info!("Starting TopBar main loop");
        self.is_running = true;

        // Show the window
        self.window_manager.show();

        // Enter the message loop
        self.window_manager.run_message_loop()?;

        self.is_running = false;
        info!("TopBar main loop ended");
        
        Ok(())
    }

    /// Stop the application
    pub fn stop(&mut self) {
        info!("Stopping TopBar application");
        self.is_running = false;
        
        // Hide window
        self.window_manager.hide();
    }

    /// Check if application is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get window manager
    pub fn window_manager(&self) -> &WindowManager {
        &self.window_manager
    }

    /// Toggle window visibility
    pub fn toggle_visibility(&self) {
        self.window_manager.toggle_visibility();
    }

    /// Update theme
    pub fn update_theme(&self) {
        self.window_manager.update_theme();
    }

    /// Reload configuration
    pub fn reload_config(&mut self) -> Result<()> {
        info!("Reloading configuration");
        
        match Config::load_or_default() {
            Ok(config) => {
                self.config = Arc::new(config);
                self.window_manager.request_redraw();
                info!("Configuration reloaded successfully");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to reload configuration: {}", e);
                Err(e)
            }
        }
    }

    /// Save current configuration
    pub fn save_config(&self) -> Result<()> {
        self.config.save()?;
        info!("Configuration saved");
        Ok(())
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        info!("Cleaning up TopBar application");
        // Cleanup happens automatically through Drop implementations
    }
}

/// Application builder for customization
pub struct ApplicationBuilder {
    config: Option<Arc<Config>>,
    show_tray: bool,
    start_hidden: bool,
}

impl ApplicationBuilder {
    /// Create a new application builder
    pub fn new() -> Self {
        Self {
            config: None,
            show_tray: true,
            start_hidden: false,
        }
    }

    /// Set custom configuration
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(Arc::new(config));
        self
    }

    /// Disable tray icon
    pub fn without_tray(mut self) -> Self {
        self.show_tray = false;
        self
    }

    /// Start hidden
    pub fn start_hidden(mut self) -> Self {
        self.start_hidden = true;
        self
    }

    /// Build the application
    pub fn build(self) -> Result<Application> {
        let config = self.config.unwrap_or_else(|| {
            Arc::new(Config::load_or_default().unwrap_or_default())
        });

        Application::new(config)
    }
}

impl Default for ApplicationBuilder {
    fn default() -> Self {
        Self::new()
    }
}
