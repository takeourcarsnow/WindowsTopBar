use windows::Win32::Graphics::Gdi::HDC;

use crate::modules::ModuleRenderContext;
use crate::theme::Theme;

/// Render context passed to modules
impl ModuleRenderContext {
    pub fn new(hdc: HDC, theme: &Theme, dpi: u32) -> Self {
        Self {
            hdc,
            theme: theme.clone(),
            dpi,
        }
    }
}