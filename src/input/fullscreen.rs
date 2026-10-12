//! "Does a fullscreen app own the monitor under (x, y)?" probe (upstream 6030e94 … 684de87).
//! Called only when a gesture would start. Coordinates are global screen coordinates as
//! reported by the hooks (Windows: physical px; macOS: points, top-left origin).

#[allow(unused_imports)]
use super::decision::{covers_monitor, is_shell_window_class, Rect};

#[cfg(target_os = "windows")]
pub fn click_is_on_fullscreen_app(x: f64, y: f64) -> bool {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, GWL_STYLE, WS_CAPTION,
    };
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let mut cls = [0u16; 64];
        let n = GetClassNameW(fg, &mut cls);
        if n > 0 && is_shell_window_class(&String::from_utf16_lossy(&cls[..n as usize])) {
            return false;
        }
        let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let pt = POINT { x: x as i32, y: y as i32 };
        if mon.is_invalid() || mon != MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) {
            return false; // click on another monitor still gestures
        }
        let mut wr = RECT::default();
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetWindowRect(fg, &mut wr).is_err() || !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return false;
        }
        let rc = |r: RECT| Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
        let style = GetWindowLongW(fg, GWL_STYLE) as u32;
        let has_caption = style & WS_CAPTION.0 == WS_CAPTION.0;
        covers_monitor(rc(wr), rc(mi.rcMonitor), rc(mi.rcWork), has_caption)
    }
}

/// macOS best effort: the front-most normal (layer 0) on-screen window of another app whose
/// bounds contain the whole display under the cursor (CGWindowList is front-to-back).
/// Native fullscreen windows and borderless games equal the display bounds (menu bar hidden);
/// a zoomed window stops below the menu bar, so it doesn't count. The work area is modelled
/// as "display minus menu bar", so with "Automatically hide the menu bar" a zoomed window that
/// reaches the top edge would also count (same trade-off upstream accepts on Windows, bd64923).
#[cfg(target_os = "macos")]
pub fn click_is_on_fullscreen_app(x: f64, y: f64) -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::CGDisplay;
    use core_graphics::geometry::{CGPoint, CGRect};
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
        kCGWindowListOptionOnScreenOnly,
    };

    let Ok((ids, n)) = CGDisplay::displays_with_point(CGPoint::new(x, y), 1) else { return false };
    if n == 0 {
        return false;
    }
    let db = CGDisplay::new(ids[0]).bounds();
    let to_rect = |r: CGRect| Rect {
        left: r.origin.x.round() as i32,
        top: r.origin.y.round() as i32,
        right: (r.origin.x + r.size.width).round() as i32,
        bottom: (r.origin.y + r.size.height).round() as i32,
    };
    let monitor = to_rect(db);
    let mut work = monitor;
    work.top += 1; // "menu bar reserves space": see doc comment

    let Some(list) = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
    ) else {
        return false;
    };
    let me = std::process::id() as i64;
    let key = |s: &'static str| CFString::from_static_string(s);
    for item in list.iter() {
        let dict: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
        let num = |k: &'static str| {
            dict.find(key(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_i64())
        };
        if num("kCGWindowLayer") != Some(0) {
            continue;
        }
        if num("kCGWindowOwnerPID") == Some(me) {
            continue;
        }
        let owner = dict
            .find(key("kCGWindowOwnerName"))
            .and_then(|v| v.downcast::<CFString>())
            .map(|s| s.to_string())
            .unwrap_or_default();
        if owner == "Dock" || owner == "Window Server" {
            continue;
        }
        let Some(bounds) = dict
            .find(key("kCGWindowBounds"))
            .and_then(|v| v.downcast::<CFDictionary>())
            .and_then(|d| CGRect::from_dict_representation(&d))
        else {
            continue;
        };
        // Front-most normal window decides; titles can't be probed without Screen Recording,
        // so has_caption is treated as false.
        return covers_monitor(to_rect(bounds), monitor, work, false);
    }
    false
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn click_is_on_fullscreen_app(_x: f64, _y: f64) -> bool {
    false
}

/// Whether this platform has a real probe (the setting is shown only then, as upstream).
pub const PROBE_AVAILABLE: bool = cfg!(any(target_os = "windows", target_os = "macos"));
