//! Moving another app's window on macOS, for "make room at the top". This
//! needs the Accessibility permission, which Ledgeling asks for only when the
//! user switches that feature on. Only a window's position and size are ever
//! read or written; no other attribute is touched.
use std::ffi::{c_void, CStr};

type Ref = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    width: f64,
    height: f64,
}

const POINT_TYPE: u32 = 1;
const SIZE_TYPE: u32 = 2;
const UTF8: u32 = 0x0800_0100;
const API_DISABLED: i32 = -25211;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: Ref) -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> Ref;
    fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    fn AXUIElementSetAttributeValue(element: Ref, attribute: Ref, value: Ref) -> i32;
    fn AXValueCreate(kind: u32, value: *const c_void) -> Ref;
    fn AXValueGetValue(value: Ref, kind: u32, out: *mut c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: Ref;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: Ref);
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFStringCreateWithCString(allocator: Ref, text: *const i8, encoding: u32) -> Ref;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> Ref;
    static kCFBooleanTrue: Ref;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
}

struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

fn text(value: &CStr) -> Owned {
    Owned(unsafe { CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), UTF8) })
}

pub fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Show the system's own "allow Ledgeling to control your computer" prompt.
pub fn ask() {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = Owned(CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const u8 as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const u8 as *const c_void,
        ));
        AXIsProcessTrustedWithOptions(options.0);
    }
}

unsafe fn read<T: Default>(element: Ref, attribute: &Owned, kind: u32) -> Option<T> {
    let mut value: Ref = std::ptr::null();
    if AXUIElementCopyAttributeValue(element, attribute.0, &mut value) != 0 || value.is_null() {
        return None;
    }
    let value = Owned(value);
    let mut out = T::default();
    AXValueGetValue(value.0, kind, &mut out as *mut T as *mut c_void).then_some(out)
}

unsafe fn write<T>(element: Ref, attribute: &Owned, kind: u32, value: T) -> Result<(), String> {
    let value = Owned(AXValueCreate(kind, &value as *const T as *const c_void));
    match AXUIElementSetAttributeValue(element, attribute.0, value.0) {
        0 => Ok(()),
        API_DISABLED => Err("Ledgeling doesn't have Accessibility permission.".into()),
        code => Err(format!("macOS would not move that window (error {code}).")),
    }
}

/// Move one of `pid`'s windows from `from` to `to`, both `(x, y, width,
/// height)` in points. AX does not expose window numbers, so the window is
/// found by its current frame.
pub fn set_frame(
    pid: i32,
    from: (f64, f64, f64, f64),
    to: (f64, f64, f64, f64),
) -> Result<(), String> {
    unsafe {
        let app = Owned(AXUIElementCreateApplication(pid));
        if app.0.is_null() {
            return Err("That app can't be reached.".into());
        }
        let (windows_key, position_key, size_key) =
            (text(c"AXWindows"), text(c"AXPosition"), text(c"AXSize"));
        let mut list: Ref = std::ptr::null();
        match AXUIElementCopyAttributeValue(app.0, windows_key.0, &mut list) {
            0 if !list.is_null() => {}
            API_DISABLED => return Err("Ledgeling doesn't have Accessibility permission.".into()),
            _ => return Err("That app doesn't list its windows.".into()),
        }
        let list = Owned(list);
        let close = |a: f64, b: f64| (a - b).abs() <= 2.;
        for i in 0..CFArrayGetCount(list.0) {
            let window = CFArrayGetValueAtIndex(list.0, i);
            let (Some(at), Some(size)) = (
                read::<Point>(window, &position_key, POINT_TYPE),
                read::<Size>(window, &size_key, SIZE_TYPE),
            ) else {
                continue;
            };
            if close(at.x, from.0)
                && close(at.y, from.1)
                && close(size.width, from.2)
                && close(size.height, from.3)
            {
                write(
                    window,
                    &position_key,
                    POINT_TYPE,
                    Point { x: to.0, y: to.1 },
                )?;
                return write(
                    window,
                    &size_key,
                    SIZE_TYPE,
                    Size {
                        width: to.2,
                        height: to.3,
                    },
                );
            }
        }
        Err("That window could not be found to move it.".into())
    }
}
