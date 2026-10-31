//! Scripted platform for integration tests: replay a window-layout timeline so
//! "the user dragged a window while the squirrel was climbing it" is repeatable.
use super::{Platform, ProcessInfo, ResizeAccess};
use crate::world::{MonitorInfo, Point, Rect, WindowInfo};
use std::{collections::HashMap, sync::Mutex};

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
    /// Executable names by pid, for windows that carry one.
    pub names: Mutex<HashMap<u32, String>>,
    /// How many times a name was asked for, so tests can see the cache work.
    pub name_lookups: Mutex<usize>,
    pub access: Mutex<ResizeAccess>,
    /// Every window move asked for, in order. A move also changes the window
    /// in the current timeline frame, as the real desktop would.
    pub moves: Mutex<Vec<(u64, Rect)>>,
    /// Windows that refuse to be moved, like an elevated app on Windows.
    pub stubborn: Mutex<Vec<u64>>,
    /// Windows brought to the front, in order.
    pub focused: Mutex<Vec<u64>>,
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
    fn process_name(&self, pid: u32) -> Option<String> {
        *self.name_lookups.lock().unwrap() += 1;
        self.names.lock().unwrap().get(&pid).cloned()
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
    fn resize_access(&self) -> ResizeAccess {
        *self.access.lock().unwrap()
    }
    fn focus_window(&self, window: &WindowInfo) -> Result<(), String> {
        self.focused.lock().unwrap().push(window.id);
        Ok(())
    }
    fn set_window_frame(&self, window: &WindowInfo, to: Rect) -> Result<(), String> {
        if *self.access.lock().unwrap() != ResizeAccess::Granted {
            return Err("No permission.".into());
        }
        if self.stubborn.lock().unwrap().contains(&window.id) {
            return Err("That window refused.".into());
        }
        self.moves.lock().unwrap().push((window.id, to));
        let i = *self.cursor_frame.lock().unwrap();
        if let Some(frame) = self.frames.lock().unwrap().get_mut(i) {
            for w in frame.iter_mut().filter(|w| w.id == window.id) {
                w.rect = to;
            }
        }
        Ok(())
    }
}
