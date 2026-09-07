//! Step 6 of the surface build pipeline: diff successive scans so pets can
//! react to edges appearing, moving and disappearing rather than polling.
use super::{Point, Surface, SurfaceKey};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceChange {
    Added(SurfaceKey),
    Lost(SurfaceKey),
    Moved {
        key: SurfaceKey,
        /// Physical pixels per second, used to decide whether to look startled.
        speed: f64,
    },
}

#[derive(Default)]
pub struct SurfaceTracker {
    previous: HashMap<SurfaceKey, (Point, Point)>,
}

impl SurfaceTracker {
    /// `dt` is the time since the previous scan, in seconds.
    pub fn diff(&mut self, surfaces: &[Surface], dt: f64) -> Vec<SurfaceChange> {
        let mut current: HashMap<SurfaceKey, (Point, Point)> = HashMap::new();
        for s in surfaces {
            current.entry(s.key()).or_insert((s.span_start, s.span_end));
        }
        let mut changes = Vec::new();
        for (key, span) in &current {
            match self.previous.get(key) {
                None => changes.push(SurfaceChange::Added(*key)),
                Some(old) => {
                    let moved = old.0.distance(span.0).max(old.1.distance(span.1));
                    if moved > 0.5 {
                        changes.push(SurfaceChange::Moved {
                            key: *key,
                            speed: if dt > 1e-4 { moved / dt } else { 0. },
                        });
                    }
                }
            }
        }
        for key in self.previous.keys() {
            if !current.contains_key(key) {
                changes.push(SurfaceChange::Lost(*key));
            }
        }
        self.previous = current;
        changes
    }
}
