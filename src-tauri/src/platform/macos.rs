//! Geometry-only CoreGraphics adapter. Never accesses window names, owner names,
//! screen images, input contents, or accessibility permissions.
use super::Platform;
use crate::world::{Point, Rect, WindowInfo};
use std::ffi::c_void;
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
    static kCGWindowNumber: Ref;
    static kCGWindowLayer: Ref;
    static kCGWindowBounds: Ref;
    static kCGWindowAlpha: Ref;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFDictionaryGetValue(dict: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(number: Ref, kind: isize, value: *mut c_void) -> bool;
    fn CFRelease(value: Ref);
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
    if CFNumberGetValue(value, 13, &mut n as *mut f64 as *mut c_void) {
        Some(n)
    } else {
        None
    }
}
pub struct NativePlatform;
impl Platform for NativePlatform {
    fn enumerate_windows(&self, scale: f64) -> Result<Vec<WindowInfo>, String> {
        // CF objects from Copy/Create are released exactly once; borrowed
        // dictionaries and numbers remain valid for the array's lifetime.
        unsafe {
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
                if let Some(id) = number(dict, kCGWindowNumber) {
                    windows.push(WindowInfo {
                        id: id as u64,
                        rect: Rect {
                            x: r.origin.x * scale,
                            y: r.origin.y * scale,
                            width: r.size.width * scale,
                            height: r.size.height * scale,
                        },
                    });
                }
            }
            Ok(windows)
        }
    }
    fn cursor(&self, scale: f64) -> Option<Point> {
        unsafe {
            let raw = CGEventCreate(std::ptr::null());
            if raw.is_null() {
                return None;
            }
            let event = Owned(raw);
            let p = CGEventGetLocation(event.0);
            Some(Point {
                x: p.x * scale,
                y: p.y * scale,
            })
        }
    }
    fn mouse_down(&self) -> bool {
        unsafe { CGEventSourceButtonState(1, 0) }
    }
}
