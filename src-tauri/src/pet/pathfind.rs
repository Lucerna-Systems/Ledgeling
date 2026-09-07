//! Turning "go somewhere" into a route the pet's class is actually allowed to
//! walk. Routes are stored parametrically — `(surface key, t)` rather than
//! world points — so a route stays correct while the windows under it move.
use crate::{
    packs::{
        manifest::{Jump, Locomotion},
        LocomotionClass, Manifest,
    },
    world::{graph::Edge, EdgeKind, Orientation, Point, Surface, SurfaceGraph, SurfaceKey},
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Perch {
    pub key: SurfaceKey,
    pub t: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub key: SurfaceKey,
    pub from_t: f64,
    pub to_t: f64,
    /// How the pet leaves this surface; `None` on the final step.
    pub exit: Option<EdgeKind>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub steps: Vec<Step>,
    pub index: usize,
}
impl Plan {
    pub fn current(&self) -> Option<Step> {
        self.steps.get(self.index).copied()
    }
    pub fn next(&self) -> Option<Step> {
        self.steps.get(self.index + 1).copied()
    }
    pub fn finished(&self) -> bool {
        self.index >= self.steps.len()
    }
    pub fn destination(&self) -> Option<Perch> {
        self.steps.last().map(|s| Perch {
            key: s.key,
            t: s.to_t,
        })
    }
}

/// The visible fragment of `key` that currently holds `t`, if any.
pub fn locate(surfaces: &[Surface], perch: Perch) -> Option<usize> {
    surfaces
        .iter()
        .position(|s| s.key() == perch.key && s.contains_t(perch.t))
}

pub fn find_key(surfaces: &[Surface], key: SurfaceKey) -> Option<usize> {
    surfaces.iter().position(|s| s.key() == key)
}

/// May this pet stand here at all? Combines the pack's surface list with the
/// class's own restrictions and its minimum usable segment length.
pub fn allowed(manifest: &Manifest, surface: &Surface, scale: f64) -> bool {
    if !manifest.allows(surface.kind) {
        return false;
    }
    if surface.length() < manifest.locomotion.min_surface_length * scale {
        return false;
    }
    match manifest.locomotion.class {
        LocomotionClass::Walker | LocomotionClass::Hopper | LocomotionClass::Flier => {
            surface.orientation() == Orientation::Horizontal
        }
        LocomotionClass::Climber => true,
    }
}

/// The traversal ruleset. This is the only thing that differs between classes.
pub fn traversable(locomotion: &Locomotion, edge: &Edge, scale: f64) -> bool {
    let jump = |jump: &Jump| jump.enabled && edge.gap <= jump.max_gap * scale;
    match locomotion.class {
        // Fliers never consult the graph; this exists only for completeness.
        LocomotionClass::Flier => true,
        LocomotionClass::Climber => edge.kind == EdgeKind::Contiguous || jump(&locomotion.jump),
        LocomotionClass::Walker => edge.kind == EdgeKind::Contiguous,
        LocomotionClass::Hopper => edge.kind == EdgeKind::Contiguous || jump(&locomotion.jump),
    }
}

/// A route from `from` to a point on surface `goal`, or `None` when this pet
/// cannot get there — a climber facing a gap wider than `max_gap` stays put.
#[allow(clippy::too_many_arguments)]
pub fn route(
    surfaces: &[Surface],
    graph: &SurfaceGraph,
    manifest: &Manifest,
    from: usize,
    from_point: Point,
    goal: usize,
    goal_point: Point,
    scale: f64,
) -> Option<Plan> {
    let path = graph.route(
        from,
        from_point,
        goal,
        goal_point,
        &|i| surfaces.get(i).is_some_and(|s| allowed(manifest, s, scale)),
        &|edge| traversable(&manifest.locomotion, edge, scale),
    )?;
    let steps = path
        .legs
        .iter()
        .filter_map(|leg| {
            let surface = surfaces.get(leg.surface)?;
            Some(Step {
                key: surface.key(),
                from_t: surface.t_of(leg.enter),
                to_t: surface.t_of(leg.exit),
                exit: leg.exit_kind,
            })
        })
        .collect::<Vec<_>>();
    if steps.len() != path.legs.len() {
        return None;
    }
    Some(Plan { steps, index: 0 })
}

/// A single-surface plan: used for patrolling and for a flier's landing.
pub fn hop(surface: &Surface, from_t: f64, to_t: f64) -> Plan {
    Plan {
        steps: vec![Step {
            key: surface.key(),
            from_t,
            to_t,
            exit: None,
        }],
        index: 0,
    }
}

/// Every surface this pet could settle on, cheapest lookup for the actions.
pub fn candidates<'s>(
    surfaces: &'s [Surface],
    manifest: &Manifest,
    scale: f64,
) -> Vec<(usize, &'s Surface)> {
    surfaces
        .iter()
        .enumerate()
        .filter(|(_, s)| allowed(manifest, s, scale))
        .collect()
}
