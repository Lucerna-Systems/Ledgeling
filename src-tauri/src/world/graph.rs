//! Step 5 of the surface build pipeline: connect segments into a traversal
//! graph. The graph is shared by every locomotion class; only the traversal
//! ruleset differs, so adding a species is data and adding a class is code.
use super::{Point, Surface};
use serde::Serialize;
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Segments meeting at a corner: walkable without leaving a surface.
    Contiguous,
    /// Separated by open air, close enough that a jump could bridge it.
    Jumpable,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub kind: EdgeKind,
    /// Where the traversal leaves `a` and arrives on `b`.
    pub from: Point,
    pub to: Point,
    pub gap: f64,
}

/// Widest gap the graph bothers to record. Pets filter further by `max_gap`.
pub const MAX_RECORDED_GAP: f64 = 420.;
const CORNER_EPSILON: f64 = 3.;

#[derive(Default)]
pub struct SurfaceGraph {
    pub edges: Vec<Edge>,
    adjacency: Vec<Vec<usize>>,
}

impl SurfaceGraph {
    pub fn build(surfaces: &[Surface]) -> Self {
        let mut edges = Vec::new();
        let mut adjacency = vec![Vec::new(); surfaces.len()];
        for i in 0..surfaces.len() {
            for j in (i + 1)..surfaces.len() {
                let (pa, pb, gap) = closest_points(surfaces[i], surfaces[j]);
                if gap > MAX_RECORDED_GAP {
                    continue;
                }
                let kind = if gap <= CORNER_EPSILON {
                    EdgeKind::Contiguous
                } else {
                    EdgeKind::Jumpable
                };
                adjacency[i].push(edges.len());
                adjacency[j].push(edges.len());
                edges.push(Edge {
                    a: i,
                    b: j,
                    kind,
                    from: pa,
                    to: pb,
                    gap,
                });
            }
        }
        Self { edges, adjacency }
    }

    /// Edges leaving `surface`, already oriented so `from` is on it.
    pub fn outgoing(&self, surface: usize) -> impl Iterator<Item = Edge> + '_ {
        self.adjacency
            .get(surface)
            .into_iter()
            .flatten()
            .map(move |&i| {
                let e = self.edges[i];
                if e.a == surface {
                    e
                } else {
                    Edge {
                        a: e.b,
                        b: e.a,
                        from: e.to,
                        to: e.from,
                        ..e
                    }
                }
            })
    }

    /// A* over the edges an individual pet is allowed to use.
    /// `allowed` gates surfaces (a pack's `surfaces` list); `usable` gates edges
    /// (a class's traversal ruleset plus its `max_gap`).
    pub fn route(
        &self,
        start: usize,
        start_point: Point,
        goal: usize,
        goal_point: Point,
        allowed: &dyn Fn(usize) -> bool,
        usable: &dyn Fn(&Edge) -> bool,
    ) -> Option<Path> {
        if start == goal {
            return Some(Path {
                legs: vec![Leg {
                    surface: start,
                    enter: start_point,
                    exit: goal_point,
                    exit_kind: None,
                }],
            });
        }
        let heuristic = |p: Point| p.distance(goal_point);
        let mut best: HashMap<usize, f64> = HashMap::new();
        let mut came: HashMap<usize, (usize, Edge)> = HashMap::new();
        let mut arrival: HashMap<usize, Point> = HashMap::new();
        let mut open = BinaryHeap::new();
        best.insert(start, 0.);
        arrival.insert(start, start_point);
        open.push(Candidate {
            cost: Ranked(heuristic(start_point)),
            surface: start,
        });
        while let Some(Candidate { surface, .. }) = open.pop() {
            if surface == goal {
                return Some(self.rebuild(&came, start, goal, start_point, goal_point));
            }
            let here = arrival[&surface];
            let travelled = best[&surface];
            for edge in self.outgoing(surface) {
                if !allowed(edge.b) || !usable(&edge) {
                    continue;
                }
                // Cost is real distance: along this surface, then across the gap.
                let step = here.distance(edge.from) + edge.gap.max(1.);
                let next = travelled + step;
                if best.get(&edge.b).is_some_and(|&c| c <= next + 1e-6) {
                    continue;
                }
                best.insert(edge.b, next);
                came.insert(edge.b, (surface, edge));
                arrival.insert(edge.b, edge.to);
                open.push(Candidate {
                    cost: Ranked(next + heuristic(edge.to)),
                    surface: edge.b,
                });
            }
        }
        None
    }

    fn rebuild(
        &self,
        came: &HashMap<usize, (usize, Edge)>,
        start: usize,
        goal: usize,
        start_point: Point,
        goal_point: Point,
    ) -> Path {
        let mut chain = Vec::new();
        let mut node = goal;
        while node != start {
            let (previous, edge) = came[&node];
            chain.push(edge);
            node = previous;
        }
        chain.reverse();
        let mut legs = Vec::new();
        let mut enter = start_point;
        let mut surface = start;
        for edge in chain {
            legs.push(Leg {
                surface,
                enter,
                exit: edge.from,
                exit_kind: Some(edge.kind),
            });
            surface = edge.b;
            enter = edge.to;
        }
        legs.push(Leg {
            surface,
            enter,
            exit: goal_point,
            exit_kind: None,
        });
        Path { legs }
    }
}

/// One surface's worth of a route: travel from `enter` to `exit`, then leave by
/// `exit_kind` (`None` on the final leg — that is the destination).
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leg {
    pub surface: usize,
    pub enter: Point,
    pub exit: Point,
    pub exit_kind: Option<EdgeKind>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Path {
    pub legs: Vec<Leg>,
}
impl Path {
    pub fn length(&self) -> f64 {
        self.legs.iter().map(|l| l.enter.distance(l.exit)).sum()
    }
    pub fn destination(&self) -> Option<Leg> {
        self.legs.last().copied()
    }
}

struct Candidate {
    cost: Ranked,
    surface: usize,
}
impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}
impl Eq for Candidate {}
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // BinaryHeap is a max-heap; invert so the cheapest is popped first.
        other.cost.0.total_cmp(&self.cost.0)
    }
}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(PartialEq)]
struct Ranked(f64);

/// Closest pair of points on two segments, and the distance between them.
pub fn closest_points(a: Surface, b: Surface) -> (Point, Point, f64) {
    let (p1, p2) = (a.start, a.end);
    let (q1, q2) = (b.start, b.end);
    let d1 = Point::new(p2.x - p1.x, p2.y - p1.y);
    let d2 = Point::new(q2.x - q1.x, q2.y - q1.y);
    let r = Point::new(p1.x - q1.x, p1.y - q1.y);
    let a_len = d1.x * d1.x + d1.y * d1.y;
    let e_len = d2.x * d2.x + d2.y * d2.y;
    let f = d2.x * r.x + d2.y * r.y;
    let (mut s, mut t);
    if a_len <= f64::EPSILON && e_len <= f64::EPSILON {
        return (p1, q1, p1.distance(q1));
    }
    if a_len <= f64::EPSILON {
        s = 0.;
        t = (f / e_len).clamp(0., 1.);
    } else {
        let c = d1.x * r.x + d1.y * r.y;
        if e_len <= f64::EPSILON {
            t = 0.;
            s = (-c / a_len).clamp(0., 1.);
        } else {
            let b_dot = d1.x * d2.x + d1.y * d2.y;
            let denom = a_len * e_len - b_dot * b_dot;
            s = if denom.abs() > f64::EPSILON {
                ((b_dot * f - c * e_len) / denom).clamp(0., 1.)
            } else {
                0.
            };
            t = (b_dot * s + f) / e_len;
            if t < 0. {
                t = 0.;
                s = (-c / a_len).clamp(0., 1.);
            } else if t > 1. {
                t = 1.;
                s = ((b_dot - c) / a_len).clamp(0., 1.);
            }
        }
    }
    let pa = Point::new(p1.x + d1.x * s, p1.y + d1.y * s);
    let pb = Point::new(q1.x + d2.x * t, q1.y + d2.y * t);
    (pa, pb, pa.distance(pb))
}
