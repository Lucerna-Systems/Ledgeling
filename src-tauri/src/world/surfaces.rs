//! Step 1-4 of the surface build pipeline: edge extraction, z-order occlusion
//! subtraction, permanent monitor surfaces, and short-segment rejection.
use super::{MonitorInfo, Point, Rect, WindowInfo};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    WindowTop,
    WindowBottom,
    WindowLeft,
    WindowRight,
    MonitorTop,
    MonitorBottom,
    MonitorLeft,
    MonitorRight,
    TaskbarTop,
    /// A ledge on a biome prop, such as the top of a bush. Never named by a
    /// pack: a pet may stand here wherever it may stand on a window top.
    PropTop,
}
impl SurfaceKind {
    pub const ALL: [SurfaceKind; 9] = [
        SurfaceKind::WindowTop,
        SurfaceKind::WindowBottom,
        SurfaceKind::WindowLeft,
        SurfaceKind::WindowRight,
        SurfaceKind::MonitorTop,
        SurfaceKind::MonitorBottom,
        SurfaceKind::MonitorLeft,
        SurfaceKind::MonitorRight,
        SurfaceKind::TaskbarTop,
    ];
    pub fn orientation(self) -> Orientation {
        use SurfaceKind::*;
        match self {
            WindowLeft | WindowRight | MonitorLeft | MonitorRight => Orientation::Vertical,
            _ => Orientation::Horizontal,
        }
    }
    /// Which way is "up" for a pet standing here.
    pub fn normal(self) -> Point {
        use SurfaceKind::*;
        match self {
            WindowTop | MonitorBottom | TaskbarTop | PropTop => Point::new(0., -1.),
            WindowBottom | MonitorTop => Point::new(0., 1.),
            WindowLeft | MonitorRight => Point::new(-1., 0.),
            WindowRight | MonitorLeft => Point::new(1., 0.),
        }
    }
    pub fn is_window(self) -> bool {
        use SurfaceKind::*;
        matches!(self, WindowTop | WindowBottom | WindowLeft | WindowRight)
    }
    pub fn as_str(self) -> &'static str {
        use SurfaceKind::*;
        match self {
            WindowTop => "window_top",
            WindowBottom => "window_bottom",
            WindowLeft => "window_left",
            WindowRight => "window_right",
            MonitorTop => "monitor_top",
            MonitorBottom => "monitor_bottom",
            MonitorLeft => "monitor_left",
            MonitorRight => "monitor_right",
            TaskbarTop => "taskbar_top",
            PropTop => "prop_top",
        }
    }
    pub fn parse(name: &str) -> Option<SurfaceKind> {
        SurfaceKind::ALL.into_iter().find(|k| k.as_str() == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Identity of an edge independent of how occlusion currently splits it, so a
/// pet's `(surface, t)` anchor survives another window sliding across it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SurfaceKey {
    pub owner: u64,
    pub kind: SurfaceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    pub owner: u64,
    pub kind: SurfaceKind,
    /// Ordinal of this visible fragment within its edge, front-to-back stable.
    pub index: u32,
    pub start: Point,
    pub end: Point,
    /// The whole edge, occluded parts included: the basis for `t`.
    pub span_start: Point,
    pub span_end: Point,
    pub monitor: u64,
}
impl Surface {
    pub fn key(&self) -> SurfaceKey {
        SurfaceKey {
            owner: self.owner,
            kind: self.kind,
        }
    }
    pub fn orientation(&self) -> Orientation {
        self.kind.orientation()
    }
    pub fn normal(&self) -> Point {
        self.kind.normal()
    }
    pub fn span_length(&self) -> f64 {
        self.span_start.distance(self.span_end)
    }
    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }
    pub fn point(&self, t: f64) -> Point {
        self.span_start.lerp(self.span_end, t)
    }
    pub fn midpoint(&self) -> Point {
        self.start.lerp(self.end, 0.5)
    }
    /// `t` of the visible fragment's midpoint.
    pub fn midpoint_t(&self) -> f64 {
        self.t_of(self.midpoint())
    }
    /// Projection of a world point onto this edge's parameter, unclamped ends
    /// excluded: always within the visible fragment.
    pub fn t_of(&self, p: Point) -> f64 {
        let len = self.span_length();
        if len <= f64::EPSILON {
            return 0.;
        }
        let dx = (self.span_end.x - self.span_start.x) / len;
        let dy = (self.span_end.y - self.span_start.y) / len;
        let raw = ((p.x - self.span_start.x) * dx + (p.y - self.span_start.y) * dy) / len;
        raw.clamp(self.t_start(), self.t_end())
    }
    pub fn t_start(&self) -> f64 {
        self.raw_t(self.start)
    }
    pub fn t_end(&self) -> f64 {
        self.raw_t(self.end)
    }
    fn raw_t(&self, p: Point) -> f64 {
        let len = self.span_length();
        if len <= f64::EPSILON {
            0.
        } else {
            self.span_start.distance(p) / len
        }
    }
    /// Is `t` on the visible fragment with `half_width` of clearance each side?
    pub fn fits(&self, t: f64, half_width: f64) -> bool {
        let p = self.point(t);
        let along = |q: Point| p.distance(q);
        t >= self.t_start()
            && t <= self.t_end()
            && along(self.start) >= half_width - 0.001
            && along(self.end) >= half_width - 0.001
    }
    pub fn contains_t(&self, t: f64) -> bool {
        t >= self.t_start() - 1e-9 && t <= self.t_end() + 1e-9
    }
    /// Nearest point on the visible fragment.
    pub fn nearest(&self, p: Point) -> Point {
        self.point(self.t_of(p))
    }
}

/// How close, in logical pixels, a window edge must be to a screen edge to count
/// as pushed up against it.
const FLUSH: f64 = 4.;

/// Extract every visible edge. `windows` must be front-to-back (index 0 frontmost).
/// `min_segment` is logical pixels and is scaled per monitor.
pub fn surfaces(
    windows: &[WindowInfo],
    monitors: &[MonitorInfo],
    min_segment: f64,
) -> Vec<Surface> {
    let mut out = Vec::new();
    for (i, window) in windows.iter().enumerate() {
        if window.is_minimized || window.rect.width <= 0. || window.rect.height <= 0. {
            continue;
        }
        let occluders: Vec<Rect> = windows[..i]
            .iter()
            .filter(|w| !w.is_minimized)
            .map(|w| w.rect)
            .collect();
        for kind in [
            SurfaceKind::WindowTop,
            SurfaceKind::WindowBottom,
            SurfaceKind::WindowLeft,
            SurfaceKind::WindowRight,
        ] {
            let (span_start, span_end) = window_edge(window.rect, kind);
            push_segments(
                &mut out,
                window.id,
                kind,
                span_start,
                span_end,
                &occluders,
                monitors,
                min_segment,
            );
        }
    }
    for monitor in monitors {
        let work = monitor.work;
        let floor_is_taskbar = monitor.work.bottom() < monitor.bounds.bottom() - 0.5;
        let mut edges = vec![
            (
                SurfaceKind::MonitorTop,
                Point::new(work.x, work.y),
                Point::new(work.right(), work.y),
            ),
            (
                SurfaceKind::MonitorLeft,
                Point::new(work.x, work.y),
                Point::new(work.x, work.bottom()),
            ),
            (
                SurfaceKind::MonitorRight,
                Point::new(work.right(), work.y),
                Point::new(work.right(), work.bottom()),
            ),
            // The usable floor is always reachable, whether or not a bar is there:
            // packs that list only `taskbar_top` still get somewhere to land.
            (
                SurfaceKind::TaskbarTop,
                Point::new(work.x, work.bottom()),
                Point::new(work.right(), work.bottom()),
            ),
        ];
        if floor_is_taskbar {
            edges.push((
                SurfaceKind::MonitorBottom,
                Point::new(monitor.bounds.x, monitor.bounds.bottom()),
                Point::new(monitor.bounds.right(), monitor.bounds.bottom()),
            ));
        }
        for (kind, a, b) in edges {
            let pieces = if kind == SurfaceKind::MonitorTop {
                ceiling_pieces(a, b, windows, monitor, min_segment)
            } else {
                vec![(a, b)]
            };
            for (index, (start, end)) in pieces.into_iter().enumerate() {
                out.push(Surface {
                    owner: monitor.owner(),
                    kind,
                    index: index as u32,
                    start,
                    end,
                    span_start: a,
                    span_end: b,
                    monitor: monitor.id,
                });
            }
        }
    }
    out
}

/// Hanging from the ceiling above a window pushed up against it would draw the
/// pet upside down over that window's title bar; the window's own top edge is
/// the same line, the right way up.
fn ceiling_pieces(
    a: Point,
    b: Point,
    windows: &[WindowInfo],
    monitor: &MonitorInfo,
    min_segment: f64,
) -> Vec<(Point, Point)> {
    let mut pieces = vec![(a.x, b.x)];
    for w in windows.iter().filter(|w| !w.is_minimized) {
        if w.rect.y <= a.y + FLUSH * monitor.scale && w.rect.bottom() > a.y {
            pieces = pieces
                .into_iter()
                .flat_map(|(s, e)| subtract(s, e, w.rect.x, w.rect.right()))
                .collect();
        }
    }
    pieces
        .into_iter()
        .filter(|(s, e)| e - s >= min_segment * monitor.scale)
        .map(|(s, e)| (Point::new(s, a.y), Point::new(e, a.y)))
        .collect()
}

fn window_edge(r: Rect, kind: SurfaceKind) -> (Point, Point) {
    match kind {
        SurfaceKind::WindowTop => (Point::new(r.x, r.y), Point::new(r.right(), r.y)),
        SurfaceKind::WindowBottom => (
            Point::new(r.x, r.bottom()),
            Point::new(r.right(), r.bottom()),
        ),
        SurfaceKind::WindowLeft => (Point::new(r.x, r.y), Point::new(r.x, r.bottom())),
        SurfaceKind::WindowRight => (
            Point::new(r.right(), r.y),
            Point::new(r.right(), r.bottom()),
        ),
        _ => (Point::default(), Point::default()),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_segments(
    out: &mut Vec<Surface>,
    owner: u64,
    kind: SurfaceKind,
    span_start: Point,
    span_end: Point,
    occluders: &[Rect],
    monitors: &[MonitorInfo],
    min_segment: f64,
) {
    let vertical = kind.orientation() == Orientation::Vertical;
    let (line, lo, hi) = if vertical {
        (span_start.x, span_start.y, span_end.y)
    } else {
        (span_start.y, span_start.x, span_end.x)
    };
    let mut pieces = vec![(lo, hi)];
    for f in occluders {
        let hides = if vertical {
            f.x <= line && f.right() >= line
        } else {
            f.y <= line && f.bottom() >= line
        };
        if !hides {
            continue;
        }
        let (a, b) = if vertical {
            (f.y, f.bottom())
        } else {
            (f.x, f.right())
        };
        pieces = pieces
            .into_iter()
            .flat_map(|(s, e)| subtract(s, e, a, b))
            .collect();
    }
    let mut index = 0;
    for (s, e) in pieces {
        for monitor in monitors {
            let w = monitor.work;
            // A window edge outside the usable area is not somewhere a pet can be.
            let on_monitor = if vertical {
                line >= w.x - 0.5 && line <= w.right() + 0.5
            } else {
                line >= w.y - 0.5 && line <= w.bottom() + 0.5
            };
            // A pet stands on the outward side of an edge, so one flush with the
            // screen's side or floor would be drawn off-screen or under the dock.
            let faces_out = match kind {
                SurfaceKind::WindowLeft => line <= w.x + FLUSH * monitor.scale,
                SurfaceKind::WindowRight => line >= w.right() - FLUSH * monitor.scale,
                SurfaceKind::WindowBottom => line >= w.bottom() - FLUSH * monitor.scale,
                _ => false,
            };
            if !on_monitor || faces_out {
                continue;
            }
            let (clip_lo, clip_hi) = if vertical {
                (w.y, w.bottom())
            } else {
                (w.x, w.right())
            };
            let (a, b) = (s.max(clip_lo), e.min(clip_hi));
            if b - a < min_segment * monitor.scale {
                continue;
            }
            let (start, end) = if vertical {
                (Point::new(line, a), Point::new(line, b))
            } else {
                (Point::new(a, line), Point::new(b, line))
            };
            out.push(Surface {
                owner,
                kind,
                index,
                start,
                end,
                span_start,
                span_end,
                monitor: monitor.id,
            });
            index += 1;
        }
    }
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

    fn monitor(id: u64, x: f64, y: f64, w: f64, h: f64, scale: f64) -> MonitorInfo {
        MonitorInfo {
            id,
            bounds: Rect::new(x, y, w, h),
            work: Rect::new(x, y, w, h - 40.),
            scale,
            is_primary: id == 0,
        }
    }
    fn screen() -> Vec<MonitorInfo> {
        vec![monitor(0, 0., 0., 1600., 1000., 1.)]
    }
    fn win(id: u64, x: f64, y: f64, w: f64, h: f64) -> WindowInfo {
        WindowInfo::new(id, Rect::new(x, y, w, h))
    }
    fn of(surfaces: &[Surface], owner: u64, kind: SurfaceKind) -> Vec<(f64, f64)> {
        surfaces
            .iter()
            .filter(|s| s.owner == owner && s.kind == kind)
            .map(|s| {
                if s.orientation() == Orientation::Vertical {
                    (s.start.y, s.end.y)
                } else {
                    (s.start.x, s.end.x)
                }
            })
            .collect()
    }

    #[test]
    fn every_edge_of_a_lone_window_is_visible() {
        let s = surfaces(&[win(1, 200., 200., 400., 300.)], &screen(), 48.);
        assert_eq!(of(&s, 1, SurfaceKind::WindowTop), vec![(200., 600.)]);
        assert_eq!(of(&s, 1, SurfaceKind::WindowBottom), vec![(200., 600.)]);
        assert_eq!(of(&s, 1, SurfaceKind::WindowLeft), vec![(200., 500.)]);
        assert_eq!(of(&s, 1, SurfaceKind::WindowRight), vec![(200., 500.)]);
    }

    #[test]
    fn a_fully_covered_window_reports_no_edges_at_all() {
        // Window 2 is entirely behind window 1: none of its edges are visible.
        let s = surfaces(
            &[
                win(1, 100., 100., 800., 600.),
                win(2, 200., 200., 300., 200.),
            ],
            &screen(),
            48.,
        );
        assert!(
            !s.iter().any(|s| s.owner == 2),
            "hidden edges must not be offered"
        );
        assert!(s.iter().any(|s| s.owner == 1));
    }

    #[test]
    fn partial_overlap_splits_an_edge_into_visible_fragments() {
        let s = surfaces(
            &[
                win(1, 500., 100., 200., 400.),
                win(2, 100., 300., 900., 300.),
            ],
            &screen(),
            48.,
        );
        assert_eq!(
            of(&s, 2, SurfaceKind::WindowTop),
            vec![(100., 500.), (700., 1000.)]
        );
    }

    #[test]
    fn nested_windows_leave_only_the_frame_around_the_inner_one() {
        // The inner window is in front, so the outer window's top survives on
        // both sides but its own edges stay whole.
        let s = surfaces(
            &[
                win(2, 300., 250., 200., 200.),
                win(1, 100., 200., 800., 500.),
            ],
            &screen(),
            48.,
        );
        assert_eq!(
            of(&s, 1, SurfaceKind::WindowTop),
            vec![(100., 900.)],
            "the inner window does not reach the outer top edge"
        );
        assert_eq!(of(&s, 2, SurfaceKind::WindowTop), vec![(300., 500.)]);
    }

    #[test]
    fn offscreen_windows_are_clipped_not_invented() {
        let s = surfaces(
            &[
                win(1, -300., 400., 500., 200.),
                win(2, 200., -200., 400., 300.),
            ],
            &screen(),
            48.,
        );
        assert_eq!(of(&s, 1, SurfaceKind::WindowTop), vec![(0., 200.)]);
        assert!(
            of(&s, 2, SurfaceKind::WindowTop).is_empty(),
            "a top edge above the screen is not a perch"
        );
        assert_eq!(of(&s, 2, SurfaceKind::WindowBottom), vec![(200., 600.)]);
    }

    #[test]
    fn segments_shorter_than_the_minimum_are_discarded() {
        let s = surfaces(&[win(1, 100., 300., 40., 200.)], &screen(), 48.);
        assert!(of(&s, 1, SurfaceKind::WindowTop).is_empty());
        assert!(!of(&s, 1, SurfaceKind::WindowLeft).is_empty());
    }

    #[test]
    fn multi_monitor_handles_negative_coordinates_and_mixed_dpi() {
        let monitors = vec![
            monitor(0, 0., 0., 1600., 1000., 1.),
            monitor(1, -2560., -200., 2560., 1440., 2.),
        ];
        let s = surfaces(&[win(1, -2000., 300., 800., 400.)], &monitors, 48.);
        let tops: Vec<&Surface> = s
            .iter()
            .filter(|s| s.owner == 1 && s.kind == SurfaceKind::WindowTop)
            .collect();
        assert_eq!(tops.len(), 1);
        assert_eq!(tops[0].monitor, 1);
        assert_eq!(tops[0].point(0.5), Point::new(-1600., 300.));
        // The 48 px minimum is scaled by the monitor it lands on.
        let narrow = surfaces(&[win(2, -2000., 300., 80., 400.)], &monitors, 48.);
        assert!(!narrow
            .iter()
            .any(|s| s.owner == 2 && s.kind == SurfaceKind::WindowTop));
    }

    #[test]
    fn every_monitor_always_offers_a_floor_and_its_own_edges() {
        let s = surfaces(&[], &screen(), 48.);
        for kind in [
            SurfaceKind::TaskbarTop,
            SurfaceKind::MonitorTop,
            SurfaceKind::MonitorLeft,
            SurfaceKind::MonitorRight,
            SurfaceKind::MonitorBottom,
        ] {
            assert!(
                s.iter().any(|s| s.kind == kind),
                "{} should always exist",
                kind.as_str()
            );
        }
    }

    #[test]
    fn parametric_position_survives_an_edge_being_split() {
        let whole = surfaces(&[win(1, 200., 300., 600., 200.)], &screen(), 48.);
        let top = whole
            .iter()
            .find(|s| s.owner == 1 && s.kind == SurfaceKind::WindowTop)
            .unwrap();
        let t = top.t_of(Point::new(700., 300.));
        let split = surfaces(
            &[
                win(2, 300., 200., 200., 300.),
                win(1, 200., 300., 600., 200.),
            ],
            &screen(),
            48.,
        );
        let fragment = split
            .iter()
            .find(|s| s.owner == 1 && s.kind == SurfaceKind::WindowTop && s.contains_t(t))
            .expect("the right-hand fragment still holds the pet's position");
        assert_eq!(fragment.point(t), Point::new(700., 300.));
    }

    #[test]
    fn window_sides_flush_with_the_screen_edge_are_not_perches() {
        // A pet stands on the outside of an edge; against the screen side or
        // floor that outside is off-screen.
        let s = surfaces(&[win(1, 0., 100., 800., 860.)], &screen(), 48.);
        assert!(of(&s, 1, SurfaceKind::WindowLeft).is_empty());
        assert!(of(&s, 1, SurfaceKind::WindowBottom).is_empty());
        assert_eq!(of(&s, 1, SurfaceKind::WindowRight), vec![(100., 960.)]);
        assert_eq!(of(&s, 1, SurfaceKind::WindowTop), vec![(0., 800.)]);
    }

    #[test]
    fn the_ceiling_above_a_window_pushed_against_it_is_not_a_perch() {
        // Hanging there would draw the pet upside down over the title bar.
        let owner = screen()[0].owner();
        let s = surfaces(&[win(1, 400., 0., 600., 500.)], &screen(), 48.);
        assert_eq!(
            of(&s, owner, SurfaceKind::MonitorTop),
            vec![(0., 400.), (1000., 1600.)]
        );
        assert_eq!(of(&s, 1, SurfaceKind::WindowTop), vec![(400., 1000.)]);
        let maximised = surfaces(&[win(1, 0., 0., 1600., 960.)], &screen(), 48.);
        assert!(of(&maximised, owner, SurfaceKind::MonitorTop).is_empty());
    }

    #[test]
    fn surfaces_know_which_way_is_up() {
        assert_eq!(SurfaceKind::WindowTop.normal(), Point::new(0., -1.));
        assert_eq!(SurfaceKind::WindowBottom.normal(), Point::new(0., 1.));
        assert_eq!(SurfaceKind::WindowLeft.orientation(), Orientation::Vertical);
        assert_eq!(
            SurfaceKind::TaskbarTop.orientation(),
            Orientation::Horizontal
        );
    }
}
