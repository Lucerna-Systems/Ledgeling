//! World model: geometry primitives, surface extraction, and the traversal graph.
//! All coordinates are physical pixels in virtual-desktop space; negative values
//! are legal for monitors placed left of or above the primary display.
pub mod graph;
pub mod scanner;
pub mod surfaces;

use serde::{Deserialize, Serialize};

pub use graph::{Edge, EdgeKind, SurfaceGraph};
pub use scanner::{SurfaceChange, SurfaceTracker};
pub use surfaces::{surfaces, Orientation, Surface, SurfaceKey, SurfaceKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Point) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }
    pub fn lerp(self, other: Point, t: f64) -> Point {
        Point::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn right(self) -> f64 {
        self.x + self.width
    }
    pub fn bottom(self) -> f64 {
        self.y + self.height
    }
    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2., self.y + self.height / 2.)
    }
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
    pub fn intersects(self, other: Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
    /// Smallest rectangle containing both; used to span the virtual desktop.
    pub fn union(self, other: Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }
}

/// A visible top-level window, in virtual-desktop physical pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub rect: Rect,
    /// 0 = frontmost.
    pub z_index: usize,
    pub is_minimized: bool,
    pub monitor_id: u64,
    /// The owning process, where the platform can tell. Only ever turned into
    /// a name to match a biome, never kept otherwise; see `biome::apps`.
    #[serde(default)]
    pub pid: Option<u32>,
}
impl WindowInfo {
    pub fn new(id: u64, rect: Rect) -> Self {
        Self {
            id,
            rect,
            z_index: 0,
            is_minimized: false,
            monitor_id: 0,
            pid: None,
        }
    }
    pub fn with_pid(self, pid: u32) -> Self {
        Self {
            pid: Some(pid),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: u64,
    pub bounds: Rect,
    /// Work area excludes the taskbar / menu bar / dock.
    pub work: Rect,
    pub scale: f64,
    pub is_primary: bool,
}
impl MonitorInfo {
    /// Monitors and windows share the owner-id namespace, so monitor-owned
    /// surfaces are tagged above any plausible window handle.
    pub fn owner(&self) -> u64 {
        MONITOR_OWNER_BASE + self.id
    }
}
pub const MONITOR_OWNER_BASE: u64 = 1 << 56;
/// Biome props share the namespace too, above monitors: slot × 64 + prop.
pub const PROP_OWNER_BASE: u64 = 1 << 57;

/// The rectangle spanning every monitor. Empty input yields a zero rect.
pub fn virtual_desktop(monitors: &[MonitorInfo]) -> Rect {
    monitors
        .iter()
        .map(|m| m.bounds)
        .reduce(Rect::union)
        .unwrap_or_default()
}

/// The monitor containing a point, else the nearest one by centre distance.
pub fn monitor_at(monitors: &[MonitorInfo], p: Point) -> Option<&MonitorInfo> {
    monitors.iter().find(|m| m.bounds.contains(p)).or_else(|| {
        monitors.iter().min_by(|a, b| {
            a.bounds
                .center()
                .distance(p)
                .total_cmp(&b.bounds.center().distance(p))
        })
    })
}
