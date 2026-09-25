//! Win32 adapter. It reads window geometry, z-order, idle time and the
//! foreground process's executable name. It never asks for a window title,
//! never captures the screen, and never installs an input hook.
//!
//! Only a first hand-run on one Windows 11 machine so far; see
//! docs/verification.md for what has and has not been run on real hardware.
use super::{Platform, ProcessInfo};
use crate::world::{MonitorInfo, Point, Rect, WindowInfo};
use std::sync::{Mutex, OnceLock};

type Handle = isize;
type Bool = i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RectW {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PointW {
    x: i32,
    y: i32,
}
#[repr(C)]
struct LastInputInfo {
    size: u32,
    time: u32,
}

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(callback: extern "system" fn(Handle, isize) -> Bool, param: isize) -> Bool;
    fn IsWindowVisible(window: Handle) -> Bool;
    fn IsIconic(window: Handle) -> Bool;
    fn GetWindowLongPtrW(window: Handle, index: i32) -> isize;
    fn GetWindowRect(window: Handle, rect: *mut RectW) -> Bool;
    fn GetForegroundWindow() -> Handle;
    fn GetShellWindow() -> Handle;
    fn GetWindowThreadProcessId(window: Handle, pid: *mut u32) -> u32;
    fn GetLastInputInfo(info: *mut LastInputInfo) -> Bool;
    fn GetCursorPos(point: *mut PointW) -> Bool;
    fn GetAncestor(window: Handle, flags: u32) -> Handle;
    fn SetWindowLongPtrW(window: Handle, index: i32, value: isize) -> isize;
    fn SetPropW(window: Handle, name: *const u16, data: isize) -> Bool;
    fn GetPropW(window: Handle, name: *const u16) -> isize;
    fn CallWindowProcW(
        previous: isize,
        window: Handle,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
    fn OpenProcess(access: u32, inherit: Bool, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> Bool;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        buffer: *mut u16,
        size: *mut u32,
    ) -> Bool;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(
        window: Handle,
        attribute: u32,
        value: *mut std::ffi::c_void,
        size: u32,
    ) -> i32;
}

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
const WS_EX_NOACTIVATE: isize = 0x0800_0000;
const WS_EX_TRANSPARENT: isize = 0x0000_0020;
const WS_EX_LAYERED: isize = 0x0008_0000;
/// `GetWindowRect` includes invisible resize borders; pets would perch several
/// pixels off the visual edge without this.
const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
/// Without the cloak check, closed UWP shells appear as phantom windows.
const DWMWA_CLOAKED: u32 = 14;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const GA_ROOT: u32 = 2;
const MINIMUM_WIDTH: f64 = 80.;
const MINIMUM_HEIGHT: f64 = 40.;

/// Toggle only `WS_EX_TRANSPARENT`. The toolkit's own toggle also flips
/// `WS_EX_LAYERED` and forces a frame change on every hover, and Windows
/// briefly paints the overlay's hidden caption during that frame change.
pub(super) fn set_click_through(window: Handle, click_through: bool) {
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) | WS_EX_LAYERED;
        let style = if click_through {
            style | WS_EX_TRANSPARENT
        } else {
            style & !WS_EX_TRANSPARENT
        };
        SetWindowLongPtrW(window, GWL_EXSTYLE, style);
    }
}

pub(super) fn is_click_through(window: Handle) -> bool {
    unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) & WS_EX_TRANSPARENT != 0 }
}

const GWLP_WNDPROC: i32 = -4;
const WM_NCPAINT: u32 = 0x0085;
const WM_NCACTIVATE: u32 = 0x0086;
/// Undocumented messages the visual-styles engine uses to draw the caption
/// and frame directly, bypassing `WM_NCPAINT`.
const WM_NCUAHDRAWCAPTION: u32 = 0x00AE;
const WM_NCUAHDRAWFRAME: u32 = 0x00AF;

/// Where each quieted window keeps the procedure it replaced: a window
/// property, so the overlay and the bubble each chain to their own.
fn previous_proc_name() -> *const u16 {
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    NAME.get_or_init(|| "PerchPreviousProc\0".encode_utf16().collect())
        .as_ptr()
}

extern "system" fn overlay_proc(
    window: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        // TRUE lets the (non-)activation proceed without repainting a caption.
        WM_NCACTIVATE => return 1,
        WM_NCPAINT | WM_NCUAHDRAWCAPTION | WM_NCUAHDRAWFRAME => return 0,
        _ => {}
    }
    unsafe {
        CallWindowProcW(
            GetPropW(window, previous_proc_name()),
            window,
            message,
            wparam,
            lparam,
        )
    }
}

/// The overlay keeps the hidden caption its toolkit gave it (removing it makes
/// Windows paint a permanent one), but Windows' default handling of the
/// non-client paint and activation it sends when the overlay is created draws
/// that caption across the top of the screen, where it stays until something
/// repaints it. The overlay has no frame to draw, so those messages are
/// answered here and never reach the default handler. The pet bubble is the
/// same kind of undecorated window, and gets the same treatment; unlike the
/// overlay it is activated, which is exactly when the caption would appear.
pub(super) fn quiet_overlay_frame(window: Handle) {
    unsafe {
        if GetPropW(window, previous_proc_name()) != 0 {
            return;
        }
        let ours = overlay_proc as *const () as isize;
        let previous = GetWindowLongPtrW(window, GWLP_WNDPROC);
        // The property is set first: the next message may already arrive at
        // the new procedure.
        SetPropW(window, previous_proc_name(), previous);
        SetWindowLongPtrW(window, GWLP_WNDPROC, ours);
    }
}

/// A tool window has no taskbar button and no Alt+Tab entry, and the window
/// scan skips it, so pets never perch on the bubble.
pub(super) fn tool_window(window: Handle) {
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        if style & WS_EX_TOOLWINDOW == 0 {
            SetWindowLongPtrW(window, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW);
        }
    }
}

pub struct NativePlatform;
impl Default for NativePlatform {
    fn default() -> Self {
        NativePlatform
    }
}

fn monitor_cache() -> &'static Mutex<Vec<MonitorInfo>> {
    static CACHE: OnceLock<Mutex<Vec<MonitorInfo>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

struct Collector {
    windows: Vec<WindowInfo>,
}

extern "system" fn collect(window: Handle, param: isize) -> Bool {
    // EnumWindows walks top-level windows in z-order, front to back.
    let collector = unsafe { &mut *(param as *mut Collector) };
    unsafe {
        if IsWindowVisible(window) == 0 || GetAncestor(window, GA_ROOT) != window {
            return 1;
        }
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        if style & WS_EX_TOOLWINDOW != 0 || style & WS_EX_NOACTIVATE != 0 {
            return 1;
        }
        let mut cloaked: u32 = 0;
        let cloak_result = DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        if cloak_result == 0 && cloaked != 0 {
            return 1;
        }
        let mut bounds = RectW::default();
        let framed = DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut bounds as *mut RectW as *mut std::ffi::c_void,
            std::mem::size_of::<RectW>() as u32,
        );
        if framed != 0 && GetWindowRect(window, &mut bounds) == 0 {
            return 1;
        }
        let rect = Rect::new(
            bounds.left as f64,
            bounds.top as f64,
            (bounds.right - bounds.left) as f64,
            (bounds.bottom - bounds.top) as f64,
        );
        if rect.width < MINIMUM_WIDTH || rect.height < MINIMUM_HEIGHT {
            return 1;
        }
        let z_index = collector.windows.len();
        collector.windows.push(WindowInfo {
            id: window as u64,
            rect,
            z_index,
            is_minimized: IsIconic(window) != 0,
            monitor_id: monitor_for(rect),
        });
    }
    1
}

fn monitor_for(rect: Rect) -> u64 {
    monitor_cache()
        .lock()
        .ok()
        .and_then(|monitors| {
            monitors
                .iter()
                .find(|m| m.bounds.contains(rect.center()))
                .or_else(|| monitors.first())
                .map(|m| m.id)
        })
        .unwrap_or(0)
}

impl Platform for NativePlatform {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String> {
        let mut collector = Collector {
            windows: Vec::new(),
        };
        let ok = unsafe { EnumWindows(collect, &mut collector as *mut Collector as isize) };
        if ok == 0 && collector.windows.is_empty() {
            return Err("Windows could not list open windows. Using the desktop edge.".into());
        }
        collector.windows.retain(|w| !w.is_minimized);
        Ok(collector.windows)
    }

    fn idle_seconds(&self) -> Result<f64, String> {
        let mut info = LastInputInfo {
            size: std::mem::size_of::<LastInputInfo>() as u32,
            time: 0,
        };
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return Ok(0.);
        }
        // Both values are 32-bit tick counts that wrap about every 49 days;
        // wrapping subtraction gives the right answer across the wrap.
        let now = unsafe { GetTickCount() };
        Ok(now.wrapping_sub(info.time) as f64 / 1000.)
    }

    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String> {
        unsafe {
            let window = GetForegroundWindow();
            if window == 0 || window == GetShellWindow() {
                return Ok(None);
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == 0 {
                return Ok(None);
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return Ok(None);
            }
            let mut buffer = [0_u16; 512];
            let mut size = buffer.len() as u32;
            // The executable path, not the window title. Only the file stem is
            // kept, and only long enough to map it to a category.
            let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size);
            CloseHandle(process);
            if ok == 0 {
                return Ok(None);
            }
            let path = String::from_utf16_lossy(&buffer[..size as usize]);
            let name = path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or_default()
                .trim_end_matches(".exe")
                .to_lowercase();
            Ok((!name.is_empty()).then_some(ProcessInfo { name, pid }))
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
        unsafe {
            let window = GetForegroundWindow();
            if window == 0 || window == GetShellWindow() {
                return Ok(false);
            }
            let mut bounds = RectW::default();
            if GetWindowRect(window, &mut bounds) == 0 {
                return Ok(false);
            }
            let rect = Rect::new(
                bounds.left as f64,
                bounds.top as f64,
                (bounds.right - bounds.left) as f64,
                (bounds.bottom - bounds.top) as f64,
            );
            // A foreground window covering a whole monitor, taskbar included,
            // is the observable signature of an exclusive or borderless game.
            Ok(monitors.iter().any(|m| {
                (rect.x - m.bounds.x).abs() < 2.
                    && (rect.y - m.bounds.y).abs() < 2.
                    && (rect.width - m.bounds.width).abs() < 2.
                    && (rect.height - m.bounds.height).abs() < 2.
            }))
        }
    }

    fn cursor(&self) -> Option<Point> {
        let mut point = PointW::default();
        (unsafe { GetCursorPos(&mut point) } != 0)
            .then(|| Point::new(point.x as f64, point.y as f64))
    }

    fn mouse_down(&self) -> Option<bool> {
        // Deliberately not implemented with GetAsyncKeyState: Perch installs no
        // input hooks and reads no key state. The overlay's pointer capture
        // reports the release instead.
        None
    }
}
