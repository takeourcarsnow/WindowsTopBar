//! Custom error types for the TopBar application

use thiserror::Error;

/// Main error type for TopBar operations
#[derive(Error, Debug)]
pub enum TopBarError {
    #[error("Window creation failed: {0}")]
    WindowCreation(String),

    #[error("Windows API error: {0}")]
    WindowsApi(#[from] windows::core::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Theme error: {0}")]
    Theme(String),

    #[error("Module error: {0}")]
    Module(String),

    #[error("Render error: {0}")]
    Render(String),

    #[error("System info error: {0}")]
    SystemInfo(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Tray icon error: {0}")]
    TrayIcon(String),
}

/// Result type alias for TopBar operations
pub type TopBarResult<T> = Result<T, TopBarError>;
