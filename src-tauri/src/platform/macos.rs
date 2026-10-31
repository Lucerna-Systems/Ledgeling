//! Geometry-only CoreGraphics adapter. It reads window numbers, layers, bounds,
//! alpha and owner PIDs, plus input idle time. It never reads window names,
//! owner names, screen images, keystrokes, or the clipboard.
use super::{Platform, ProcessInfo, ResizeAccess};
use crate::world::{MonitorInfo, Point, Rect, WindowInfo};
use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

type Ref = *const c_void;
#[repr(C)]
#[derive(Default)]
struct CGpoint {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Default)]
struct CGsize {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Default)]
struct CGrect {
    origin: CGpoint,
    size: CGsize,
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> Ref;
    fn CGRectMakeWithDictionaryRepresentation(dict: Ref, rect: *mut CGrect) -> bool;
    fn CGEventCreate(source: Ref) -> Ref;
    fn CGEventGetLocation(event: Ref) -> CGpoint;
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    static kCGWindowNumber: Ref;
    static kCGWindowLayer: Ref;
    static kCGWindowBounds: Ref;
    static kCGWindowAlpha: Ref;
    static kCGWindowOwnerPID: Ref;
    static kCGWindowIsOnscreen: Ref;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFDictionaryGetValue(dict: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(number: Ref, kind: isize, value: *mut c_void) -> bool;
    fn CFBooleanGetValue(boolean: Ref) -> bool;
    fn CFRelease(value: Ref);
}
extern "C" {
    /// libproc; returns the executable name for a pid. Not a window title.
    fn proc_name(pid: i32, buffer: *mut c_void, size: u32) -> i32;
}

struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
unsafe fn number(dict: Ref, key: Ref) -> Option<f64> {
    let value = CFDictionaryGetValue(dict, key);
    if value.is_null() {
        return None;
    }
    let mut n = 0_f64;
    // 13 = kCFNumberFloat64Type.
    if CFNumberGetValue(value, 13, &mut n as *mut f64 as *mut c_void) {
        Some(n)
    } else {
        None
    }
}
unsafe fn flag(dict: Ref, key: Ref) -> bool {
    let value = CFDictionaryGetValue(dict, key);
    !value.is_null() && CFBooleanGetValue(value)
}

#[derive(Default)]
pub struct NativePlatform;

fn monitor_cache() -> &'static Mutex<Vec<MonitorInfo>> {
    static CACHE: OnceLock<Mutex<Vec<MonitorInfo>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

/// CoreGraphics reports points; the engine works in physical pixels. The app
/// layer hands this adapter exactly one monitor — the pet screen — so its
/// scale is the only conversion the engine's space ever needs, on any DPI mix.
fn engine_scale() -> f64 {
    monitor_cache()
        .lock()
        .ok()
        .and_then(|m| {
            m.iter()
                .find(|m| m.is_primary)
                .or_else(|| m.first())
                .map(|m| m.scale)
        })
        .unwrap_or(1.)
}

impl Platform for NativePlatform {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String> {
        let scale = engine_scale();
        // CF objects from Copy/Create are released exactly once; borrowed
        // dictionaries and numbers remain valid for the array's lifetime.
        unsafe {
            // kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements.
            let raw = CGWindowListCopyWindowInfo(1 | 16, 0);
            if raw.is_null() {
                return Err(
                    "macOS could not provide window geometry. Using the desktop edge.".into(),
                );
            }
            let list = Owned(raw);
            let mut windows = Vec::new();
            for i in 0..CFArrayGetCount(list.0) {
                let dict = CFArrayGetValueAtIndex(list.0, i);
                if number(dict, kCGWindowLayer) != Some(0.)
                    || number(dict, kCGWindowAlpha).unwrap_or(0.) <= 0.
                {
                    continue;
                }
                let bounds = CFDictionaryGetValue(dict, kCGWindowBounds);
                let mut r = CGrect::default();
                if bounds.is_null() || !CGRectMakeWithDictionaryRepresentation(bounds, &mut r) {
                    continue;
                }
                if r.size.width < 80. || r.size.height < 40. {
                    continue;
                }
                let Some(id) = number(dict, kCGWindowNumber) else {
                    continue;
                };
                let rect = Rect::new(
                    r.origin.x * scale,
                    r.origin.y * scale,
                    r.size.width * scale,
                    r.size.height * scale,
                );
                windows.push(WindowInfo {
                    id: id as u64,
                    rect,
                    z_index: windows.len(),
                    is_minimized: !flag(dict, kCGWindowIsOnscreen),
                    monitor_id: monitor_for(rect),
                    pid: number(dict, kCGWindowOwnerPID).map(|pid| pid as u32),
                });
            }
            Ok(windows)
        }
    }

    fn process_name(&self, pid: u32) -> Option<String> {
        process_name(pid as i32)
    }

    fn resize_access(&self) -> ResizeAccess {
        if super::macos_ax::trusted() {
            ResizeAccess::Granted
        } else {
            ResizeAccess::NeedsPermission
        }
    }

    fn request_resize_access(&self) {
        super::macos_ax::ask();
    }

    fn focus_window(&self, window: &WindowInfo) -> Result<(), String> {
        extern "C" {
            fn perch_app_activate(pid: i32) -> i32;
        }
        let pid = window.pid.ok_or("That window's app is unknown.")?;
        // A biome stands on its app's front window, so bringing the app
        // forward brings that window with it.
        match unsafe { perch_app_activate(pid as i32) } {
            1 => Ok(()),
            _ => Err("macOS would not bring that app forward.".into()),
        }
    }

    fn set_window_frame(&self, window: &WindowInfo, to: Rect) -> Result<(), String> {
        let pid = window.pid.ok_or("That window's app is unknown.")?;
        let scale = engine_scale();
        let points = |r: Rect| (r.x / scale, r.y / scale, r.width / scale, r.height / scale);
        super::macos_ax::set_frame(pid as i32, points(window.rect), points(to))
    }

    fn idle_seconds(&self) -> Result<f64, String> {
        // 1 = kCGEventSourceStateHIDSystemState, 0xFFFFFFFF = kCGAnyInputEventType.
        let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(1, 0xFFFF_FFFF) };
        if seconds.is_finite() && seconds >= 0. {
            Ok(seconds)
        } else {
            Ok(0.)
        }
    }

    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String> {
        unsafe {
            let raw = CGWindowListCopyWindowInfo(1 | 16, 0);
            if raw.is_null() {
                return Ok(None);
            }
            let list = Owned(raw);
            for i in 0..CFArrayGetCount(list.0) {
                let dict = CFArrayGetValueAtIndex(list.0, i);
                if number(dict, kCGWindowLayer) != Some(0.) {
                    continue;
                }
                let Some(pid) = number(dict, kCGWindowOwnerPID) else {
                    continue;
                };
                return Ok(process_name(pid as i32).map(|name| ProcessInfo {
                    name,
                    pid: pid as u32,
                }));
            }
            Ok(None)
        }
    }

    fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        let cache = monitor_cache()
            .lock()
            .map_err(|_| "Monitor geometry is unavailable")?;
        if cache.is_empty() {
            Err("No monitor geometry has been reported yet".into())
        } else {
            Ok(cache.clone())
        }
    }

    fn set_monitors(&self, monitors: Vec<MonitorInfo>) {
        if let Ok(mut cache) = monitor_cache().lock() {
            *cache = monitors;
        }
    }

    fn fullscreen_app_active(&self) -> Result<bool, String> {
        let monitors = self.monitors().unwrap_or_default();
        if monitors.is_empty() {
            return Ok(false);
        }
        // A layer-0 window covering an entire display, menu bar included, is
        // macOS's observable signature of a fullscreen or exclusive app.
        Ok(self.enumerate_windows()?.iter().any(|w| {
            monitors.iter().any(|m| {
                (w.rect.x - m.bounds.x).abs() < 2.
                    && (w.rect.y - m.bounds.y).abs() < 2.
                    && (w.rect.width - m.bounds.width).abs() < 2.
                    && (w.rect.height - m.bounds.height).abs() < 2.
            })
        }))
    }

    fn cursor(&self) -> Option<Point> {
        let scale = engine_scale();
        unsafe {
            let raw = CGEventCreate(std::ptr::null());
            if raw.is_null() {
                return None;
            }
            let event = Owned(raw);
            let p = CGEventGetLocation(event.0);
            Some(Point::new(p.x * scale, p.y * scale))
        }
    }

    fn mouse_down(&self) -> Option<bool> {
        Some(unsafe { CGEventSourceButtonState(1, 0) })
    }
}

fn monitor_for(rect: Rect) -> u64 {
    monitor_cache()
        .lock()
        .ok()
        .and_then(|m| {
            m.iter()
                .find(|m| m.bounds.contains(rect.center()))
                .or_else(|| m.first())
                .map(|m| m.id)
        })
        .unwrap_or(0)
}

fn process_name(pid: i32) -> Option<String> {
    let mut buffer = [0_u8; 256];
    let written =
        unsafe { proc_name(pid, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32) };
    if written <= 0 {
        return None;
    }
    let name = String::from_utf8_lossy(&buffer[..written as usize]).to_lowercase();
    Some(name.trim_end_matches(".app").to_string())
}
