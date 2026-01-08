use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Foundation::COLORREF;

use crate::theme::Theme;
use crate::utils::Rect;

/// Draw the background
pub fn draw_background(hdc: HDC, rect: &Rect, theme: &Theme) {
    unsafe {
        let brush = CreateSolidBrush(theme.background.colorref());
        let win_rect = windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: rect.width,
            bottom: rect.height,
        };
        FillRect(hdc, &win_rect, brush);
        let _ = DeleteObject(brush);

        // Draw subtle bottom border
        let border_brush = CreateSolidBrush(theme.border.colorref());
        let border_rect = windows::Win32::Foundation::RECT {
            left: 0,
            top: rect.height - 1,
            right: rect.width,
            bottom: rect.height,
        };
        FillRect(hdc, &border_rect, border_brush);
        let _ = DeleteObject(border_brush);
    }
}

/// Create a font with optimized rendering for modern UI (macOS-inspired)
pub fn create_font(family: &str, size: i32, bold: bool) -> HFONT {
    unsafe {
        let family_wide: Vec<u16> = family.encode_utf16().chain(std::iter::once(0)).collect();
        let mut lf = LOGFONTW {
            lfHeight: -size,
            lfWeight: if bold { 600 } else { 400 },
            lfCharSet: DEFAULT_CHARSET,
            lfOutPrecision: OUT_TT_PRECIS,
            lfClipPrecision: CLIP_DEFAULT_PRECIS,
            lfQuality: CLEARTYPE_QUALITY,
            lfPitchAndFamily: VARIABLE_PITCH.0 | FF_SWISS.0,
            ..Default::default()
        };

        let face_len = family_wide.len().min(32);
        lf.lfFaceName[..face_len].copy_from_slice(&family_wide[..face_len]);

        CreateFontIndirectW(&lf)
    }
}

/// Measure text dimensions
pub fn measure_text(hdc: HDC, text: &str) -> (i32, i32) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let mut size = windows::Win32::Foundation::SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &wide[..wide.len() - 1], &mut size);
        (size.cx, size.cy)
    }
}

/// Draw text at position
pub fn draw_text(hdc: HDC, x: i32, y: i32, text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = TextOutW(hdc, x, y, &wide[..wide.len() - 1]);
    }
}

/// Scale a value by DPI
pub fn scale(value: i32, dpi: u32) -> i32 {
    (value as f32 * dpi as f32 / 96.0) as i32
}

/// Downsample a series of values to fit within max_points by averaging chunks
pub fn downsample_values(values: Vec<f32>, max_points: usize) -> Vec<f32> {
    if values.len() <= max_points || max_points == 0 {
        return values;
    }
    
    let mut out = Vec::with_capacity(max_points);
    let chunk = values.len() / max_points;
    let mut idx = 0usize;
    
    for _ in 0..max_points {
        let end = (idx + chunk).min(values.len());
        let slice = &values[idx..end];
        let avg = if slice.is_empty() { 0.0 } else { slice.iter().sum::<f32>() / slice.len() as f32 };
        out.push(avg);
        idx = end;
    }
    
    // Fold remaining samples into the last value
    if idx < values.len() && !out.is_empty() {
        let rem_avg = values[idx..].iter().sum::<f32>() / (values.len() - idx) as f32;
        if let Some(last) = out.last_mut() {
            *last = (*last + rem_avg) / 2.0;
        }
    }
    
    out
}

/// Draw a line graph from values (0-100) within a rectangle
pub fn draw_line_graph(hdc: HDC, values: &[f32], rect: &Rect, padding: i32, color: COLORREF) {
    if values.is_empty() {
        return;
    }
    
    let inner_w = rect.width - padding * 2;
    let inner_h = rect.height - 4;
    
    let mut points: Vec<windows::Win32::Foundation::POINT> = Vec::with_capacity(values.len());
    let step = if values.len() > 1 { inner_w as f32 / (values.len() - 1) as f32 } else { 0.0 };
    
    for (i, v) in values.iter().enumerate() {
        let clamped = v.clamp(0.0, 100.0) / 100.0;
        let px = rect.x + padding + (i as f32 * step) as i32;
        let py = rect.y + 2 + ((1.0 - clamped) * inner_h as f32) as i32;
        points.push(windows::Win32::Foundation::POINT { x: px, y: py });
    }
    
    // Ensure at least 2 points for drawing
    if points.len() == 1 {
        points.push(points[0]);
    }
    
    unsafe {
        use windows::Win32::Graphics::Gdi::{CreatePen, PS_SOLID, SelectObject, MoveToEx, LineTo};
        let pen = CreatePen(PS_SOLID, 1, color);
        let old_pen = SelectObject(hdc, pen);
        
        let mut first = true;
        for p in &points {
            if first {
                let _ = MoveToEx(hdc, p.x, p.y, Some(std::ptr::null_mut()));
                first = false;
            } else {
                let _ = LineTo(hdc, p.x, p.y);
            }
        }
        
        let _ = SelectObject(hdc, old_pen);
        let _ = DeleteObject(pen);
    }
}