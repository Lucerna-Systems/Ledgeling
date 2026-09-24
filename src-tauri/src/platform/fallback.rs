//! Platforms without a native adapter yet: pets live on monitor edges only.
use super::{Platform, ProcessInfo};
use crate::world::{MonitorInfo, Point, WindowInfo};
use std::sync::{Mutex, OnceLock};

/// A unit struct like the macOS and Windows adapters, so the app constructs
/// every platform the same way; the monitor cache lives beside it.
#[derive(Default)]
pub struct NativePlatform;

fn monitor_cache() -> &'static Mutex<Vec<MonitorInfo>> {
    static CACHE: OnceLock<Mutex<Vec<MonitorInfo>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}
impl Platform for NativePlatform {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String> {
        Err(
            "Real window edges need a native adapter for this platform. Using the desktop edge."
                .into(),
        )
    }
    fn idle_seconds(&self) -> Result<f64, String> {
        Ok(0.)
    }
    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String> {
        Ok(None)
    }
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        Ok(monitor_cache()
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default())
    }
    fn set_monitors(&self, monitors: Vec<MonitorInfo>) {
        if let Ok(mut cache) = monitor_cache().lock() {
            *cache = monitors;
        }
    }
    fn fullscreen_app_active(&self) -> Result<bool, String> {
        Ok(false)
    }
    fn cursor(&self) -> Option<Point> {
        None
    }
    fn mouse_down(&self) -> Option<bool> {
        None
    }
}
