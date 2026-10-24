//! Every OS query sits behind this trait so pet behavior can be tested without
//! a desktop and ported without touching the engine.
use crate::world::{MonitorInfo, Point, WindowInfo};

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessInfo {
    /// Executable name, lowercased, without extension. The window title is
    /// never read; see docs/privacy.md.
    pub name: String,
    pub pid: u32,
}

pub trait Platform: Send + Sync {
    /// Visible top-level windows, front-to-back in z-order.
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String>;
    /// Seconds since the last keyboard or mouse input.
    fn idle_seconds(&self) -> Result<f64, String>;
    /// Focused application — process name only, never the title.
    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String>;
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
}

#[cfg(target_os = "macos")]
mod macos;
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
