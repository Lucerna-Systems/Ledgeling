use crate::world::{Point, Rect, WindowInfo};

#[derive(Clone, Copy)]
pub struct Display {
    pub bounds: Rect,
    pub work: Rect,
    pub scale: f64,
}
pub trait Platform: Send + Sync {
    fn enumerate_windows(&self, scale: f64) -> Result<Vec<WindowInfo>, String>;
    fn cursor(&self, scale: f64) -> Option<Point>;
    fn mouse_down(&self) -> bool;
}
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::NativePlatform;
#[cfg(not(target_os = "macos"))]
mod fallback;
#[cfg(not(target_os = "macos"))]
pub use fallback::NativePlatform;
