//! Dropdown menu system for TopBar
//! 
//! Provides macOS-style dropdown menus for various modules.

#![allow(dead_code)]

use windows::Win32::Foundation::{HWND, RECT, LPARAM, WPARAM, LRESULT, COLORREF};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetCapture, ReleaseCapture};
use windows::Win32::Graphics::Gdi::*;
use windows::core::PCWSTR;
use anyhow::Result;

use crate::theme::{Theme, Color};
use crate::utils::{to_wide_string, to_pcwstr};

/// Menu item data
#[derive(Debug, Clone)]
pub struct DropdownItem {
    pub id: u32,
    pub label: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    pub is_separator: bool,
    pub is_disabled: bool,
    pub is_checked: bool,
    pub submenu: Option<Vec<DropdownItem>>,
}

impl DropdownItem {
    /// Create a new menu item
    pub fn new(id: u32, label: &str) -> Self {
        Self {
            id,
            label: label.to_string(),
            icon: None,
            shortcut: None,
            is_separator: false,
            is_disabled: false,
            is_checked: false,
            submenu: None,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self {
            id: 0,
            label: String::new(),
            icon: None,
            shortcut: None,
            is_separator: true,
            is_disabled: false,
            is_checked: false,
            submenu: None,
        }
    }

    /// Add icon
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Add shortcut hint
    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    /// Mark as disabled
    pub fn disabled(mut self) -> Self {
        self.is_disabled = true;
        self
    }

    /// Mark as checked
    pub fn checked(mut self, checked: bool) -> Self {
        self.is_checked = checked;
        self
    }

    /// Add submenu
    pub fn with_submenu(mut self, items: Vec<DropdownItem>) -> Self {
        self.submenu = Some(items);
        self
    }
}

/// Dropdown menu window
pub struct DropdownMenu {
    hwnd: HWND,
    items: Vec<DropdownItem>,
    item_height: i32,
    padding: i32,
    hover_index: Option<usize>,
    theme: Theme,
    on_select: Option<Box<dyn Fn(u32) + Send + Sync>>,
}

/// Window class name for dropdown menus
const DROPDOWN_CLASS: &str = "TopBarDropdownClass";

impl DropdownMenu {
    /// Create a new dropdown menu
    pub fn new(parent: HWND, items: Vec<DropdownItem>, theme: &Theme) -> Result<Self> {
        // Register window class if needed
        Self::ensure_class_registered()?;

        let item_height = 36;  // Taller items for better touch targets
        let padding = 10;  // More breathing room
        
        // Calculate menu size
        let width = 280;  // Wider for modern look
        let height = Self::calculate_height(&items, item_height, padding);

        // Create popup window
        let hwnd = unsafe {
            let class = to_wide_string(DROPDOWN_CLASS);
            let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;

            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                to_pcwstr(&class),
                PCWSTR::null(),
                WS_POPUP,
                0, 0, width, height,
                parent,
                None,
                hinstance,
                None,
            )?
        };

        Ok(Self {
            hwnd,
            items,
            item_height,
            padding,
            hover_index: None,
            theme: theme.clone(),
            on_select: None,
        })
    }

    /// Register window class
    fn ensure_class_registered() -> Result<()> {
        unsafe {
            let class_name = to_wide_string(DROPDOWN_CLASS);
            let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
                lpfnWndProc: Some(dropdown_proc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: to_pcwstr(&class_name),
                hbrBackground: HBRUSH::default(),
                ..Default::default()
            };

            // Ignore error if class already exists
            let _ = RegisterClassExW(&wc);
        }
        Ok(())
    }

    /// Calculate menu height
    fn calculate_height(items: &[DropdownItem], item_height: i32, padding: i32) -> i32 {
        let mut height = padding * 2;
        for item in items {
            if item.is_separator {
                height += 13;  // Taller separator with margins
            } else {
                height += item_height;
            }
        }
        height
    }

    /// Show the menu at position
    pub fn show_at(&mut self, x: i32, y: i32) {
        unsafe {
            // Get menu size
            let width = 280;  // Match new width
            let height = Self::calculate_height(&self.items, self.item_height, self.padding);

            // Adjust position to keep on screen with margin
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let margin = 8;  // Screen edge margin

            let mut final_x = x;
            let mut final_y = y;

            if x + width > screen_width - margin {
                final_x = screen_width - width - margin;
            }
            if y + height > screen_height - margin {
                final_y = y - height;
            }

            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                final_x, final_y, width, height,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            ).ok();

            // Capture mouse
            SetCapture(self.hwnd);
        }
    }

    /// Hide the menu
    pub fn hide(&self) {
        unsafe {
            ReleaseCapture().ok();
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Set selection callback
    pub fn on_select<F: Fn(u32) + Send + Sync + 'static>(&mut self, callback: F) {
        self.on_select = Some(Box::new(callback));
    }

    /// Draw the menu
    fn draw(&self, hdc: HDC) {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.hwnd, &mut rect).ok();

            // Draw background
            let bg_brush = CreateSolidBrush(self.theme.background_secondary.to_colorref());
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(bg_brush);

            // Draw border
            let border_brush = CreateSolidBrush(self.theme.border.to_colorref());
            FrameRect(hdc, &rect, border_brush);
            let _ = DeleteObject(border_brush);

            // Draw items
            let mut y = self.padding;
            
            // Create font
            let font = self.create_font(13, false);
            let old_font = SelectObject(hdc, font);
            SetBkMode(hdc, TRANSPARENT);

            for (index, item) in self.items.iter().enumerate() {
                if item.is_separator {
                    // Draw separator line with proper margins
                    let sep_rect = RECT {
                        left: self.padding + 8,
                        top: y + 6,
                        right: rect.right - self.padding - 8,
                        bottom: y + 7,
                    };
                    let sep_brush = CreateSolidBrush(self.theme.border.to_colorref());
                    FillRect(hdc, &sep_rect, sep_brush);
                    let _ = DeleteObject(sep_brush);
                    y += 13;  // Match new separator height
                } else {
                    // Draw item
                    let item_rect = RECT {
                        left: self.padding,
                        top: y,
                        right: rect.right - self.padding,
                        bottom: y + self.item_height,
                    };

                    // Hover background
                    if Some(index) == self.hover_index && !item.is_disabled {
                        let hover_brush = CreateSolidBrush(self.theme.accent.to_colorref());
                        let rounded_rect = item_rect;
                        FillRect(hdc, &rounded_rect, hover_brush);
                        let _ = DeleteObject(hover_brush);

                        SetTextColor(hdc, Color::rgb(255, 255, 255).to_colorref());
                    } else {
                        let color = if item.is_disabled {
                            self.theme.text_disabled
                        } else {
                            self.theme.text_primary
                        };
                        SetTextColor(hdc, color.to_colorref());
                    }

                    // Draw icon if present
                    let text_x = self.padding + 8;
                    if let Some(ref icon) = item.icon {
                        let icon_wide: Vec<u16> = icon.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = TextOutW(hdc, text_x, y + 4, &icon_wide[..icon_wide.len()-1]);
                    }

                    // Draw label
                    let label_x = text_x + 24;
                    let label_wide: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = TextOutW(hdc, label_x, y + 5, &label_wide[..label_wide.len()-1]);

                    // Draw shortcut
                    if let Some(ref shortcut) = item.shortcut {
                        SetTextColor(hdc, self.theme.text_secondary.to_colorref());
                        let shortcut_wide: Vec<u16> = shortcut.encode_utf16().chain(std::iter::once(0)).collect();
                        
                        let mut size = windows::Win32::Foundation::SIZE::default();
                        let _ = GetTextExtentPoint32W(hdc, &shortcut_wide[..shortcut_wide.len()-1], &mut size);
                        
                        let _ = TextOutW(hdc, item_rect.right - size.cx - 8, y + 5, &shortcut_wide[..shortcut_wide.len()-1]);
                    }

                    // Draw submenu arrow
                    if item.submenu.is_some() {
                        SetTextColor(hdc, self.theme.text_secondary.to_colorref());
                        let arrow: Vec<u16> = "▶".encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = TextOutW(hdc, item_rect.right - 16, y + 5, &arrow[..arrow.len()-1]);
                    }

                    // Draw checkmark
                    if item.is_checked {
                        let check: Vec<u16> = "✓".encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = TextOutW(hdc, self.padding, y + 5, &check[..check.len()-1]);
                    }

                    y += self.item_height;
                }
            }

            SelectObject(hdc, old_font);
            let _ = DeleteObject(font);
        }
    }

    /// Create a font
    fn create_font(&self, size: i32, bold: bool) -> HFONT {
        unsafe {
            let family: Vec<u16> = "Segoe UI Variable Text".encode_utf16().chain(std::iter::once(0)).collect();
            let mut lf = LOGFONTW::default();
            lf.lfHeight = -size - 1;  // Slightly larger for readability
            lf.lfWeight = if bold { FW_SEMIBOLD.0 as i32 } else { FW_NORMAL.0 as i32 };
            lf.lfCharSet = DEFAULT_CHARSET;
            lf.lfQuality = CLEARTYPE_QUALITY;
            lf.lfOutPrecision = OUT_TT_PRECIS;
            
            let face_len = family.len().min(32);
            lf.lfFaceName[..face_len].copy_from_slice(&family[..face_len]);
            
            CreateFontIndirectW(&lf)
        }
    }

    /// Hit test to find item at position
    fn hit_test(&self, y: i32) -> Option<usize> {
        let mut current_y = self.padding;
        
        for (index, item) in self.items.iter().enumerate() {
            let item_h = if item.is_separator { 13 } else { self.item_height };
            
            if y >= current_y && y < current_y + item_h {
                if !item.is_separator && !item.is_disabled {
                    return Some(index);
                }
                return None;
            }
            
            current_y += item_h;
        }
        
        None
    }
}

impl Drop for DropdownMenu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Window procedure for dropdown menus
unsafe extern "system" fn dropdown_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            
            // Would draw menu here (need to store reference)
            // For now, just fill with a color
            let brush = CreateSolidBrush(COLORREF(0x2D2D2D));
            FillRect(hdc, &ps.rcPaint, brush);
            let _ = DeleteObject(brush);
            
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            // Update hover state
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            // Handle selection
            ReleaseCapture().ok();
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }

        WM_CAPTURECHANGED => {
            // Lost capture, close menu
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            match vk {
                0x1B => {  // VK_ESCAPE
                    ReleaseCapture().ok();
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                _ => {}
            }
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
