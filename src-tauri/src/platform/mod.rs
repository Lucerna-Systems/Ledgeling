//! Every OS query sits behind this trait so pet behavior can be tested without
//! a desktop and ported without touching the engine.
use crate::world::{MonitorInfo, Point, Rect, WindowInfo};

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessInfo {
    /// Executable name, lowercased, without extension. The window title is
    /// never read; see docs/privacy.md.
    pub name: String,
    pub pid: u32,
}

/// Whether Ledgeling may move other apps' windows, for "make room at the top".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeAccess {
    Granted,
    /// Possible once the user allows it (macOS Accessibility).
    NeedsPermission,
    #[default]
    Unsupported,
}

pub trait Platform: Send + Sync {
    /// Visible top-level windows, front-to-back in z-order.
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String>;
    /// Seconds since the last keyboard or mouse input.
    fn idle_seconds(&self) -> Result<f64, String>;
    /// Focused application — process name only, never the title.
    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String>;
    /// Executable name for a process, lowercased and without extension, the
    /// same form `foreground_process` gives. Never a window title.
    fn process_name(&self, _pid: u32) -> Option<String> {
        None
    }
    /// Monitor geometry, work areas and scale factors.
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String>;
    /// Is a fullscreen or exclusive app running? Suspend trigger.
    fn fullscreen_app_active(&self) -> Result<bool, String>;
    /// Cursor position in virtual-desktop physical pixels.
    fn cursor(&self) -> Option<Point>;
    /// Is the primary mouse button held down? `None` when the platform cannot
    /// tell without reading input state; the overlay's pointer capture then
    /// reports the release instead.
    fn mouse_down(&self) -> Option<bool>;
    /// Monitor geometry is supplied by the window toolkit, which already
    /// reports DPI-correct work areas on every platform. The app layer passes
    /// exactly one monitor — the pet screen — which is what anchors the
    /// platform's coordinate conversion and window-to-screen tagging.
    fn set_monitors(&self, _monitors: Vec<MonitorInfo>) {}
    /// May another app's window be moved? Only "make room at the top" asks.
    fn resize_access(&self) -> ResizeAccess {
        ResizeAccess::Unsupported
    }
    /// Show the system's own permission prompt, where there is one.
    fn request_resize_access(&self) {}
    /// Move and resize another app's window to `to`, in the same physical
    /// pixels `enumerate_windows` reports. Nothing else about it is touched.
    fn set_window_frame(&self, _window: &WindowInfo, _to: Rect) -> Result<(), String> {
        Err("Moving windows isn't supported on this desktop.".into())
    }
    /// Bring another app's window to the front, because the user asked to
    /// decorate it. Never for a nudge: nudges are body language only.
    fn focus_window(&self, _window: &WindowInfo) -> Result<(), String> {
        Err("Bringing windows forward isn't supported on this desktop.".into())
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_ax;
#[cfg(target_os = "macos")]
pub use macos::NativePlatform;
#[cfg(target_os = "windows")]
pub(crate) mod windows;
#[cfg(target_os = "windows")]
pub use windows::NativePlatform;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{layer, prepare_process, set_overlay_pointer, NativePlatform};
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod fallback;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub use fallback::NativePlatform;

/// Runs before the window toolkit starts. Only Linux needs anything here.
#[cfg(not(target_os = "linux"))]
pub fn prepare_process() {}

pub mod mock;
pub mod wayland_model;
pub mod x11_model;

pub mod handoff;
#[cfg(target_os = "windows")]
mod windows_drop;

pub mod pet_ui;
