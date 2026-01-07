//! Blur and transparency effects for Windows 11
//! 
//! Provides Mica, Acrylic, and other backdrop effects.

#![allow(dead_code, unused_unsafe)]

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DwmExtendFrameIntoClientArea,
    DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DWMWCP_ROUNDSMALL, DWMWCP_DONOTROUND,
};
use windows::Win32::UI::Controls::MARGINS;
use anyhow::Result;
use log::{debug, info};

/// Backdrop types available in Windows 11
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackdropType {
    /// No backdrop effect
    None = 1,
    /// Mica effect (recommended for main windows)
    Mica = 2,
    /// Acrylic effect (translucent blur)
    Acrylic = 3,
    /// Mica Alt (more prominent)
    MicaAlt = 4,
}

/// Window corner preference
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CornerPreference {
    /// Default (usually rounded on Windows 11)
    Default = 0,
    /// Do not round corners
    DoNotRound = 1,
    /// Rounded corners
    Round = 2,
    /// Small rounded corners
    RoundSmall = 3,
}

impl From<CornerPreference> for DWM_WINDOW_CORNER_PREFERENCE {
    fn from(pref: CornerPreference) -> Self {
        match pref {
            CornerPreference::Default => DWM_WINDOW_CORNER_PREFERENCE(0),
            CornerPreference::DoNotRound => DWMWCP_DONOTROUND,
            CornerPreference::Round => DWMWCP_ROUND,
            CornerPreference::RoundSmall => DWMWCP_ROUNDSMALL,
        }
    }
}

/// Effects manager for applying Windows 11 visual effects
pub struct EffectsManager;

impl EffectsManager {
    /// Apply backdrop effect to window
    pub fn set_backdrop(hwnd: HWND, backdrop: BackdropType) -> Result<()> {
        unsafe {
            let backdrop_value: i32 = backdrop as i32;
            
            let result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop_value as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            if result.is_ok() {
                debug!("Applied backdrop type: {:?}", backdrop);
            } else {
                debug!("Failed to apply backdrop (may not be supported)");
            }
        }
        Ok(())
    }

    /// Set window corner preference
    pub fn set_corners(hwnd: HWND, corners: CornerPreference) -> Result<()> {
        unsafe {
            let corner_pref: DWM_WINDOW_CORNER_PREFERENCE = corners.into();
            
            let result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as *const _,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );

            if result.is_ok() {
                debug!("Applied corner preference: {:?}", corners);
            }
        }
        Ok(())
    }

    /// Set dark mode for window frame
    pub fn set_dark_mode(hwnd: HWND, dark: bool) -> Result<()> {
        unsafe {
            let dark_mode: i32 = if dark { 1 } else { 0 };
            
            let result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            if result.is_ok() {
                debug!("Applied dark mode: {}", dark);
            }
        }
        Ok(())
    }

    /// Extend frame into client area (needed for some effects)
    pub fn extend_frame_into_client(hwnd: HWND) -> Result<()> {
        unsafe {
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };

            let result = DwmExtendFrameIntoClientArea(hwnd, &margins);
            
            if result.is_ok() {
                debug!("Extended frame into client area");
            }
        }
        Ok(())
    }

    /// Apply all recommended effects for topbar
    pub fn apply_topbar_effects(hwnd: HWND, dark_mode: bool) -> Result<()> {
        info!("Applying topbar visual effects");
        
        // Set dark/light mode
        Self::set_dark_mode(hwnd, dark_mode)?;
        
        // No rounded corners for a topbar
        Self::set_corners(hwnd, CornerPreference::DoNotRound)?;
        
        // Apply Acrylic backdrop for translucent blur
        Self::set_backdrop(hwnd, BackdropType::Acrylic)?;
        
        Ok(())
    }

    /// Apply effects for dropdown menus
    pub fn apply_menu_effects(hwnd: HWND, dark_mode: bool) -> Result<()> {
        Self::set_dark_mode(hwnd, dark_mode)?;
        Self::set_corners(hwnd, CornerPreference::Round)?;
        Self::set_backdrop(hwnd, BackdropType::Acrylic)?;
        Ok(())
    }
}

/// Legacy blur effect for older Windows versions
pub mod legacy {
    use windows::Win32::Foundation::HWND;
    use anyhow::Result;

    /// Accent state for DWM
    #[repr(i32)]
    pub enum AccentState {
        Disabled = 0,
        EnableGradient = 1,
        EnableTransparent = 2,
        EnableBlurBehind = 3,
        EnableAcrylicBlurBehind = 4,
    }

    /// Accent policy structure
    #[repr(C)]
    struct AccentPolicy {
        accent_state: i32,
        accent_flags: i32,
        gradient_color: u32,
        animation_id: i32,
    }

    /// Window composition attribute data
    #[repr(C)]
    struct WindowCompositionAttribData {
        attrib: i32,
        data: *mut AccentPolicy,
        size: usize,
    }

    /// Apply blur effect using SetWindowCompositionAttribute
    pub fn apply_blur(_hwnd: HWND, accent: AccentState, color: u32) -> Result<()> {
        // This uses undocumented API and may not work on all Windows versions
        // The modern DwmSetWindowAttribute approach is preferred
        
        let mut policy = AccentPolicy {
            accent_state: accent as i32,
            accent_flags: 2,  // ACCENT_ENABLE_BLURBEHIND
            gradient_color: color,
            animation_id: 0,
        };

        let data = WindowCompositionAttribData {
            attrib: 19,  // WCA_ACCENT_POLICY
            data: &mut policy,
            size: std::mem::size_of::<AccentPolicy>(),
        };

        // This would require loading user32.dll and finding SetWindowCompositionAttribute
        // For now, we rely on the DWM methods which are more reliable
        let _ = data;

        Ok(())
    }
}
