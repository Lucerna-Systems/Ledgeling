//! Locomotion, pathfinding and recovery. None of this needs a GUI: the world
//! is built from synthetic window layouts and stepped by hand.
use super::*;
use crate::{
    packs::{loader, manifest, Pack},
    world::{self, MonitorInfo, WindowInfo},
};
use std::path::Path;

fn monitors() -> Vec<MonitorInfo> {
    vec![MonitorInfo {
        id: 0,
        bounds: Rect::new(0., 0., 1600., 1000.),
        work: Rect::new(0., 0., 1600., 960.),
        scale: 1.,
        is_primary: true,
    }]
}

fn win(id: u64, x: f64, y: f64, w: f64, h: f64) -> WindowInfo {
    WindowInfo::new(id, Rect::new(x, y, w, h))
}

fn pack(name: &str) -> Arc<Pack> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../packs")
        .join(name);
    Arc::new(
        loader::load(&path, true).unwrap_or_else(|report| panic!("{name}:\n{}", report.text())),
    )
}

struct World {
    surfaces: Vec<Surface>,
    graph: SurfaceGraph,
}
impl World {
    fn new(windows: &[WindowInfo]) -> Self {
        let surfaces = world::surfaces(windows, &monitors(), 48.);
        let graph = SurfaceGraph::build(&surfaces);
        World { surfaces, graph }
    }
    fn view(&self) -> WorldView<'_> {
        WorldView {
            surfaces: &self.surfaces,
            graph: &self.graph,
            bounds: Rect::new(0., 0., 1600., 1000.),
            cursor: None,
            scale: 1.,
            speed_scale: 1.,
            attention: None,
        }
    }
}

fn step(pet: &mut Pet, world: &World, seconds: f64) {
    let mut events = Vec::new();
    let mut elapsed = 0.;
    while elapsed < seconds {
        elapsed += 1. / 60.;
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
    }
}

fn settle(pet: &mut Pet, world: &World, seconds: f64) {
    let mut events = Vec::new();
    let mut elapsed = 0.;
    while elapsed < seconds {
        elapsed += 1. / 60.;
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
        if pet.motion == Motion::Settled && pet.perch.is_some() {
            return;
        }
    }
}

use crate::packs::Signals;

// ------------------------------------------------------------------- riding

#[test]
fn a_pet_rides_the_window_it_is_standing_on() {
    let before = World::new(&[win(1, 200., 400., 600., 300.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(500., 400.));
    settle(&mut pet, &before, 20.);
    let perch = pet.perch.expect("landed somewhere");
    let offset = pet.position.x - 200.;
    let after = World::new(&[win(1, 500., 250., 600., 300.)]);
    step(&mut pet, &after, 0.05);
    assert_eq!(pet.perch.map(|p| p.key), Some(perch.key), "same edge");
    assert!(
        (pet.position.x - (500. + offset)).abs() < 1.,
        "moved with the window"
    );
    assert!((pet.position.y - 250.).abs() < 1.);
}

#[test]
fn a_pet_never_ends_a_tick_floating_off_the_desktop() {
    let world = World::new(&[
        win(1, 100., 300., 400., 200.),
        win(2, 900., 500., 400., 200.),
    ]);
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(300., 100.));
    let mut events = Vec::new();
    for _ in 0..4000 {
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
        assert!(pet.position.is_finite(), "position is never NaN");
        assert!(pet.position.x >= 0. && pet.position.x <= 1600.);
        assert!(pet.position.y >= 0. && pet.position.y <= 1000.);
    }
}

// ---------------------------------------------------------------- recovery

#[test]
fn a_flier_takes_off_when_its_window_closes() {
    let before = World::new(&[win(1, 200., 400., 600., 300.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(500., 400.));
    settle(&mut pet, &before, 20.);
    assert!(pet.perch.is_some());
    let empty = World::new(&[]);
    step(&mut pet, &empty, 0.05);
    assert_eq!(
        pet.motion,
        Motion::Flying,
        "a flier leaves under its own power"
    );
    settle(&mut pet, &empty, 30.);
    assert!(pet.perch.is_some(), "and lands on the desktop edge");
}

#[test]
fn a_climber_falls_when_its_window_closes_and_lands_on_what_is_below() {
    let before = World::new(&[
        win(1, 200., 300., 600., 100.),
        win(2, 100., 700., 800., 100.),
    ]);
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 300.));
    settle(&mut pet, &before, 30.);
    // Put it on the high window's top edge specifically.
    let top = before
        .surfaces
        .iter()
        .find(|s| s.owner == 1 && s.kind == crate::world::SurfaceKind::WindowTop)
        .unwrap();
    pet.perch = Some(pathfind::Perch {
        key: top.key(),
        t: top.midpoint_t(),
    });
    pet.position = top.point(top.midpoint_t());
    pet.motion = Motion::Settled;

    let after = World::new(&[win(2, 100., 700., 800., 100.)]);
    step(&mut pet, &after, 0.05);
    assert_eq!(pet.motion, Motion::Falling, "a climber has no wings");
    settle(&mut pet, &after, 5.);
    assert_eq!(pet.motion, Motion::Settled);
    assert!(
        (pet.position.y - 700.).abs() < 1.,
        "it lands on the first surface below, not the nearest one: {:?}",
        pet.position
    );
}

#[test]
fn a_falling_pet_with_nothing_below_still_lands_on_the_desktop_floor() {
    let world = World::new(&[]);
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(800., 100.));
    settle(&mut pet, &world, 10.);
    assert_eq!(pet.motion, Motion::Settled);
    assert!(
        pet.position.y <= 960.5,
        "the floor is the work area, not the void"
    );
}

#[test]
fn a_released_flier_falls_instead_of_hovering() {
    let world = World::new(&[win(1, 200., 500., 600., 100.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(500., 500.));
    settle(&mut pet, &world, 20.);
    assert!(pet.perch.is_some());
    pet.grab();
    pet.position = Point::new(500., 300.);
    pet.release();
    assert_eq!(
        pet.motion,
        Motion::Falling,
        "a dropped bird does not freeze"
    );
    settle(&mut pet, &world, 10.);
    assert_eq!(pet.motion, Motion::Settled);
    assert!(
        (pet.position.y - 500.).abs() < 1.,
        "it lands on the surface below where it was dropped: {:?}",
        pet.position
    );
}

#[test]
fn a_perch_that_shrinks_under_the_pet_makes_it_move() {
    let before = World::new(&[win(1, 200., 400., 600., 300.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(500., 400.));
    settle(&mut pet, &before, 20.);
    let at = pet.position;
    // Cover everything except a sliver, so the old perch no longer fits.
    let after = World::new(&[
        win(2, at.x - 300., 200., 620., 400.),
        win(1, 200., 400., 600., 300.),
    ]);
    step(&mut pet, &after, 0.2);
    assert_ne!(pet.motion, Motion::Settled, "it does not stand on nothing");
}

// -------------------------------------------------------------- pathfinding

#[test]
fn a_climber_cannot_cross_a_gap_wider_than_its_max_gap() {
    // Two windows 500 px apart, far above the floor: the only route is a jump
    // the squirrel cannot make (max_gap 180).
    let world = World::new(&[
        win(1, 100., 300., 300., 120.),
        win(2, 900., 300., 300., 120.),
    ]);
    let squirrel = pack("squirrel-red");
    let left = world
        .surfaces
        .iter()
        .position(|s| s.owner == 1 && s.kind == crate::world::SurfaceKind::WindowTop)
        .unwrap();
    let right = world
        .surfaces
        .iter()
        .position(|s| s.owner == 2 && s.kind == crate::world::SurfaceKind::WindowTop)
        .unwrap();
    let direct = world
        .graph
        .edges
        .iter()
        .find(|e| (e.a == left && e.b == right) || (e.a == right && e.b == left));
    assert!(
        direct.is_none_or(|e| e.gap > squirrel.manifest.locomotion.jump.max_gap),
        "the gap is genuinely too wide"
    );
    let route = pathfind::route(
        &world.surfaces,
        &world.graph,
        &squirrel.manifest,
        left,
        world.surfaces[left].midpoint(),
        right,
        world.surfaces[right].midpoint(),
        1.,
    );
    // A route may exist the long way round via the monitor edges, but it can
    // never contain a single hop wider than max_gap.
    if let Some(plan) = route {
        for pair in plan.steps.windows(2) {
            let [a, b] = pair else { continue };
            if a.exit == Some(crate::world::EdgeKind::Jumpable) {
                let from = pathfind::find_key(&world.surfaces, a.key).unwrap();
                let to = pathfind::find_key(&world.surfaces, b.key).unwrap();
                let (_, _, gap) =
                    crate::world::graph::closest_points(world.surfaces[from], world.surfaces[to]);
                assert!(
                    gap <= squirrel.manifest.locomotion.jump.max_gap + 1.,
                    "no leg may exceed max_gap, found {gap}"
                );
            }
        }
    }
}

#[test]
fn a_flier_reaches_what_a_climber_cannot() {
    let world = World::new(&[
        win(1, 100., 300., 300., 120.),
        win(2, 1100., 200., 300., 120.),
    ]);
    let mut budgie = Pet::new(1, pack("budgie-classic"), Point::new(250., 300.));
    settle(&mut budgie, &world, 20.);
    let first = budgie.perch.map(|p| p.key);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..80 {
        step(&mut budgie, &world, 1.);
        if let Some(perch) = budgie.perch {
            seen.insert(perch.key.owner);
        }
    }
    assert!(first.is_some());
    assert!(
        seen.len() > 1,
        "a flier ignores the graph and gets everywhere: {seen:?}"
    );
}

#[test]
fn a_walker_only_uses_horizontal_surfaces() {
    let world = World::new(&[win(1, 300., 400., 500., 300.)]);
    let mut manifest = pack("squirrel-red").manifest.clone();
    manifest.locomotion = manifest::Locomotion::defaults(manifest::LocomotionClass::Walker);
    for surface in &world.surfaces {
        let usable = pathfind::allowed(&manifest, surface, 1.);
        if surface.orientation() == crate::world::Orientation::Vertical {
            assert!(
                !usable,
                "a walker cannot stand on {}",
                surface.kind.as_str()
            );
        }
    }
}

#[test]
fn a_gecko_only_ever_lives_on_window_sides() {
    let world = World::new(&[win(1, 300., 300., 500., 400.)]);
    let gecko = pack("gecko-glass");
    let mut pet = Pet::new(1, gecko.clone(), Point::new(400., 300.));
    let mut events = Vec::new();
    for _ in 0..6000 {
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
        if let Some(perch) = pet.perch {
            assert!(
                matches!(
                    perch.key.kind,
                    crate::world::SurfaceKind::WindowLeft | crate::world::SurfaceKind::WindowRight
                ),
                "the gecko settled on {}",
                perch.key.kind.as_str()
            );
        }
    }
}

// ------------------------------------------------------------------- sprite

#[test]
fn the_sprite_rectangle_follows_the_anchor_and_rotation() {
    let squirrel = pack("squirrel-red");
    let mut pet = Pet::new(1, squirrel, Point::new(500., 400.));
    let rect = pet.sprite_rect(1.);
    assert_eq!(rect.width, 48.);
    assert_eq!(rect.center().x, 500.);
    pet.rotation = 90.;
    let rotated = pet.sprite_rect(1.);
    assert_eq!(rotated.width, 48.);
    assert_eq!(rotated.height, 48.);
}

#[test]
fn transparent_padding_at_the_bottom_of_a_frame_does_not_lift_the_pet() {
    // The bundled strips carry transparent rows below the clip's ground
    // line; anchoring to the texture edge would leave the pet floating
    // above the surface it is standing on.
    let pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 400.));
    let frame = pet.frame(1., 1.);
    let strip = pet.pack.strip(pet.clip()).expect("the clip has a strip");
    assert!(
        strip.ground < strip.height,
        "the fixture actually has bottom padding"
    );
    let top = frame.y - frame.height * frame.anchor_y;
    let ground = top + frame.height * (strip.ground as f64 / strip.height as f64);
    assert_eq!(ground, 400., "the clip's contact frame sits on the surface");
    assert!(frame.anchor_y < 1., "the anchor skipped the padding");
}

#[test]
fn airborne_frames_of_a_clip_stay_airborne() {
    // The budgie's `happy` clip hops: its ground line comes from the contact
    // frames (bottom 44), and the apex frames (bottom 38) must stay lifted
    // rather than being glued down to the surface.
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(500., 400.));
    pet.machine.clip = "happy".into();
    pet.clip_started = 0.;
    let frame = pet.frame(1., 1.);
    let strip = pet.pack.strip("happy").unwrap();
    let apex = strip.masks.iter().map(|m| m.bottom).min().unwrap();
    let top = frame.y - frame.height * frame.anchor_y;
    let lowest_at_apex = top + frame.height * (apex as f64 / strip.height as f64);
    assert!(
        lowest_at_apex < 400.,
        "a hop frame is above the surface, not sunk into it"
    );
}

#[test]
fn animation_frames_advance_loop_and_hold() {
    let budgie = pack("budgie-classic");
    let mut pet = Pet::new(1, budgie, Point::new(500., 400.));
    pet.machine.clip = "idle".into();
    pet.clock = 0.;
    assert_eq!(pet.frame_index(), 0);
    pet.clock = 0.5;
    assert_eq!(pet.frame_index(), 3, "idle runs at 6 fps across 4 frames");
    pet.clock = 10.;
    assert!(pet.frame_index() < 4, "looping clips wrap");
    pet.machine.clip = "land".into();
    pet.clock = 100.;
    assert_eq!(
        pet.frame_index(),
        2,
        "a non-looping clip holds its last frame"
    );
}

// ------------------------------------------------------- attending the user

#[test]
fn a_pet_the_user_is_talking_to_stays_where_it_is() {
    let world = World::new(&[
        win(1, 100., 300., 600., 200.),
        win(2, 900., 300., 600., 200.),
    ]);
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(300., 100.));
    settle(&mut pet, &world, 30.);
    pet.listen(true);
    let held = pet.position;
    // Long enough for several behavior states, any of which would normally
    // send it walking to another window.
    step(&mut pet, &world, 120.);
    assert_eq!(pet.attend(), Some(Attend::Listening));
    assert_eq!(pet.motion, Motion::Settled);
    assert!(
        (pet.position.x - held.x).abs() < 1. && (pet.position.y - held.y).abs() < 1.,
        "it stayed under the bubble: {:?} → {:?}",
        held,
        pet.position
    );
    pet.listen(false);
    step(&mut pet, &world, 120.);
    assert_eq!(pet.attend(), None, "closing the bubble frees it");
    assert!(
        (pet.position.x - held.x).abs() > 1. || pet.motion != Motion::Settled,
        "and it wanders again"
    );
}

#[test]
fn a_walking_pet_stops_when_its_bubble_opens() {
    let world = World::new(&[
        win(1, 100., 300., 600., 200.),
        win(2, 200., 700., 600., 200.),
    ]);
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(300., 100.));
    settle(&mut pet, &world, 30.);
    let mut events = Vec::new();
    for _ in 0..4000 {
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
        if pet.motion == Motion::Walking {
            break;
        }
    }
    assert_eq!(pet.motion, Motion::Walking, "it set off somewhere");
    pet.listen(true);
    assert_eq!(pet.motion, Motion::Settled, "and stopped on the spot");
    assert!(pet.plan.is_none());
    assert!(pet.perch.is_some(), "still standing on its surface");
}

#[test]
fn a_working_pet_plays_its_assistant_clip_and_falls_back_when_it_has_none() {
    let world = World::new(&[win(1, 100., 300., 600., 200.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(300., 100.));
    settle(&mut pet, &world, 30.);
    // The bundled packs ship no busy or attention clips, so both resolve down
    // the fallback chain rather than failing to draw.
    pet.work(Some(Attend::Busy));
    assert_eq!(pet.clip(), "preen", "busy → preen → idle");
    pet.work(Some(Attend::NeedsUser));
    assert_eq!(pet.clip(), "alert", "attention → alert → happy → idle");
    let mut gecko = Pet::new(2, pack("gecko-glass"), Point::new(300., 100.));
    settle(&mut gecko, &world, 30.);
    gecko.work(Some(Attend::Busy));
    assert_eq!(gecko.clip(), "idle", "a two-animation pack still draws");
}

#[test]
fn the_assistant_clip_never_replaces_a_locomotion_clip_in_the_air() {
    let world = World::new(&[win(1, 100., 300., 600., 200.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(300., 100.));
    pet.work(Some(Attend::Busy));
    assert_eq!(pet.motion, Motion::Falling);
    assert_eq!(pet.clip(), pet.machine.clip, "physics keeps its own clip");
    settle(&mut pet, &world, 30.);
    assert_eq!(
        pet.clip(),
        "preen",
        "and the busy clip takes over on landing"
    );
}

#[test]
fn a_task_outranks_the_bubble_and_survives_it_closing() {
    let world = World::new(&[win(1, 100., 300., 600., 200.)]);
    let mut pet = Pet::new(1, pack("budgie-classic"), Point::new(300., 100.));
    settle(&mut pet, &world, 30.);
    pet.listen(true);
    pet.work(Some(Attend::NeedsUser));
    assert_eq!(pet.attend(), Some(Attend::NeedsUser));
    pet.listen(false);
    assert_eq!(
        pet.attend(),
        Some(Attend::NeedsUser),
        "a task waiting on the user keeps it here"
    );
    let held = pet.position;
    step(&mut pet, &world, 120.);
    assert!((pet.position.x - held.x).abs() < 1.);
    pet.work(None);
    step(&mut pet, &world, 120.);
    assert_eq!(pet.attend(), None);
}
