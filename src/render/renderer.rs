use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

use crate::modules::ModuleRegistry;
use crate::theme::Theme;
use crate::utils::Rect;

/// Main renderer for the topbar
pub struct Renderer {
    pub(crate) hwnd: HWND,
    pub(crate) dpi: u32,
    pub(crate) module_registry: ModuleRegistry,
    pub(crate) module_bounds: HashMap<String, Rect>,
    pub(crate) icons: crate::render::icons::Icons,
    // Cache of small HICONs for executables (keyed by path)
    pub(crate) icon_cache: std::collections::HashMap<String, windows::Win32::UI::WindowsAndMessaging::HICON>,
    // Double buffering
    back_buffer: HDC,
    back_bitmap: HBITMAP,
    buffer_size: (i32, i32),
}

impl Renderer {
    /// Create a new renderer
    pub fn new(hwnd: HWND, dpi: u32) -> Result<Self, anyhow::Error> {
        let module_registry = ModuleRegistry::new();
        let icons = crate::render::icons::Icons::new();

        Ok(Self {
            hwnd,
            dpi,
            module_registry,
            module_bounds: HashMap::new(),
            icons,
            icon_cache: std::collections::HashMap::new(),
            back_buffer: HDC::default(),
            back_bitmap: HBITMAP::default(),
            buffer_size: (0, 0),
        })
    }

    /// Update DPI
    pub fn set_dpi(&mut self, dpi: u32) {
        self.dpi = dpi;
    }

    /// Ensure back buffer is correct size
    fn ensure_back_buffer(&mut self, hdc: HDC, width: i32, height: i32) {
        if self.buffer_size.0 != width || self.buffer_size.1 != height {
            unsafe {
                // Clean up old buffer
                if !self.back_buffer.is_invalid() {
                    let _ = DeleteDC(self.back_buffer);
                }
                if !self.back_bitmap.is_invalid() {
                    let _ = DeleteObject(self.back_bitmap);
                }

                // Create new buffer
                self.back_buffer = CreateCompatibleDC(hdc);
                self.back_bitmap = CreateCompatibleBitmap(hdc, width, height);
                SelectObject(self.back_buffer, self.back_bitmap);
                self.buffer_size = (width, height);
            }
        }
    }

    /// Main paint function
    pub fn paint(&mut self, hdc: HDC, bar_rect: &Rect, theme: &Theme) {
        self.ensure_back_buffer(hdc, bar_rect.width, bar_rect.height);

        // Clear module bounds
        self.module_bounds.clear();

        // Draw to back buffer
        super::drawing::draw_background(self.back_buffer, bar_rect, theme);
        super::modules::draw_modules(self, self.back_buffer, bar_rect, theme);

        // Copy to screen
        unsafe {
            let _ = BitBlt(
                hdc,
                0,
                0,
                bar_rect.width,
                bar_rect.height,
                self.back_buffer,
                0,
                0,
                SRCCOPY,
            );
        }
    }

    /// Hit test to find which module was clicked
    pub fn hit_test(&self, x: i32, y: i32) -> Option<String> {
        for (id, rect) in &self.module_bounds {
            if rect.contains(x, y) {
                return Some(id.clone());
            }
        }
        None
    }

    /// Get module bounds
    pub fn module_bounds(&self) -> &HashMap<String, Rect> {
        &self.module_bounds
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            if !self.back_buffer.is_invalid() {
                let _ = DeleteDC(self.back_buffer);
            }
            if !self.back_bitmap.is_invalid() {
                let _ = DeleteObject(self.back_bitmap);
            }

            // Destroy any cached icon handles
            for (_path, icon) in self.icon_cache.drain() {
                if !icon.is_invalid() {
                    let _ = DestroyIcon(icon);
                }
            }
        }
    }
}