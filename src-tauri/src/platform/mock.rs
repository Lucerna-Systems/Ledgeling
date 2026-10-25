//! Scripted platform for integration tests: replay a window-layout timeline so
//! "the user dragged a window while the squirrel was climbing it" is repeatable.
use super::{Platform, ProcessInfo};
use crate::world::{MonitorInfo, Point, WindowInfo};
use std::sync::Mutex;

#[derive(Default)]
pub struct MockPlatform {
    pub frames: Mutex<Vec<Vec<WindowInfo>>>,
    pub monitors: Mutex<Vec<MonitorInfo>>,
    pub idle: Mutex<f64>,
    pub process: Mutex<Option<ProcessInfo>>,
    pub fullscreen: Mutex<bool>,
    pub cursor: Mutex<Option<Point>>,
    pub mouse_down: Mutex<bool>,
    /// Index of the next timeline frame; the last frame repeats forever.
    pub cursor_frame: Mutex<usize>,
}
impl MockPlatform {
    pub fn new(monitors: Vec<MonitorInfo>, frames: Vec<Vec<WindowInfo>>) -> Self {
        Self {
            frames: Mutex::new(frames),
            monitors: Mutex::new(monitors),
            ..Default::default()
        }
    }
    pub fn advance(&self) {
        let last = self.frames.lock().unwrap().len().saturating_sub(1);
        let mut i = self.cursor_frame.lock().unwrap();
        *i = (*i + 1).min(last);
    }
}
impl Platform for MockPlatform {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, String> {
        let frames = self.frames.lock().unwrap();
        let i = *self.cursor_frame.lock().unwrap();
        Ok(frames.get(i).cloned().unwrap_or_default())
    }
    fn idle_seconds(&self) -> Result<f64, String> {
        Ok(*self.idle.lock().unwrap())
    }
    fn foreground_process(&self) -> Result<Option<ProcessInfo>, String> {
        Ok(self.process.lock().unwrap().clone())
    }
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        Ok(self.monitors.lock().unwrap().clone())
    }
    fn set_monitors(&self, monitors: Vec<MonitorInfo>) {
        *self.monitors.lock().unwrap() = monitors;
    }
    fn fullscreen_app_active(&self) -> Result<bool, String> {
        Ok(*self.fullscreen.lock().unwrap())
    }
    fn cursor(&self) -> Option<Point> {
        *self.cursor.lock().unwrap()
    }
    fn mouse_down(&self) -> Option<bool> {
        Some(*self.mouse_down.lock().unwrap())
    }
}
