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
    /// Is the primary mouse button held down?
    fn mouse_down(&self) -> bool;
    /// Monitor geometry is supplied by the window toolkit, which already
    /// reports DPI-correct work areas on every platform.
    fn set_monitors(&self, _monitors: Vec<MonitorInfo>) {}
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::NativePlatform;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::NativePlatform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod fallback;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use fallback::NativePlatform;

pub mod mock;
