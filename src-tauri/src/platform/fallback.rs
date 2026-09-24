//! Platforms without a native adapter yet: pets live on monitor edges only.
use super::{Platform, ProcessInfo};
use crate::world::{MonitorInfo, Point, WindowInfo};
use std::sync::Mutex;

#[derive(Default)]
pub struct NativePlatform {
    monitors: Mutex<Vec<MonitorInfo>>,
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
        Ok(self.monitors.lock().map(|m| m.clone()).unwrap_or_default())
    }
    fn set_monitors(&self, monitors: Vec<MonitorInfo>) {
        if let Ok(mut cache) = self.monitors.lock() {
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
