//! The pet runtime. Rust owns every truth here: where the pet is, what it is
//! doing, and what it will do next. The webview only draws what this produces.
pub mod fsm;
pub mod locomotion;
pub mod pathfind;

use crate::{
    packs::{
        manifest::Event, Action, LocomotionClass, Manifest, Pack, Signals, SurfaceOrientation,
    },
    world::{EdgeKind, Orientation, Point, Rect, Surface, SurfaceGraph, SurfaceKey},
};
use fsm::{Machine, Rng};
use locomotion::{air_offset, sprite_rotation};
use pathfind::{Perch, Plan};
use serde::Serialize;
use std::sync::Arc;

/// Gravity in physical pixels per second squared at `gravity: 1.0`.
const GRAVITY: f64 = 1400.;
const TERMINAL_VELOCITY: f64 = 2200.;
/// How close the cursor must be for `cursor_nearby`, in logical pixels.
const CURSOR_NEARBY: f64 = 120.;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Motion {
    Settled,
    Walking,
    Jumping,
    Flying,
    Falling,
    Held,
}

#[derive(Clone, Copy, Debug)]
enum Arc_ {
    None,
    Jump {
        from: Point,
        to: Point,
        elapsed: f64,
        duration: f64,
        height: f64,
    },
    Air {
        from: Point,
        target: Perch,
        elapsed: f64,
        duration: f64,
    },
    Fall {
        velocity: f64,
    },
}

/// What the world looks like to one pet on one tick.
pub struct WorldView<'a> {
    pub surfaces: &'a [Surface],
    pub graph: &'a SurfaceGraph,
    pub bounds: Rect,
    pub cursor: Option<Point>,
    /// Physical pixels per logical pixel, so pack distances mean what the
    /// author expected on any display.
    pub scale: f64,
    /// User-facing speed preference, applied on top of the pack's own speeds.
    pub speed_scale: f64,
    /// Window the user is working in. Only set while a nudge is escalating, so
    /// the pet drifts somewhere visible instead of somewhere random.
    pub attention: Option<u64>,
}

pub struct Pet {
    pub id: u64,
    pub pack: Arc<Pack>,
    pub position: Point,
    pub rotation: f64,
    pub flip_x: bool,
    pub motion: Motion,
    pub machine: Machine,
    pub perch: Option<Perch>,
    pub home: Option<Perch>,
    pub plan: Option<Plan>,
    pub energy: f64,
    pub mood: f64,
    pub clock: f64,
    pub rng: Rng,
    arc: Arc_,
    action_done: bool,
    was_near_cursor: bool,
    clip_started: f64,
    pending: Vec<Event>,
    /// Sound the pack asked for on the last state change, if any.
    pub sound_request: Option<String>,
}

impl Pet {
    pub fn new(id: u64, pack: Arc<Pack>, position: Point) -> Self {
        let mut rng = Rng::new(0xA51C_974F ^ (id.wrapping_mul(0x9E37_79B9)));
        let machine = Machine::new(&pack.manifest, &mut rng);
        Self {
            id,
            pack,
            position,
            rotation: 0.,
            flip_x: false,
            motion: Motion::Falling,
            machine,
            perch: None,
            home: None,
            plan: None,
            energy: 1.,
            mood: 0.7,
            clock: 0.,
            rng,
            arc: Arc_::Fall { velocity: 0. },
            action_done: false,
            was_near_cursor: false,
            clip_started: 0.,
            pending: Vec::new(),
            sound_request: None,
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.pack.manifest
    }
    pub fn clip(&self) -> &str {
        &self.machine.clip
    }
    pub fn state(&self) -> &str {
        &self.machine.state
    }

    /// Frame index for the current clip, honouring loop and pingpong.
    pub fn frame_index(&self) -> u32 {
        let Some(animation) = self.manifest().animations.get(self.clip()) else {
            return 0;
        };
        let elapsed = (self.clock - self.clip_started).max(0.);
        let step = (elapsed * animation.fps) as u32;
        let frames = animation.frames.max(1);
        if animation.pingpong && frames > 1 {
            let period = frames * 2 - 2;
            let at = step % period;
            if at < frames {
                at
            } else {
                period - at
            }
        } else if animation.looping {
            step % frames
        } else {
            step.min(frames - 1)
        }
    }

    // ------------------------------------------------------------ interaction

    pub fn grab(&mut self) {
        self.motion = Motion::Held;
        self.arc = Arc_::None;
        self.plan = None;
        self.perch = None;
    }

    pub fn release(&mut self) {
        if self.motion == Motion::Held {
            self.motion = Motion::Falling;
            self.arc = Arc_::Fall { velocity: 0. };
            self.mood = (self.mood + 0.05).min(1.);
        }
    }

    pub fn is_held(&self) -> bool {
        self.motion == Motion::Held
    }

    /// Placed onto a fresh world after the pet screen changed and its old
    /// position is no longer covered: everything bound to the old world —
    /// plan, home, arc — is dead weight, so it enters like a fresh spawn.
    pub fn rehome(&mut self, position: Point, perch: Option<Perch>) {
        self.position = position;
        self.rotation = 0.;
        self.flip_x = false;
        self.perch = perch;
        self.home = perch;
        self.plan = None;
        self.pending.clear();
        if self.motion == Motion::Held {
            return;
        }
        if perch.is_some() {
            self.motion = Motion::Settled;
            self.arc = Arc_::None;
        } else {
            self.motion = Motion::Falling;
            self.arc = Arc_::Fall { velocity: 0. };
        }
    }

    /// A window yanked across the desktop is alarming; the pack decides what
    /// that looks like through its `surface_moved_fast` reaction.
    pub fn startle(&mut self) {
        self.pending.push(Event::SurfaceMovedFast);
    }

    pub fn click(&mut self) {
        self.pending.push(Event::Clicked);
        self.mood = (self.mood + 0.03).min(1.);
    }

    /// Swap in a reloaded pack without losing where the pet is standing.
    pub fn adopt(&mut self, pack: Arc<Pack>, position: Point) {
        if Arc::ptr_eq(&self.pack, &pack) {
            return;
        }
        let mut rng = self.rng;
        self.machine = Machine::new(&pack.manifest, &mut rng);
        self.rng = rng;
        self.pack = pack;
        self.position = position;
        self.plan = None;
        self.arc = Arc_::None;
        self.motion = Motion::Settled;
        self.clip_started = self.clock;
    }

    /// Events raised by the runtime for this pet before the next tick.
    pub fn queue(&mut self, event: Event) {
        self.pending.push(event);
    }

    // ------------------------------------------------------------------ tick

    pub fn tick(
        &mut self,
        dt: f64,
        view: &WorldView<'_>,
        base: Signals,
        events: &mut Vec<Event>,
    ) -> Signals {
        let dt = dt.clamp(0., 0.1);
        self.clock += dt;
        events.append(&mut self.pending);
        let signals = self.signals(base, view);
        self.update_scalars(dt, &signals);
        self.detect_events(&signals, view, events);
        if self.motion == Motion::Held {
            if let Some(cursor) = view.cursor {
                self.position = cursor;
            }
            self.clamp(view);
            self.step_machine(dt, &signals, events, view);
            return signals;
        }
        self.ride(view, events);
        self.step_machine(dt, &signals, events, view);
        self.step_motion(dt, view);
        self.clamp(view);
        signals
    }

    fn signals(&self, base: Signals, view: &WorldView<'_>) -> Signals {
        Signals {
            energy: self.energy,
            mood: self.mood,
            surface_count: view.surfaces.len() as f64,
            cursor_distance: view
                .cursor
                .map(|c| c.distance(self.position) / view.scale)
                .unwrap_or(f64::INFINITY),
            is_dragging: self.motion == Motion::Held,
            ..base
        }
    }

    /// Energy and mood move on a timescale of minutes, so the pet has
    /// continuity rather than snapping between personalities.
    fn update_scalars(&mut self, dt: f64, signals: &Signals) {
        let hours = dt / 3600.;
        let personality = self.manifest().personality;
        let resting = matches!(self.motion, Motion::Settled)
            && self.machine.clip == self.manifest().resolve_clip("sleep");
        if resting || signals.idle_seconds > 240. {
            self.energy = (self.energy + 0.50 * hours).min(1.);
        } else {
            let effort = if self.motion == Motion::Settled {
                0.6
            } else {
                1.4
            };
            self.energy =
                (self.energy - 0.25 * hours * effort * (0.5 + personality.sleepiness)).max(0.);
        }
        let mut mood = self.mood;
        if signals.session_minutes > 75. {
            mood -= 0.10 * hours;
        }
        if signals.surface_count <= 1. {
            mood -= 0.05 * hours;
        } else {
            mood += 0.02 * hours;
        }
        self.mood = mood.clamp(0., 1.);
    }

    fn detect_events(&mut self, signals: &Signals, view: &WorldView<'_>, events: &mut Vec<Event>) {
        let near = signals.cursor_distance < CURSOR_NEARBY;
        if near && !self.was_near_cursor {
            events.push(Event::CursorNearby);
        }
        self.was_near_cursor = near;
        if self.energy < 0.15 {
            events.push(Event::LowEnergy);
        }
        let _ = view;
    }

    /// Hold `(surface, t)` as windows move, and notice when the perch is gone.
    fn ride(&mut self, view: &WorldView<'_>, events: &mut Vec<Event>) {
        let Some(perch) = self.perch else {
            return;
        };
        match pathfind::locate(view.surfaces, perch) {
            Some(index) => {
                let surface = &view.surfaces[index];
                let half = self.half_width(view.scale);
                if !surface.fits(perch.t, half) {
                    // The perch shrank under the pet: find somewhere that fits.
                    events.push(Event::SurfaceLost);
                    self.lose_surface(view);
                    return;
                }
                if self.motion == Motion::Settled || self.motion == Motion::Walking {
                    self.position = surface.point(perch.t);
                    self.rotation = sprite_rotation(self.manifest(), surface);
                }
            }
            None => {
                events.push(Event::SurfaceLost);
                self.lose_surface(view);
            }
        }
    }

    fn lose_surface(&mut self, view: &WorldView<'_>) {
        self.perch = None;
        self.plan = None;
        self.action_done = true;
        if self.manifest().locomotion.class == LocomotionClass::Flier {
            // A flier simply takes off toward somewhere else it is allowed.
            self.motion = Motion::Settled;
            if let Some(target) = self.nearest_perch(view, self.position) {
                self.begin_air(target, view);
            }
        } else {
            self.motion = Motion::Falling;
            self.arc = Arc_::Fall { velocity: 0. };
            self.rotation = 0.;
        }
    }

    fn step_machine(
        &mut self,
        dt: f64,
        signals: &Signals,
        events: &mut Vec<Event>,
        view: &WorldView<'_>,
    ) {
        let manifest = self.pack.manifest.clone();
        let done = self.action_done;
        let mut rng = self.rng;
        let entered = self
            .machine
            .tick(dt, &manifest, signals, events, done, &mut rng);
        self.rng = rng;
        self.action_done = false;
        if let Some(entered) = entered {
            self.clip_started = self.clock;
            self.sound_request = entered.sound;
            if let Some(action) = entered.action {
                self.start_action(action, view);
            }
        }
        events.clear();
    }

    fn start_action(&mut self, action: Action, view: &WorldView<'_>) {
        if self.motion == Motion::Held || self.motion == Motion::Falling {
            self.action_done = true;
            return;
        }
        let target = match action {
            Action::MoveToRandomSurface => self.random_perch(view),
            Action::MoveToNearestSurface => self.nearest_perch(view, self.position),
            Action::MoveTowardCursor => view
                .cursor
                .and_then(|c| self.nearest_perch(view, c))
                .or_else(|| self.random_perch(view)),
            Action::FleeFromCursor => view
                .cursor
                .and_then(|c| self.farthest_perch(view, c))
                .or_else(|| self.random_perch(view)),
            Action::PatrolCurrentSurface => self.patrol_target(view),
            Action::ReturnToHomeSurface => self.home.or_else(|| self.random_perch(view)),
        };
        match target {
            Some(target) => self.travel_to(target, view),
            None => self.action_done = true,
        }
    }

    fn travel_to(&mut self, target: Perch, view: &WorldView<'_>) {
        if self.manifest().locomotion.class == LocomotionClass::Flier {
            self.begin_air(target, view);
            return;
        }
        let (Some(perch), Some(goal)) = (self.perch, pathfind::locate(view.surfaces, target))
        else {
            self.action_done = true;
            return;
        };
        let Some(from) = pathfind::locate(view.surfaces, perch) else {
            self.action_done = true;
            return;
        };
        let plan = pathfind::route(
            view.surfaces,
            view.graph,
            self.manifest(),
            from,
            view.surfaces[from].point(perch.t),
            goal,
            view.surfaces[goal].point(target.t),
            view.scale,
        );
        match plan {
            // Unreachable is a legitimate answer: a climber facing too wide a
            // gap stays where it is rather than teleporting.
            None => self.action_done = true,
            Some(plan) => {
                self.plan = Some(plan);
                self.motion = Motion::Walking;
            }
        }
    }

    fn begin_air(&mut self, target: Perch, view: &WorldView<'_>) {
        let Some(index) = pathfind::locate(view.surfaces, target) else {
            self.action_done = true;
            return;
        };
        let destination = view.surfaces[index].point(target.t);
        let speed = self.manifest().locomotion.speed.air.max(20.) * view.scale * view.speed_scale;
        let duration = (self.position.distance(destination) / speed).clamp(0.4, 8.);
        self.flip_x = destination.x < self.position.x;
        self.arc = Arc_::Air {
            from: self.position,
            target,
            elapsed: 0.,
            duration,
        };
        self.motion = Motion::Flying;
        self.perch = None;
        self.plan = None;
    }

    fn step_motion(&mut self, dt: f64, view: &WorldView<'_>) {
        match self.motion {
            Motion::Settled | Motion::Held => {}
            Motion::Walking => self.step_walk(dt, view),
            Motion::Jumping => self.step_jump(dt, view),
            Motion::Flying => self.step_air(dt, view),
            Motion::Falling => self.step_fall(dt, view),
        }
    }

    fn step_walk(&mut self, dt: f64, view: &WorldView<'_>) {
        let (Some(mut plan), Some(mut perch)) = (self.plan.clone(), self.perch) else {
            self.settle();
            return;
        };
        let Some(step) = plan.current() else {
            self.plan = None;
            self.settle();
            return;
        };
        let Some(index) = pathfind::locate(view.surfaces, perch) else {
            return; // `ride` has already noticed and will recover.
        };
        let surface = &view.surfaces[index];
        let speed = locomotion::speed(self.manifest(), surface) * view.scale * view.speed_scale;
        let span = surface.span_length().max(1.);
        let travel = (speed * dt) / span;
        let direction = if step.to_t >= perch.t { 1. } else { -1. };
        self.flip_x = direction < 0.;
        let next_t = perch.t + travel * direction;
        let arrived = (next_t - step.to_t) * direction >= 0.;
        perch.t = if arrived { step.to_t } else { next_t };
        self.perch = Some(perch);
        self.position = surface.point(perch.t);
        self.rotation = sprite_rotation(self.manifest(), surface);
        if !arrived {
            self.plan = Some(plan);
            return;
        }
        match (step.exit, plan.next()) {
            (Some(EdgeKind::Contiguous), Some(next)) => {
                plan.index += 1;
                self.perch = Some(Perch {
                    key: next.key,
                    t: next.from_t,
                });
                self.plan = Some(plan);
            }
            (Some(EdgeKind::Jumpable), Some(next)) => {
                let landing = pathfind::find_key(view.surfaces, next.key)
                    .map(|i| view.surfaces[i].point(next.from_t));
                match landing {
                    Some(to) => {
                        let jump = self.manifest().locomotion.jump;
                        let speed = locomotion::speed(self.manifest(), surface).max(60.)
                            * view.scale
                            * view.speed_scale;
                        self.arc = Arc_::Jump {
                            from: self.position,
                            to,
                            elapsed: 0.,
                            duration: (self.position.distance(to) / speed).clamp(0.25, 1.6),
                            height: jump.arc_height * view.scale,
                        };
                        self.motion = Motion::Jumping;
                        self.flip_x = to.x < self.position.x;
                        plan.index += 1;
                        self.plan = Some(plan);
                        self.perch = None;
                    }
                    None => self.settle(),
                }
            }
            _ => {
                self.plan = None;
                self.settle();
            }
        }
    }

    fn step_jump(&mut self, dt: f64, view: &WorldView<'_>) {
        let Arc_::Jump {
            from,
            to,
            mut elapsed,
            duration,
            height,
        } = self.arc
        else {
            self.settle();
            return;
        };
        elapsed += dt;
        let t = (elapsed / duration).min(1.);
        self.position = Point::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t - (std::f64::consts::PI * t).sin() * height,
        );
        self.rotation = 0.;
        if t >= 1. {
            self.arc = Arc_::None;
            let landed = self
                .plan
                .as_ref()
                .and_then(|p| p.current())
                .map(|step| Perch {
                    key: step.key,
                    t: step.from_t,
                });
            match landed.filter(|p| pathfind::locate(view.surfaces, *p).is_some()) {
                Some(perch) => {
                    self.perch = Some(perch);
                    self.motion = Motion::Walking;
                }
                None => {
                    self.motion = Motion::Falling;
                    self.arc = Arc_::Fall { velocity: 0. };
                    self.plan = None;
                }
            }
        } else {
            self.arc = Arc_::Jump {
                from,
                to,
                elapsed,
                duration,
                height,
            };
        }
    }

    fn step_air(&mut self, dt: f64, view: &WorldView<'_>) {
        let Arc_::Air {
            from,
            target,
            mut elapsed,
            duration,
        } = self.arc
        else {
            self.settle();
            return;
        };
        let Some(index) = pathfind::locate(view.surfaces, target) else {
            // The destination vanished mid-flight: pick another one.
            self.arc = Arc_::None;
            match self.nearest_perch(view, self.position) {
                Some(next) => self.begin_air(next, view),
                None => self.settle(),
            }
            return;
        };
        let destination = view.surfaces[index].point(target.t);
        elapsed += dt;
        let t = (elapsed / duration).min(1.);
        let eased = t * t * (3. - 2. * t);
        let base = Point::new(
            from.x + (destination.x - from.x) * eased,
            from.y + (destination.y - from.y) * eased,
        );
        let offset = air_offset(
            self.manifest().locomotion.air_style,
            t,
            self.clock,
            view.scale,
        );
        self.position = Point::new(base.x + offset.x, base.y + offset.y);
        self.rotation = 0.;
        if t >= 1. {
            self.position = destination;
            self.perch = Some(target);
            self.home.get_or_insert(target);
            self.arc = Arc_::None;
            self.settle();
        } else {
            self.arc = Arc_::Air {
                from,
                target,
                elapsed,
                duration,
            };
        }
    }

    fn step_fall(&mut self, dt: f64, view: &WorldView<'_>) {
        let Arc_::Fall { mut velocity } = self.arc else {
            self.arc = Arc_::Fall { velocity: 0. };
            return;
        };
        let gravity = GRAVITY * self.manifest().locomotion.gravity.max(0.35) * view.scale;
        velocity = (velocity + gravity * dt).min(TERMINAL_VELOCITY * view.scale);
        let from = self.position;
        let to = Point::new(from.x, from.y + velocity * dt);
        self.rotation = 0.;
        match self.landing_between(view, from, to) {
            Some((perch, point)) => {
                self.position = point;
                self.perch = Some(perch);
                self.arc = Arc_::None;
                self.settle();
                self.action_done = true;
            }
            None => {
                self.position = to;
                self.arc = Arc_::Fall { velocity };
                // Nothing below at all: the floor of the desktop always exists,
                // but if the world is empty, stop rather than fall forever.
                if to.y >= view.bounds.bottom() {
                    self.position.y = view.bounds.bottom();
                    self.arc = Arc_::None;
                    self.settle();
                    self.action_done = true;
                }
            }
        }
    }

    /// The first surface crossed by a downward move, so a falling pet lands on
    /// what is under it rather than on whatever is nearest.
    fn landing_between(
        &self,
        view: &WorldView<'_>,
        from: Point,
        to: Point,
    ) -> Option<(Perch, Point)> {
        let mut best: Option<(f64, Perch, Point)> = None;
        for (_, surface) in candidates_of(view, self.manifest()) {
            if surface.orientation() != Orientation::Horizontal || surface.normal().y >= 0. {
                continue;
            }
            let y = surface.start.y;
            if y < from.y - 0.5 || y > to.y + 0.5 {
                continue;
            }
            let (left, right) = (
                surface.start.x.min(surface.end.x),
                surface.start.x.max(surface.end.x),
            );
            if from.x < left || from.x > right {
                continue;
            }
            let point = Point::new(from.x, y);
            if best.as_ref().is_none_or(|(best_y, _, _)| y < *best_y) {
                best = Some((
                    y,
                    Perch {
                        key: surface.key(),
                        t: surface.t_of(point),
                    },
                    point,
                ));
            }
        }
        best.map(|(_, perch, point)| (perch, point))
    }

    fn settle(&mut self) {
        self.motion = Motion::Settled;
        self.action_done = true;
    }

    // -------------------------------------------------------- perch selection

    fn half_width(&self, scale: f64) -> f64 {
        self.manifest().sprite.width * self.manifest().sprite.scale * scale / 2.
    }

    fn perch_on(&self, surface: &Surface, t: f64, scale: f64) -> Option<Perch> {
        surface.fits(t, self.half_width(scale)).then_some(Perch {
            key: surface.key(),
            t,
        })
    }

    fn random_perch(&mut self, view: &WorldView<'_>) -> Option<Perch> {
        let current = self.perch.map(|p| p.key);
        let attention: Vec<&Surface> = view
            .attention
            .map(|owner| {
                candidates_of(view, self.manifest())
                    .into_iter()
                    .map(|(_, s)| s)
                    .filter(|s| s.owner == owner)
                    .collect()
            })
            .unwrap_or_default();
        if !attention.is_empty() {
            let surface = attention[self.rng.pick(attention.len())];
            if let Some(perch) = self.perch_on(surface, surface.midpoint_t(), view.scale) {
                return Some(perch);
            }
        }
        let options: Vec<&Surface> = candidates_of(view, self.manifest())
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| Some(s.key()) != current)
            .collect();
        let options = if options.is_empty() {
            candidates_of(view, self.manifest())
                .into_iter()
                .map(|(_, s)| s)
                .collect()
        } else {
            options
        };
        if options.is_empty() {
            return None;
        }
        let pick = self.rng.pick(options.len());
        let surface = options[pick];
        let t = surface.t_of(surface.point(self.rng.range(surface.t_start(), surface.t_end())));
        self.perch_on(surface, t, view.scale)
            .or_else(|| self.perch_on(surface, surface.midpoint_t(), view.scale))
    }

    fn nearest_perch(&self, view: &WorldView<'_>, to: Point) -> Option<Perch> {
        self.extreme_perch(view, to, true)
    }
    fn farthest_perch(&self, view: &WorldView<'_>, to: Point) -> Option<Perch> {
        self.extreme_perch(view, to, false)
    }
    fn extreme_perch(&self, view: &WorldView<'_>, to: Point, nearest: bool) -> Option<Perch> {
        let mut best: Option<(f64, Perch)> = None;
        for (_, surface) in candidates_of(view, self.manifest()) {
            let t = surface.t_of(to);
            let Some(perch) = self.perch_on(surface, t, view.scale) else {
                continue;
            };
            let distance = surface.point(t).distance(to);
            let better = best.as_ref().is_none_or(|(d, _)| {
                if nearest {
                    distance < *d
                } else {
                    distance > *d
                }
            });
            if better {
                best = Some((distance, perch));
            }
        }
        best.map(|(_, perch)| perch)
    }

    fn patrol_target(&mut self, view: &WorldView<'_>) -> Option<Perch> {
        let perch = self.perch?;
        let index = pathfind::locate(view.surfaces, perch)?;
        let surface = &view.surfaces[index];
        let t = self.rng.range(surface.t_start(), surface.t_end());
        self.perch_on(surface, t, view.scale)
    }

    fn clamp(&mut self, view: &WorldView<'_>) {
        if !self.position.is_finite() {
            self.position = view.bounds.center();
        }
        let margin = self.half_width(view.scale);
        self.position.x = self.position.x.clamp(
            view.bounds.x + margin,
            (view.bounds.right() - margin).max(view.bounds.x + margin),
        );
        self.position.y = self.position.y.clamp(view.bounds.y, view.bounds.bottom());
    }
}

/// The one-way render payload. Everything the webview needs, nothing more.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub pet_id: u64,
    pub clip: String,
    pub frame_index: u32,
    pub x: f64,
    pub y: f64,
    pub flip_x: bool,
    pub rotation: f64,
    pub opacity: f64,
    pub width: f64,
    pub height: f64,
    pub anchor_x: f64,
    pub anchor_y: f64,
}

impl Pet {
    /// The manifest anchor points into the texture, but frames are commonly
    /// exported with transparent rows at the bottom; anchoring there leaves
    /// the pet floating above its surface. Move the anchor up to the clip's
    /// ground line — its deepest visible row — so both the drawn position
    /// and the rotation pivot sit on the visible feet. Airborne frames of a
    /// hop or stride stay airborne, since the whole clip shares one line.
    fn anchored_y(&self) -> f64 {
        let sprite = self.manifest().sprite;
        let Some(strip) = self.pack.strip(self.clip()) else {
            return sprite.anchor_y;
        };
        if strip.ground == 0 {
            return sprite.anchor_y;
        }
        let pad = (strip.height - strip.ground) as f64;
        ((sprite.anchor_y * strip.height as f64 - pad) / strip.height as f64).max(0.)
    }

    pub fn frame(&self, scale: f64, opacity: f64) -> Frame {
        let sprite = self.manifest().sprite;
        Frame {
            pet_id: self.id,
            clip: self.clip().to_string(),
            frame_index: self.frame_index(),
            x: self.position.x / scale,
            y: self.position.y / scale,
            flip_x: self.flip_x,
            rotation: self.rotation,
            opacity,
            width: sprite.width * sprite.scale,
            height: sprite.height * sprite.scale,
            anchor_x: sprite.anchor_x,
            anchor_y: self.anchored_y(),
        }
    }

    /// Rectangle the sprite occupies in physical pixels, for hit-testing.
    pub fn sprite_rect(&self, scale: f64) -> Rect {
        let sprite = self.manifest().sprite;
        let (w, h) = (
            sprite.width * sprite.scale * scale,
            sprite.height * sprite.scale * scale,
        );
        let upright = self.manifest().sprite.surface_orientation == SurfaceOrientation::Upright
            || self.rotation.abs() < 1.;
        let (w, h) = if upright { (w, h) } else { (h, w) };
        Rect::new(
            self.position.x - w * sprite.anchor_x,
            self.position.y - h * self.anchored_y(),
            w,
            h,
        )
    }
}

fn candidates_of<'s>(view: &WorldView<'s>, manifest: &Manifest) -> Vec<(usize, &'s Surface)> {
    pathfind::candidates(view.surfaces, manifest, view.scale)
}

/// Where a new pet should appear: on a surface it likes, else mid-screen.
pub fn spawn_point(
    surfaces: &[Surface],
    manifest: &Manifest,
    bounds: Rect,
    scale: f64,
    rng: &mut Rng,
) -> (Point, Option<Perch>) {
    let options: Vec<&Surface> = pathfind::candidates(surfaces, manifest, scale)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    if options.is_empty() {
        return (
            Point::new(bounds.center().x, bounds.y + bounds.height * 0.3),
            None,
        );
    }
    let surface = options[rng.pick(options.len())];
    let t = surface.midpoint_t();
    (
        surface.point(t),
        Some(Perch {
            key: surface.key(),
            t,
        }),
    )
}

pub use pathfind::Perch as PetPerch;
pub type PerchKey = SurfaceKey;

#[cfg(test)]
mod tests;
