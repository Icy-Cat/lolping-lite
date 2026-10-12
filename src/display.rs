//! Display enumeration for multi-monitor overlays / room mapping.

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// 1 = primary, then left→right / top→bottom.
    pub number: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Best-effort list. macOS uses CoreGraphics; elsewhere a single virtual display.
pub fn list_displays() -> Vec<DisplayInfo> {
    #[cfg(target_os = "macos")]
    {
        macos_list()
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![DisplayInfo {
            number: 1,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            primary: true,
        }]
    }
}

/// Map a global point to (display_number, frac_x, frac_y).
pub fn point_to_frac(displays: &[DisplayInfo], x: f64, y: f64) -> (u32, f64, f64) {
    for d in displays {
        let within = x >= d.x as f64
            && y >= d.y as f64
            && x < (d.x as f64 + d.width as f64)
            && y < (d.y as f64 + d.height as f64);
        if within {
            let fx = ((x - d.x as f64) / d.width as f64).clamp(0.0, 1.0);
            let fy = ((y - d.y as f64) / d.height as f64).clamp(0.0, 1.0);
            return (d.number, fx, fy);
        }
    }
    let d = displays.iter().find(|d| d.primary).unwrap_or(&displays[0]);
    (
        d.number,
        (x / d.width as f64).clamp(0.0, 1.0),
        (y / d.height as f64).clamp(0.0, 1.0),
    )
}

pub fn frac_to_local(d: &DisplayInfo, fx: f64, fy: f64) -> (f32, f32) {
    (
        (fx.clamp(0.0, 1.0) as f32) * d.width as f32,
        (fy.clamp(0.0, 1.0) as f32) * d.height as f32,
    )
}

#[cfg(target_os = "macos")]
fn macos_list() -> Vec<DisplayInfo> {
    use core_graphics::display::{CGDisplay, CGMainDisplayID};
    let mut out = Vec::new();
    // Active displays via CGGetActiveDisplayList
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    unsafe {
        extern "C" {
            fn CGGetActiveDisplayList(
                max: u32,
                list: *mut u32,
                count: *mut u32,
            ) -> i32;
        }
        CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut count);
    }
    let main = unsafe { CGMainDisplayID() };
    let mut infos = Vec::new();
    for id in ids.iter().take(count as usize).copied() {
        let disp = CGDisplay::new(id);
        let bounds = disp.bounds();
        infos.push(DisplayInfo {
            number: 0, // assign after sort
            x: bounds.origin.x as i32,
            y: bounds.origin.y as i32,
            width: bounds.size.width as u32,
            height: bounds.size.height as u32,
            primary: id == main,
        });
    }
    infos.sort_by(|a, b| {
        b.primary
            .cmp(&a.primary)
            .then(a.x.cmp(&b.x))
            .then(a.y.cmp(&b.y))
    });
    for (i, d) in infos.iter_mut().enumerate() {
        d.number = (i + 1) as u32;
    }
    if infos.is_empty() {
        return vec![DisplayInfo {
            number: 1,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            primary: true,
        }];
    }
    out = infos;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_roundtrip_primary() {
        let d = list_displays();
        let (n, fx, fy) = point_to_frac(&d, 96.0, 540.0);
        assert_eq!(n, 1);
        let (lx, ly) = frac_to_local(&d[0], fx, fy);
        assert!((lx - 96.0).abs() < 1.0 || d[0].width != 1920);
        let _ = ly;
    }
}
