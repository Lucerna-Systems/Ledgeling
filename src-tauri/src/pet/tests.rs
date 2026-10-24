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
        self.view_with(&[])
    }
    fn view_with<'a>(&'a self, peers: &'a [Peer]) -> WorldView<'a> {
        WorldView {
            peers,
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
    assert!(
        rect.bottom() >= 400. && rect.y < 400.,
        "upright stands on the perch"
    );
    // The webview rotates about the feet, so a rotated pet is drawn beside
    // or below its perch, and the hit box has to be there too.
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    pet.rotation = 180.;
    let hanging = pet.sprite_rect(1.);
    assert!(
        hanging.y <= 400. && hanging.center().y > 400.,
        "hangs below"
    );
    pet.rotation = -90.;
    let left = pet.sprite_rect(1.);
    assert!(
        left.center().x < 500. && left.right() >= 500.,
        "clings to the left"
    );
    assert!(near(left.width + left.height, 96.));
    pet.rotation = 90.;
    assert!(pet.sprite_rect(1.).center().x > 500., "clings to the right");
}

#[test]
fn hit_testing_undoes_the_rotation_the_webview_draws() {
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 400.));
    let (w, h) = (48., 48.);
    let ay = pet.frame(1., 1.).anchor_y * h;
    // One pixel toward the head from the feet, in each orientation.
    for (rotation, point) in [
        (0., Point::new(500., 399.)),
        (180., Point::new(500., 401.)),
        (-90., Point::new(499., 400.)),
        (90., Point::new(501., 400.)),
    ] {
        pet.rotation = rotation;
        let local = pet.sprite_local(point, 1.);
        assert!(
            (local.x - w / 2.).abs() < 1e-6 && (local.y - (ay - 1.)).abs() < 1e-6,
            "rotation {rotation}: {local:?}"
        );
    }
}

#[test]
fn a_climber_on_a_window_side_faces_away_from_the_window() {
    let window = win(1, 400., 200., 600., 500.);
    let world = World::new(&[window]);
    let squirrel = pack("squirrel-red");
    for (kind, outside) in [
        (world::SurfaceKind::WindowLeft, -1.),
        (world::SurfaceKind::WindowRight, 1.),
        (world::SurfaceKind::WindowBottom, 1.),
    ] {
        let surface = world.surfaces.iter().find(|s| s.kind == kind).unwrap();
        let mut pet = Pet::new(1, squirrel.clone(), surface.point(0.5));
        pet.rotation = locomotion::sprite_rotation(pet.manifest(), surface);
        let rect = pet.sprite_rect(1.);
        let perch = surface.point(0.5);
        let offset = rect.center().x - perch.x + rect.center().y - perch.y;
        // Transparent rows below the feet may cross the edge; the body may not.
        assert!(
            offset * outside > 12.,
            "{} drew the pet into the window",
            kind.as_str()
        );
    }
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

// ------------------------------------------------------- throwing and petting

fn tick_with_cursor(pet: &mut Pet, world: &World, cursor: Option<Point>, events: &mut Vec<Event>) {
    let mut view = world.view();
    view.cursor = cursor;
    pet.tick(1. / 60., &view, Signals::default(), events);
}

/// Drag the held pet from `start` at `speed` logical px/s along `direction`
/// for `ticks` frames, then let go.
fn fling(pet: &mut Pet, world: &World, start: Point, direction: (f64, f64), speed: f64) {
    pet.grab();
    let mut events = Vec::new();
    let mut at = start;
    for _ in 0..8 {
        at = Point::new(
            at.x + direction.0 * speed / 60.,
            at.y + direction.1 * speed / 60.,
        );
        tick_with_cursor(pet, world, Some(at), &mut events);
    }
    pet.release();
}

fn ground() -> World {
    World::new(&[win(1, 100., 700., 1400., 100.)])
}

/// Tick until the fall ends and stop on that tick, so the position is where
/// the pet landed.
fn land(pet: &mut Pet, world: &World) {
    let mut events = Vec::new();
    for _ in 0..600 {
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
        if pet.motion != Motion::Falling {
            return;
        }
    }
    panic!("never landed");
}

fn collect(pet: &mut Pet, world: &World, seconds: f64) -> Vec<Event> {
    let mut events = Vec::new();
    let mut elapsed = 0.;
    while elapsed < seconds {
        elapsed += 1. / 60.;
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
    }
    events
}

#[test]
fn a_pet_thrown_to_the_right_lands_to_the_right() {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 300.));
    fling(&mut pet, &world, Point::new(500., 300.), (1., 0.), 900.);
    let released_at = pet.position;
    land(&mut pet, &world);
    assert!(
        pet.position.x > released_at.x + 100.,
        "it travelled with the throw: {:?} from {:?}",
        pet.position,
        released_at
    );
    assert!((pet.position.y - 700.).abs() < 2., "{:?}", pet.position);
}

#[test]
fn a_flung_pet_bounces_off_the_edges_and_never_leaves_the_world() {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(1500., 300.));
    fling(&mut pet, &world, Point::new(1500., 300.), (1., 0.), 3000.);
    let mut events = Vec::new();
    for _ in 0..600 {
        pet.tick(1. / 60., &world.view(), Signals::default(), &mut events);
        assert!(
            (0.0..=1600.).contains(&pet.position.x) && (0.0..=1000.).contains(&pet.position.y),
            "left the world at {:?}",
            pet.position
        );
    }
}

#[test]
fn throw_speed_is_clamped() {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 300.));
    fling(&mut pet, &world, Point::new(500., 300.), (1., -1.), 90_000.);
    match pet.arc {
        Arc_::Fall { vx, vy } => {
            assert!(vx.abs() <= MAX_THROW + 0.001, "vx {vx}");
            assert!(vy.abs() <= MAX_THROW + 0.001, "vy {vy}");
        }
        _ => panic!("a released pet falls"),
    }
}

#[test]
fn a_slow_release_is_released_and_a_fast_one_is_thrown() {
    let world = ground();
    let mut slow = Pet::new(1, pack("squirrel-red"), Point::new(500., 300.));
    fling(&mut slow, &world, Point::new(500., 300.), (1., 0.), 100.);
    assert!(slow.pending.contains(&Event::Released));
    assert!(!slow.pending.contains(&Event::Thrown));
    let mut fast = Pet::new(2, pack("squirrel-red"), Point::new(500., 300.));
    fling(&mut fast, &world, Point::new(500., 300.), (1., 0.), 1500.);
    assert!(fast.pending.contains(&Event::Thrown));
    assert!(!fast.pending.contains(&Event::Released));
}

#[test]
fn a_pack_without_a_thrown_reaction_still_gets_released() {
    let world = ground();
    let mut old = (*pack("squirrel-red")).clone();
    old.manifest.reactions.retain(|r| r.on != Event::Thrown);
    let mut pet = Pet::new(1, Arc::new(old), Point::new(500., 300.));
    fling(&mut pet, &world, Point::new(500., 300.), (1., 0.), 1500.);
    assert!(pet.pending.contains(&Event::Released));
    assert!(!pet.pending.contains(&Event::Thrown));
}

#[test]
fn releasing_the_pet_directly_emits_released() {
    // The app's pointer path calls `release` itself, not the engine's tick.
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 300.));
    pet.grab();
    pet.release();
    assert_eq!(pet.pending, vec![Event::Released]);
}

#[test]
fn a_hard_landing_after_a_throw_emits_landed_hard_and_a_gentle_drop_does_not() {
    let world = ground();
    let mut hard = Pet::new(1, pack("squirrel-red"), Point::new(500., 100.));
    fling(&mut hard, &world, Point::new(500., 100.), (0., 1.), 1500.);
    land(&mut hard, &world);
    collect(&mut hard, &world, 0.1);
    assert_eq!(hard.state(), "dizzy", "a hard landing reaches the pack");

    let mut gentle = Pet::new(2, pack("squirrel-red"), Point::new(500., 660.));
    gentle.grab();
    gentle.release();
    land(&mut gentle, &world);
    collect(&mut gentle, &world, 0.1);
    assert_ne!(gentle.state(), "dizzy");
    assert!(gentle.perch.is_some());
}

#[test]
fn a_fall_the_user_did_not_cause_is_not_a_hard_landing() {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(500., 0.));
    land(&mut pet, &world);
    collect(&mut pet, &world, 0.1);
    assert!(pet.perch.is_some());
    assert_ne!(pet.state(), "dizzy");
}

#[test]
fn a_thrown_pet_lands_on_a_surface_crossed_diagonally() {
    // From just left of a window's edge, a fast diagonal throw crosses its
    // top edge only partway along the step.
    let world = World::new(&[win(1, 400., 500., 300., 200.)]);
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(395., 490.));
    pet.arc = Arc_::Fall { vx: 600., vy: 600. };
    pet.motion = Motion::Falling;
    land(&mut pet, &world);
    assert!(pet.perch.is_some(), "it landed on the window top");
    assert!((pet.position.y - 500.).abs() < 1.);
}

fn rub(pet: &mut Pet, world: &World, xs: &[f64], y: f64, dt: f64) -> Vec<Event> {
    let mut events = Vec::new();
    for &x in xs {
        let mut view = world.view();
        view.cursor = Some(Point::new(x, y));
        pet.tick(dt, &view, Signals::default(), &mut events);
    }
    events
}

/// How many pettings a rub caused, read from mood: each one adds 0.08 and the
/// behavior machine consumes the event itself.
fn rub_count(pet: &mut Pet, world: &World, xs: &[f64], y: f64, dt: f64) -> usize {
    let before = pet.mood;
    rub(pet, world, xs, y, dt);
    ((pet.mood - before) / 0.08).round() as usize
}

fn settled_pet() -> (Pet, World) {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(600., 700.));
    settle(&mut pet, &world, 20.);
    // Pinned in place, as when its bubble is open, so the pointer stays on it.
    pet.listen(true);
    (pet, world)
}

const ZIG: [f64; 5] = [-10., 10., -10., 10., -10.];

fn zig(pet: &Pet) -> Vec<f64> {
    ZIG.iter().map(|dx| pet.position.x + dx).collect()
}

#[test]
fn rubbing_back_and_forth_over_the_pet_pets_it_once() {
    let (mut pet, world) = settled_pet();
    pet.mood = 0.5;
    let (xs, y) = (zig(&pet), pet.position.y - 20.);
    assert_eq!(rub_count(&mut pet, &world, &xs, y, 0.1), 1);
    assert_eq!(pet.state(), "enjoy", "the pack's reaction ran");
}

#[test]
fn a_single_sweep_across_the_pet_is_not_petting() {
    let (mut pet, world) = settled_pet();
    pet.mood = 0.5;
    let x = pet.position.x;
    let y = pet.position.y - 20.;
    let sweep: Vec<f64> = (0..8).map(|i| x - 20. + i as f64 * 6.).collect();
    assert_eq!(rub_count(&mut pet, &world, &sweep, y, 0.05), 0);
}

#[test]
fn rubbing_beside_the_pet_is_not_petting() {
    let (mut pet, world) = settled_pet();
    pet.mood = 0.5;
    let xs: Vec<f64> = ZIG.iter().map(|dx| pet.position.x + 200. + dx).collect();
    let y = pet.position.y - 20.;
    assert_eq!(rub_count(&mut pet, &world, &xs, y, 0.1), 0);
}

#[test]
fn a_second_rub_inside_the_cooldown_does_nothing() {
    let (mut pet, world) = settled_pet();
    pet.mood = 0.5;
    let (xs, y) = (zig(&pet), pet.position.y - 20.);
    assert_eq!(rub_count(&mut pet, &world, &xs, y, 0.05), 1);
    assert_eq!(rub_count(&mut pet, &world, &xs, y, 0.05), 0, "inside 2 s");
    collect(&mut pet, &world, 3.);
    pet.mood = 0.5;
    assert_eq!(
        rub_count(&mut pet, &world, &xs, y, 0.05),
        1,
        "after the cooldown"
    );
}

// ------------------------------------------------------------------ other pets

fn tick_with_peers(pet: &mut Pet, world: &World, peers: &[Peer]) -> Signals {
    let mut events = Vec::new();
    pet.tick(
        1. / 60.,
        &world.view_with(peers),
        Signals::default(),
        &mut events,
    )
}

fn peer_at(id: u64, x: f64, y: f64) -> Peer {
    Peer {
        id,
        position: Point::new(x, y),
        perch: None,
    }
}

/// Run `ticks` frames with the peer at `x`, counting how often the pet newly
/// notices it (the flag flips from not-near to near exactly when the event fires).
fn notices(pet: &mut Pet, world: &World, own: Peer, other_x: f64, ticks: usize) -> usize {
    let mut fired = 0;
    for _ in 0..ticks {
        let before = pet.was_near_pet;
        let peers = [own, peer_at(2, other_x, own.position.y)];
        tick_with_peers(pet, world, &peers);
        if !before && pet.was_near_pet {
            fired += 1;
        }
    }
    fired
}

fn pinned_pet() -> (Pet, World, Peer) {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(600., 700.));
    settle(&mut pet, &world, 20.);
    pet.listen(true);
    let own = peer_at(1, pet.position.x, pet.position.y);
    (pet, world, own)
}

#[test]
fn a_pet_alone_has_infinite_pet_distance_and_notices_no_one() {
    let (mut pet, world, own) = pinned_pet();
    let signals = tick_with_peers(&mut pet, &world, &[own]);
    assert!(signals.pet_distance.is_infinite(), "its own id is ignored");
    let signals = tick_with_peers(&mut pet, &world, &[]);
    assert!(signals.pet_distance.is_infinite());
    assert!(!pet.was_near_pet);
}

#[test]
fn pet_distance_is_logical_pixels_to_the_nearest_other_pet() {
    let (mut pet, world, own) = pinned_pet();
    let peers = [
        own,
        peer_at(2, own.position.x + 300., own.position.y),
        peer_at(3, own.position.x - 500., own.position.y),
    ];
    let signals = tick_with_peers(&mut pet, &world, &peers);
    assert!(
        (signals.pet_distance - 300.).abs() < 1e-6,
        "{}",
        signals.pet_distance
    );
}

#[test]
fn two_pets_within_140_notice_once_and_again_only_after_parting_past_200() {
    let (mut pet, world, own) = pinned_pet();
    let x = own.position.x;
    assert_eq!(
        notices(&mut pet, &world, own, x + 100., 30),
        1,
        "close: one event"
    );
    assert_eq!(
        notices(&mut pet, &world, own, x + 100., 120),
        0,
        "staying close: no repeats"
    );
    assert_eq!(
        notices(&mut pet, &world, own, x + 170., 30),
        0,
        "inside the 200 px hysteresis band"
    );
    assert_eq!(notices(&mut pet, &world, own, x + 250., 30), 0, "parted");
    assert_eq!(
        notices(&mut pet, &world, own, x + 100., 5),
        1,
        "back again: fires again"
    );
}

#[test]
fn noticing_another_pet_reaches_the_pack_as_a_greeting() {
    let (mut pet, world, own) = pinned_pet();
    notices(&mut pet, &world, own, own.position.x + 100., 5);
    assert_eq!(pet.state(), "greet");
}

fn peer_on(world: &World, owner: u64, t: f64) -> Peer {
    let surface = world
        .surfaces
        .iter()
        .find(|s| s.owner == owner && s.kind == crate::world::SurfaceKind::WindowTop)
        .expect("a window top");
    Peer {
        id: 2,
        position: surface.point(t),
        perch: Some(Perch {
            key: surface.key(),
            t,
        }),
    }
}

#[test]
fn move_toward_pet_brings_a_walker_beside_its_peer_on_the_same_window() {
    let world = World::new(&[win(1, 100., 600., 1000., 200.)]);
    let mut pet = Pet::new(1, pack("cat-tabby"), Point::new(150., 600.));
    settle(&mut pet, &world, 20.);
    pet.listen(false);
    let peers = [peer_on(&world, 1, 0.9)];
    pet.start_action(Action::MoveTowardPet, &world.view_with(&peers));
    assert_eq!(pet.motion, Motion::Walking);
    let mut events = Vec::new();
    for _ in 0..(30 * 60) {
        pet.tick(
            1. / 60.,
            &world.view_with(&peers),
            Signals::default(),
            &mut events,
        );
        if pet.motion != Motion::Walking {
            break;
        }
    }
    let gap = pet.position.distance(peers[0].position);
    assert!(gap < 120., "ended {gap} px from its peer");
    assert!(gap > 20., "beside it, not on top of it");
}

#[test]
fn move_toward_pet_does_not_hop_a_gap_wider_than_the_pack_allows_and_still_finishes() {
    // The cat hops 140 px; its peer is 300 px away across empty air, and the
    // windows are far above the floor, so there is no way round.
    let world = World::new(&[
        win(1, 100., 300., 400., 200.),
        win(2, 800., 300., 400., 200.),
    ]);
    let mut pet = Pet::new(1, pack("cat-tabby"), Point::new(300., 300.));
    settle(&mut pet, &world, 20.);
    pet.listen(false);
    let peers = [peer_on(&world, 2, 0.5)];
    pet.action_done = false;
    pet.start_action(Action::MoveTowardPet, &world.view_with(&peers));
    assert!(
        pet.action_done,
        "the action completes at once instead of hanging"
    );
    assert_ne!(pet.motion, Motion::Jumping);
    assert_ne!(pet.motion, Motion::Walking);
}

#[test]
fn move_toward_pet_with_no_peer_finishes_at_once() {
    let world = ground();
    let mut pet = Pet::new(1, pack("squirrel-red"), Point::new(600., 700.));
    settle(&mut pet, &world, 20.);
    pet.listen(false);
    pet.action_done = false;
    pet.start_action(Action::MoveTowardPet, &world.view());
    assert!(pet.action_done);
    assert_ne!(pet.motion, Motion::Walking);
}
