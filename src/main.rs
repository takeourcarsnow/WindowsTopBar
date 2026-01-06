//! TopBar - A native Windows 11 topbar application inspired by macOS
//! 
//! This application creates a sleek, customizable menu bar at the top of your screen,
//! similar to macOS, with system information, app menus, and more.

#![windows_subsystem = "windows"]

mod app;
mod config;
mod effects;
mod error;
mod hotkey;
mod modules;
mod render;
mod theme;
mod tray;
mod utils;
mod window;

use anyhow::Result;
use log::{info, LevelFilter};
use std::sync::Arc;

use crate::app::Application;
use crate::config::Config;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .format_timestamp_millis()
        .init();

    info!("Starting TopBar v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Arc::new(Config::load_or_default()?);
    info!("Configuration loaded successfully");

    // Create and run the application
    let mut app = Application::new(config)?;
    app.run()?;

    info!("TopBar shutting down gracefully");
    Ok(())
}
