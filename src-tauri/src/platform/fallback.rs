use super::Platform;
use crate::world::{Point, WindowInfo};
pub struct NativePlatform;
impl Platform for NativePlatform {
    fn enumerate_windows(&self, _: f64) -> Result<Vec<WindowInfo>, String> {
        Err("Real window edges are currently available in the macOS MVP. This platform uses the desktop edge.".into())
    }
    fn cursor(&self, _: f64) -> Option<Point> {
        None
    }
    fn mouse_down(&self) -> bool {
        false
    }
}
