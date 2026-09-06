use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn right(self) -> f64 {
        self.x + self.width
    }
    pub fn bottom(self) -> f64 {
        self.y + self.height
    }
    #[cfg(test)]
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WindowInfo {
    pub id: u64,
    pub rect: Rect,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Surface {
    pub owner: u64,
    pub start: f64,
    pub end: f64,
    pub y: f64,
    // Parametric position uses the owner's entire top edge, so occlusion changes
    // don't slide a perched bird along the remaining fragment.
    pub owner_start: f64,
    pub owner_width: f64,
}
impl Surface {
    pub fn point(&self, t: f64) -> Point {
        Point {
            x: self.owner_start + t * self.owner_width,
            y: self.y,
        }
    }
    pub fn fits(&self, t: f64, half_width: f64) -> bool {
        let x = self.point(t).x;
        x - half_width >= self.start && x + half_width <= self.end
    }
    pub fn midpoint_t(&self) -> f64 {
        ((self.start + self.end) / 2. - self.owner_start) / self.owner_width
    }
}

/// Visible top edges only: M0's flier proof. Input order is front-to-back.
/// All geometry is in physical pixels on the MVP's primary display.
pub fn surfaces(windows: &[WindowInfo], work: Rect, scale: f64) -> Vec<Surface> {
    let mut out = Vec::new();
    let minimum = 80. * scale;
    for (i, window) in windows.iter().enumerate() {
        let r = window.rect;
        // Leave headroom for the sprite; don't mistake the clipped top of an
        // off-screen window for a real edge.
        if r.width <= 0. || r.y < work.y + 100. * scale || r.y > work.bottom() {
            continue;
        }
        let mut segments = vec![(r.x.max(work.x), r.right().min(work.right()))];
        for front in &windows[..i] {
            let f = front.rect;
            if f.y <= r.y && f.bottom() >= r.y {
                segments = segments
                    .into_iter()
                    .flat_map(|(a, b)| subtract(a, b, f.x, f.right()))
                    .collect();
            }
        }
        for (start, end) in segments {
            if end - start >= minimum {
                out.push(Surface {
                    owner: window.id,
                    start,
                    end,
                    y: r.y,
                    owner_start: r.x,
                    owner_width: r.width,
                });
            }
        }
    }
    out.push(Surface {
        owner: 0,
        start: work.x,
        end: work.right(),
        y: work.bottom(),
        owner_start: work.x,
        owner_width: work.width,
    });
    out
}
fn subtract(a: f64, b: f64, left: f64, right: f64) -> Vec<(f64, f64)> {
    if right <= a || left >= b {
        return vec![(a, b)];
    }
    let mut pieces = Vec::new();
    if left > a {
        pieces.push((a, left.min(b)));
    }
    if right < b {
        pieces.push((right.max(a), b));
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work() -> Rect {
        Rect {
            x: 0.,
            y: 0.,
            width: 1000.,
            height: 800.,
        }
    }
    fn win(id: u64, x: f64, y: f64, w: f64, h: f64) -> WindowInfo {
        WindowInfo {
            id,
            rect: Rect {
                x,
                y,
                width: w,
                height: h,
            },
        }
    }
    #[test]
    fn fully_occluded_edges_are_absent() {
        let s = surfaces(
            &[win(1, 0., 150., 600., 400.), win(2, 10., 200., 500., 300.)],
            work(),
            1.,
        );
        assert!(!s.iter().any(|s| s.owner == 2));
    }
    #[test]
    fn partial_occlusion_splits_the_edge() {
        let s = surfaces(
            &[win(1, 300., 150., 200., 300.), win(2, 0., 250., 800., 300.)],
            work(),
            1.,
        );
        let pieces: Vec<_> = s
            .iter()
            .filter(|s| s.owner == 2)
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(pieces, vec![(0., 300.), (500., 800.)]);
    }
    #[test]
    fn offscreen_edges_are_clipped_without_inventing_a_top() {
        let s = surfaces(
            &[
                win(1, -200., 150., 500., 300.),
                win(2, 50., -30., 400., 300.),
            ],
            work(),
            1.,
        );
        assert_eq!((s[0].start, s[0].end), (0., 300.));
        assert!(!s.iter().any(|s| s.owner == 2));
    }
    #[test]
    fn narrow_fragments_are_rejected_and_floor_always_exists() {
        let s = surfaces(&[win(1, 0., 200., 79., 100.)], work(), 1.);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].owner, 0);
    }
    #[test]
    fn touching_occluder_counts_and_frontmost_edge_remains() {
        let s = surfaces(
            &[win(1, 0., 200., 400., 100.), win(2, 0., 300., 400., 100.)],
            work(),
            1.,
        );
        assert!(s.iter().any(|s| s.owner == 1));
        assert!(!s.iter().any(|s| s.owner == 2));
    }
    #[test]
    fn physical_dpi_and_negative_coordinates() {
        let work = Rect {
            x: -2000.,
            y: 0.,
            width: 2000.,
            height: 1600.,
        };
        let s = surfaces(&[win(1, -1800., 400., 800., 500.)], work, 2.);
        assert_eq!(s[0].point(0.5), Point { x: -1400., y: 400. });
        assert!(!s[0].fits(0., 64.));
    }
}
