//! The pet runtime. Rust owns every truth here: where the pet is, what it is
//! doing, and what it will do next. The webview only draws what this produces.
pub mod fsm;
pub mod locomotion;
pub mod pathfind;

use crate::{
    jelly::Jelly,
    packs::{manifest::Event, Action, LocomotionClass, Manifest, Pack, Signals},
    world::{EdgeKind, Orientation, Point, Rect, Surface, SurfaceGraph, SurfaceKey},
};
use chrono::NaiveDate;
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
/// Fastest a pet can be thrown, per axis, in logical pixels per second.
const MAX_THROW: f64 = 2400.;
/// Release speed (logical px/s) from which a let-go counts as a throw.
const THROW_SPEED: f64 = 700.;
/// Landing speed (logical px/s) from which a thrown pet lands hard.
const HARD_LANDING: f64 = 900.;
/// How close another pet must come for `pet_nearby`, in logical pixels.
const PET_NEARBY: f64 = 140.;
/// The other pet must get this far away before `pet_nearby` can fire again.
const PET_REARM: f64 = 200.;
/// Horizontal air drag per 1/60 s, so a throw arcs rather than glides.
const AIR_DRAG: f64 = 0.985;
/// Cursor moves smaller than this (logical px) are jitter, not petting.
const PET_MIN_MOVE: f64 = 3.;
const PET_REVERSALS: usize = 3;
const PET_WINDOW: f64 = 1.5;
/// Squash-spring kicks, in squash per second. Positive squashes.
const CLICK_SQUASH: f64 = 3.;
const JUMP_STRETCH: f64 = 4.;
/// Landing speed, logical px/s, per unit of squash kick.
const LANDING_SQUASH_SPEED: f64 = 150.;
const MAX_LANDING_SQUASH: f64 = 8.;
const PETTED_SQUASH: f64 = 0.12;
/// A thrown pet stretches by its speed over this, up to the cap.
const THROWN_STRETCH_SPEED: f64 = 3000.;
const MAX_THROWN_STRETCH: f64 = 0.25;
/// Share of the window's swing a shaken-off pet carries with it.
const SHAKE_CARRY: f64 = 0.6;
/// Upward speed a shaken-off pet leaves with, logical px/s.
const SHAKE_POP: f64 = 450.;
const PET_COOLDOWN: f64 = 2.;
/// Slack in logical pixels beyond the sprite's half-width that still counts as
/// standing at the end of a surface; a pet is never planned closer than that.
const EDGE_SLACK: f64 = 6.;
/// Pointer speed, in logical pixels per second, above which `cursor_fast` fires.
const CURSOR_FAST: f64 = 1800.;
/// A fast pointer only startles a pet it is passing this close, in logical pixels.
const CURSOR_FLINCH_RANGE: f64 = 260.;
/// A resting pet turns to look at a pointer within this many logical pixels.
const CURSOR_WATCH_RANGE: f64 = 360.;
/// Weight of the newest sample in the smoothed pointer speed.
const CURSOR_SPEED_SMOOTHING: f64 = 0.25;

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

/// Why a pet is standing still with the user instead of wandering off.
/// Any of these pins it: the whole point of right-clicking a pet is that it
/// stays under the bubble that is pointing at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attend {
    /// The bubble is open on this pet.
    Listening,
    /// Its task is running.
    Busy,
    /// Its task cannot go further without an answer from the user.
    NeedsUser,
    /// The user is holding the mouse on it to pet it.
    Tended,
}

/// Mood a pet gains per second of being petted.
const PETTING_MOOD_PER_SECOND: f64 = 0.15;

impl Attend {
    /// The clip slot this maps to, or `None` to keep playing the pack's own
    /// behavior. Listening deliberately has no clip: a pet that stops mid-idle
    /// still reads as attentive, and inventing a third required slot would
    /// make packs harder to write for no gain.
    pub fn clip(self) -> Option<&'static str> {
        match self {
            Attend::Listening | Attend::Tended => None,
            Attend::Busy => Some("busy"),
            Attend::NeedsUser => Some("attention"),
        }
    }
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
    /// Horizontal and vertical speed in physical pixels per second.
    Fall {
        vx: f64,
        vy: f64,
    },
}

/// Another pet, as seen by the one looking: where it is and what it stands on.
#[derive(Clone, Copy, Debug)]
pub struct Peer {
    pub id: u64,
    pub position: Point,
    pub perch: Option<Perch>,
}

/// What the world looks like to one pet on one tick.
pub struct WorldView<'a> {
    /// Every pet, this one included; each ignores its own id.
    pub peers: &'a [Peer],
    pub surfaces: &'a [Surface],
    pub graph: &'a SurfaceGraph,
    pub bounds: Rect,
    pub cursor: Option<Point>,
    /// Physical pixels per logical pixel, so pack distances mean what the
    /// author expected on any display.
    pub scale: f64,
    /// User-facing speed preference, applied on top of the pack's own speeds.
    pub speed_scale: f64,
    /// Squish and wobble are on. Off, a pet still follows its window
    /// smoothly, without overshoot, squash or being shaken loose.
    pub jelly: bool,
    /// Surfaces of this pet's home biome: every edge of its window and its
    /// props. Empty when it has none on screen; otherwise the only places
    /// the pet chooses to go.
    pub home: &'a [SurfaceKey],
    /// Windows pressed against the top of the screen that could not be given
    /// room: a pet on their top edge would be cut off, so it goes elsewhere.
    pub cramped: &'a [u64],
    /// Window the user is working in. Only set while a nudge is escalating, so
    /// the pet drifts somewhere visible instead of somewhere random.
    pub attention: Option<u64>,
}

/// What a pet remembers about the user: a name, how long they have known each
/// other and how fond it has become. Dates come from the engine, never a clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Bond {
    pub name: Option<String>,
    pub met: NaiveDate,
    pub last_seen: NaiveDate,
    pub days: u32,
    pub affection: f64,
}
impl Bond {
    pub fn new(today: NaiveDate) -> Self {
        Self {
            name: None,
            met: today,
            last_seen: today,
            days: 1,
            affection: 0.2,
        }
    }
}

/// Rubbing the pointer back and forth over a pet: reversals of horizontal
/// direction inside its hit box, counted over a short window.
#[derive(Default)]
struct Petting {
    last_x: Option<f64>,
    direction: f64,
    reversals: Vec<f64>,
    quiet_until: f64,
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
    /// Speed multiplier of the current state's custom action (1 for built-ins).
    action_speed: f64,
    was_near_cursor: bool,
    was_near_pet: bool,
    pub bond: Bond,
    /// Cursor velocity while held, logical px/s, smoothed.
    held_velocity: Point,
    /// The current fall began with the user letting go, so a hard landing is
    /// theirs to cause; a window closing under the pet is not.
    thrown_fall: bool,
    petting: Petting,
    was_fast_cursor: bool,
    last_cursor: Option<Point>,
    cursor_speed: f64,
    clip_started: f64,
    /// The bubble is open on this pet.
    listening: bool,
    /// What this pet's task is doing, when it has one.
    working: Option<Attend>,
    /// Being petted by a held press right now.
    tended: bool,
    /// Where the pet stood when it was picked up, so a press that turns out
    /// to be petting puts it back instead of leaving it under the cursor.
    grabbed_from: Option<(Point, Option<Perch>)>,
    pending: Vec<Event>,
    /// The user asked this pet to go somewhere else; served on the next tick.
    relocate_requested: bool,
    /// Sound the pack asked for on the last state change, if any.
    pub sound_request: Option<String>,
    /// The folder this pet works in, if the user gave it one.
    pub folder: Option<String>,
    /// Springs for squash, stretch and the wobble of a dragged window. Only
    /// where the pet is drawn moves; `position` stays the truth.
    pub jelly: Jelly,
    /// The perch and the point it was at last tick, so a window moving under
    /// the pet can be told apart from the pet walking along it.
    ride_ref: Option<(Perch, Point)>,
    /// The biome the user made this pet's home.
    pub home_biome: Option<u64>,
}

/// Energy pulled by the time of day, per hour: late at night pets tire
/// faster than sleeping restores them, and mornings perk them up. Applied on
/// top of the activity-driven change, so it works for every pack.
pub fn circadian_drift(hour: f64) -> f64 {
    match hour {
        h if !(5. ..23.).contains(&h) => -0.3,
        h if (6. ..10.).contains(&h) => 0.3,
        _ => 0.,
    }
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
            arc: Arc_::Fall { vx: 0., vy: 0. },
            action_done: false,
            action_speed: 1.,
            was_near_cursor: false,
            was_near_pet: false,
            bond: Bond::new(NaiveDate::default()),
            held_velocity: Point::new(0., 0.),
            thrown_fall: false,
            petting: Petting::default(),
            was_fast_cursor: false,
            last_cursor: None,
            cursor_speed: 0.,
            clip_started: 0.,
            listening: false,
            working: None,
            tended: false,
            grabbed_from: None,
            pending: Vec::new(),
            relocate_requested: false,
            sound_request: None,
            folder: None,
            jelly: Jelly::default(),
            ride_ref: None,
            home_biome: None,
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.pack.manifest
    }
    /// What is actually drawn. A working or waiting pet plays its assistant
    /// clip instead of the behavior machine's, but only once it is standing
    /// still: a fall or a jump already in the air keeps its locomotion clip,
    /// because the physics has not stopped just because a task started.
    pub fn clip(&self) -> &str {
        match self.attend().and_then(Attend::clip) {
            Some(wanted) if self.motion == Motion::Settled => self.manifest().resolve_clip(wanted),
            _ => &self.machine.clip,
        }
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
        if self.motion != Motion::Held {
            self.grabbed_from = Some((self.position, self.perch));
        }
        self.motion = Motion::Held;
        self.arc = Arc_::None;
        self.plan = None;
        self.perch = None;
    }

    /// Let go. Speed at release becomes the throw; the pet reports `Thrown`
    /// when it was flung and its pack reacts to that, else plain `Released`.
    pub fn release(&mut self) {
        self.grabbed_from = None;
        if std::mem::take(&mut self.tended) {
            return;
        }
        if self.motion == Motion::Held {
            let clamp = |v: f64| v.clamp(-MAX_THROW, MAX_THROW);
            let (vx, vy) = (clamp(self.held_velocity.x), clamp(self.held_velocity.y));
            let speed = vx.hypot(vy);
            self.motion = Motion::Falling;
            self.arc = Arc_::Fall { vx, vy };
            self.thrown_fall = true;
            self.held_velocity = Point::new(0., 0.);
            self.mood = (self.mood + 0.05).min(1.);
            let reacts_to_throw = self
                .manifest()
                .reactions
                .iter()
                .any(|r| r.on == Event::Thrown);
            self.pending
                .push(if speed > THROW_SPEED && reacts_to_throw {
                    Event::Thrown
                } else {
                    Event::Released
                });
        }
    }

    /// Arrives from above rather than appearing on a perch: the first-run
    /// welcome uses it so the new pet visibly lands and greets.
    pub fn drop_in(&mut self) {
        self.motion = Motion::Falling;
        self.arc = Arc_::Fall { vx: 0., vy: 0. };
        self.perch = None;
        self.home = None;
        self.plan = None;
        self.pending.push(Event::Startup);
    }

    /// The name the user gave it, else the pack's.
    pub fn display_name(&self) -> &str {
        self.bond
            .name
            .as_deref()
            .unwrap_or(&self.pack.manifest.name)
    }

    fn gain(&mut self, amount: f64) {
        self.bond.affection = (self.bond.affection + amount).clamp(0., 1.);
    }

    /// A later calendar day than the last it was seen: one more day together.
    pub fn roll_day(&mut self, today: NaiveDate) {
        if today > self.bond.last_seen {
            self.bond.days += 1;
            self.bond.last_seen = today;
            self.pending.push(Event::NewDay);
        }
    }

    pub fn is_held(&self) -> bool {
        self.motion == Motion::Held
    }

    /// The press stayed where it was: this is petting, not a drag. The pet
    /// goes back to where it was picked up and stays there until the button
    /// comes up. Petting raises mood for as long as it lasts.
    pub fn begin_tending(&mut self) {
        if self.motion != Motion::Held {
            return;
        }
        if let Some((position, perch)) = self.grabbed_from.take() {
            self.position = position;
            self.perch = perch;
        }
        if self.perch.is_some() {
            self.motion = Motion::Settled;
            self.arc = Arc_::None;
        } else {
            // It was airborne when picked up; it drops while being petted.
            self.motion = Motion::Falling;
            self.arc = Arc_::Fall { vx: 0., vy: 0. };
        }
        self.held_velocity = Point::new(0., 0.);
        self.plan = None;
        self.action_done = true;
        self.tended = true;
        self.pending.push(Event::Clicked);
    }

    pub fn is_tended(&self) -> bool {
        self.tended
    }

    /// Whether this pet is with the user, and why. A task outranks the bubble:
    /// what it is doing is more informative than the fact that it is listening.
    pub fn attend(&self) -> Option<Attend> {
        self.working
            .or(self.tended.then_some(Attend::Tended))
            .or(self.listening.then_some(Attend::Listening))
    }

    /// The bubble opened or closed on this pet.
    pub fn listen(&mut self, listening: bool) {
        if self.listening == listening {
            return;
        }
        self.listening = listening;
        self.settle_for_attention();
    }

    /// What this pet's task is doing: `Busy` while it runs, `NeedsUser` while
    /// it waits for an answer, `None` once it is over.
    pub fn work(&mut self, working: Option<Attend>) {
        if self.working == working {
            return;
        }
        self.working = working;
        // A new clip starts at its first frame, not partway through whatever
        // the previous one had reached.
        self.clip_started = self.clock;
        self.settle_for_attention();
    }

    /// An attending pet stops where it is. Mid-air it cannot — a jump or a
    /// fall finishes first — but it will not start anything new (see
    /// `start_action`), so it settles on landing and stays there.
    fn settle_for_attention(&mut self) {
        if self.attend().is_none() || self.motion != Motion::Walking {
            return;
        }
        self.plan = None;
        self.settle();
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
            self.arc = Arc_::Fall { vx: 0., vy: 0. };
        }
    }

    /// A window yanked across the desktop is alarming; the pack decides what
    /// that looks like through its `surface_moved_fast` reaction.
    pub fn startle(&mut self) {
        self.pending.push(Event::SurfaceMovedFast);
    }

    pub fn click(&mut self) {
        self.pending.push(Event::Clicked);
        self.jelly.impulse(CLICK_SQUASH);
        self.mood = (self.mood + 0.03).min(1.);
        self.gain(0.005);
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

    /// Travel to another surface, as asked from the tray or settings. This is
    /// a command, not a reaction: it must not go through `SurfaceLost`, which
    /// packs answer with a startled hop on the spot.
    pub fn relocate(&mut self) {
        self.relocate_requested = true;
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
        self.track_cursor(dt, view);
        let signals = self.signals(base, view);
        self.update_scalars(dt, &signals);
        self.detect_events(&signals, view, events);
        let jelly = if view.jelly {
            self.manifest().jelly_params()
        } else {
            self.manifest().jelly_params().critically_damped()
        };
        if self.motion == Motion::Held {
            self.ride_ref = None;
            self.jelly.detach();
            if let Some(cursor) = view.cursor {
                if dt > 0. {
                    let instant = Point::new(
                        (cursor.x - self.position.x) / dt / view.scale,
                        (cursor.y - self.position.y) / dt / view.scale,
                    );
                    self.held_velocity = Point::new(
                        0.5 * self.held_velocity.x + 0.5 * instant.x,
                        0.5 * self.held_velocity.y + 0.5 * instant.y,
                    );
                }
                self.position = cursor;
            }
            self.clamp(view);
            self.step_machine(dt, &signals, events, view);
            self.jelly.step(dt, jelly);
            return signals;
        }
        self.ride(dt, view, events);
        if view.jelly
            && self.perch.is_some()
            && matches!(self.motion, Motion::Settled | Motion::Walking)
            && self.jelly.shaken_off(view.scale)
        {
            self.shake_off(view, events);
        }
        if std::mem::take(&mut self.relocate_requested) && self.motion != Motion::Falling {
            if let Some(target) = self.random_perch(view) {
                self.travel_to(target, view);
            }
        }
        self.step_machine(dt, &signals, events, view);
        self.step_motion(dt, view);
        self.clamp(view);
        if self.tended {
            self.jelly.aim(PETTED_SQUASH);
        } else if self.thrown_fall && self.motion == Motion::Falling {
            if let Arc_::Fall { vx, vy } = self.arc {
                let speed = vx.hypot(vy) / view.scale;
                self.jelly
                    .aim(-(speed / THROWN_STRETCH_SPEED).min(MAX_THROWN_STRETCH));
            }
        }
        self.jelly.step(dt, jelly);
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
            cursor_speed: self.cursor_speed,
            is_dragging: self.motion == Motion::Held,
            pet_distance: self
                .nearest_peer(view)
                .map(|peer| peer.position.distance(self.position) / view.scale)
                .unwrap_or(f64::INFINITY),
            affection: self.bond.affection,
            days_together: self.bond.days as f64,
            ..base
        }
    }

    /// Smooth the pointer's speed so one jittery sample is not a flinch.
    fn track_cursor(&mut self, dt: f64, view: &WorldView<'_>) {
        let sample = match (self.last_cursor, view.cursor) {
            (Some(from), Some(to)) if dt > 0. => from.distance(to) / view.scale / dt,
            _ => 0.,
        };
        self.last_cursor = view.cursor;
        self.cursor_speed += (sample - self.cursor_speed) * CURSOR_SPEED_SMOOTHING;
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
        self.energy = (self.energy + circadian_drift(signals.hour) * hours).clamp(0., 1.);
        let mut mood = self.mood;
        if self.tended {
            mood += PETTING_MOOD_PER_SECOND * dt;
        }
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
        if events.contains(&Event::UserReturned) {
            self.gain(0.01);
        }
        if signals.pet_distance < PET_NEARBY && !self.was_near_pet {
            events.push(Event::PetNearby);
            self.was_near_pet = true;
        } else if signals.pet_distance > PET_REARM {
            self.was_near_pet = false;
        }
        let fast = self.cursor_speed > CURSOR_FAST && signals.cursor_distance < CURSOR_FLINCH_RANGE;
        if fast && !self.was_fast_cursor {
            events.push(Event::CursorFast);
        }
        self.was_fast_cursor = fast;
        self.watch_cursor(signals, view);
        if self.energy < 0.15 {
            events.push(Event::LowEnergy);
        }
        self.detect_petting(view, events);
    }

    /// Three direction reversals of the pointer inside the sprite within 1.5 s.
    fn detect_petting(&mut self, view: &WorldView<'_>, events: &mut Vec<Event>) {
        let hit = view.cursor.filter(|cursor| {
            let sprite = &self.manifest().sprite;
            let half = self.half_width(view.scale);
            let height = sprite.height * sprite.scale * view.scale;
            self.motion != Motion::Held
                && (cursor.x - self.position.x).abs() <= half
                && cursor.y <= self.position.y
                && cursor.y >= self.position.y - height
        });
        let Some(cursor) = hit else {
            self.petting.last_x = None;
            self.petting.direction = 0.;
            self.petting.reversals.clear();
            return;
        };
        let Some(last) = self.petting.last_x else {
            self.petting.last_x = Some(cursor.x);
            return;
        };
        let moved = cursor.x - last;
        if moved.abs() < PET_MIN_MOVE * view.scale {
            return;
        }
        let direction = moved.signum();
        if self.petting.direction != 0. && direction != self.petting.direction {
            self.petting.reversals.push(self.clock);
        }
        self.petting.direction = direction;
        self.petting.last_x = Some(cursor.x);
        let now = self.clock;
        self.petting.reversals.retain(|t| now - t <= PET_WINDOW);
        if self.petting.reversals.len() >= PET_REVERSALS && now >= self.petting.quiet_until {
            self.petting.reversals.clear();
            self.petting.quiet_until = now + PET_COOLDOWN;
            self.mood = (self.mood + 0.08).min(1.);
            self.gain(0.02);
            events.push(Event::Petted);
        }
    }

    /// A resting pet turns its head toward a nearby pointer.
    fn watch_cursor(&mut self, signals: &Signals, view: &WorldView<'_>) {
        if self.motion != Motion::Settled || signals.cursor_distance > CURSOR_WATCH_RANGE {
            return;
        }
        if let Some(cursor) = view.cursor {
            self.flip_x = cursor.x < self.position.x;
        }
    }

    /// Hold `(surface, t)` as windows move, and notice when the perch is gone.
    fn ride(&mut self, dt: f64, view: &WorldView<'_>, events: &mut Vec<Event>) {
        let reference = self.ride_ref.take();
        let Some(perch) = self.perch else {
            return;
        };
        match pathfind::locate(view.surfaces, perch) {
            Some(index) => {
                let surface = &view.surfaces[index];
                let half = self.half_width(view.scale);
                // A pet walking a route legitimately stands at the very end of
                // a surface: that is where it takes off for the next window, or
                // where it lands from one. Requiring clearance there would call
                // the surface lost the moment it got to the launch point and
                // freeze it at the edge for good.
                let fits = if self.motion == Motion::Walking && self.plan.is_some() {
                    surface.contains_t(perch.t)
                } else {
                    surface.fits(perch.t, half)
                };
                if !fits {
                    // The perch shrank under the pet: find somewhere that fits.
                    events.push(Event::SurfaceLost);
                    self.lose_surface(view);
                    return;
                }
                if self.motion == Motion::Settled || self.motion == Motion::Walking {
                    // Only the window moving counts: the same spot on the
                    // same edge, now and last tick. Walking changes `t`.
                    let moved = match reference {
                        Some((last, at)) if last.key == perch.key => {
                            let now = surface.point(last.t);
                            Point::new(now.x - at.x, now.y - at.y)
                        }
                        _ => Point::default(),
                    };
                    self.jelly.follow(moved, dt, view.scale);
                    self.position = surface.point(perch.t);
                    self.rotation = sprite_rotation(self.manifest(), surface);
                    self.ride_ref = Some((perch, self.position));
                }
            }
            None => {
                events.push(Event::SurfaceLost);
                self.lose_surface(view);
            }
        }
    }

    /// A window shaken hard enough loosens the pet's grip: it comes off with
    /// the window's swing, the way a throw would send it.
    fn shake_off(&mut self, view: &WorldView<'_>, events: &mut Vec<Event>) {
        let swing = self.jelly.perch_velocity;
        self.jelly.detach();
        self.ride_ref = None;
        events.push(Event::Thrown);
        self.lose_surface(view);
        if self.motion == Motion::Falling {
            // Popped up as well as sideways, or it would land straight back
            // on the edge it was standing on.
            let up = (swing.y.min(0.) * SHAKE_CARRY).min(-SHAKE_POP * view.scale);
            self.arc = Arc_::Fall {
                vx: (swing.x * SHAKE_CARRY).clamp(-MAX_THROW, MAX_THROW),
                vy: up.max(-MAX_THROW),
            };
            self.thrown_fall = true;
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
            self.arc = Arc_::Fall { vx: 0., vy: 0. };
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
            self.action_speed = manifest
                .behavior
                .states
                .get(&entered.state)
                .map_or(1., |s| s.speed_scale);
            if let Some(action) = entered.action {
                self.start_action(action, view);
            }
        }
        events.clear();
    }

    fn start_action(&mut self, action: Action, view: &WorldView<'_>) {
        // A pet the user is talking to does not wander off mid-sentence. The
        // machine keeps running so reactions and clips still play; only the
        // movement the state asked for is dropped.
        if self.attend().is_some() || self.motion == Motion::Held || self.motion == Motion::Falling
        {
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
            Action::MoveTowardPet => self.peer_perch(view),
            Action::ReturnToHomeSurface => self
                .biome_perch(view)
                .or(self.home)
                .or_else(|| self.random_perch(view)),
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
        let speed = self.manifest().locomotion.speed.air.max(20.)
            * view.scale
            * view.speed_scale
            * self.action_speed;
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
        let speed = locomotion::speed(self.manifest(), surface)
            * view.scale
            * view.speed_scale
            * self.action_speed;
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
                            * view.speed_scale
                            * self.action_speed;
                        self.arc = Arc_::Jump {
                            from: self.position,
                            to,
                            elapsed: 0.,
                            duration: (self.position.distance(to) / speed).clamp(0.25, 1.6),
                            height: jump.arc_height * view.scale,
                        };
                        self.motion = Motion::Jumping;
                        self.jelly.impulse(-JUMP_STRETCH);
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
                let reach = self.half_width(view.scale) + EDGE_SLACK * view.scale;
                let to_end = surface.span_length() * perch.t.min(1. - perch.t);
                let at_end = to_end <= reach;
                if at_end && surface.orientation() == Orientation::Horizontal {
                    self.pending.push(Event::EdgeReached);
                }
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
                    self.arc = Arc_::Fall { vx: 0., vy: 0. };
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
        let Arc_::Fall { mut vx, mut vy } = self.arc else {
            self.arc = Arc_::Fall { vx: 0., vy: 0. };
            return;
        };
        let gravity = GRAVITY * self.manifest().locomotion.gravity.max(0.35) * view.scale;
        vy = (vy + gravity * dt).min(TERMINAL_VELOCITY * view.scale);
        vx *= AIR_DRAG.powf(dt * 60.);
        let from = self.position;
        let mut to = Point::new(from.x + vx * dt, from.y + vy * dt);
        // A fling bounces off the sides of the world instead of leaving it.
        let margin = self.half_width(view.scale);
        let (left, right) = (
            view.bounds.x + margin,
            (view.bounds.right() - margin).max(view.bounds.x + margin),
        );
        if to.x < left || to.x > right {
            to.x = to.x.clamp(left, right);
            vx = -vx * 0.5;
        }
        self.rotation = 0.;
        match self.landing_between(view, from, to) {
            Some((perch, point)) => {
                self.position = point;
                self.perch = Some(perch);
                self.land(vy, view);
            }
            None => {
                self.position = to;
                self.arc = Arc_::Fall { vx, vy };
                // Nothing below at all: the floor of the desktop always exists,
                // but if the world is empty, stop rather than fall forever.
                if to.y >= view.bounds.bottom() {
                    self.position.y = view.bounds.bottom();
                    self.land(vy, view);
                }
            }
        }
    }

    fn land(&mut self, vy: f64, view: &WorldView<'_>) {
        self.jelly
            .impulse((vy / view.scale / LANDING_SQUASH_SPEED).clamp(0., MAX_LANDING_SQUASH));
        if self.thrown_fall && vy > HARD_LANDING * view.scale {
            self.pending.push(Event::LandedHard);
        } else {
            self.pending.push(Event::Landed);
        }
        self.thrown_fall = false;
        self.arc = Arc_::None;
        self.settle();
        self.action_done = true;
    }

    /// The first surface crossed by a downward move, so a falling pet lands on
    /// what is under it rather than on whatever is nearest. The move may be
    /// diagonal: the surface is hit where the segment crosses its height.
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
            let across = if to.y > from.y {
                ((y - from.y) / (to.y - from.y)).clamp(0., 1.)
            } else {
                0.
            };
            let x = from.x + (to.x - from.x) * across;
            let (left, right) = (
                surface.start.x.min(surface.end.x),
                surface.start.x.max(surface.end.x),
            );
            if x < left || x > right {
                continue;
            }
            let point = Point::new(x, y);
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

    /// Somewhere in this pet's home biome, if it has one on screen.
    fn biome_perch(&mut self, view: &WorldView<'_>) -> Option<Perch> {
        let homes: Vec<&Surface> = candidates_of(view, self.manifest())
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| view.home.contains(&s.key()))
            .collect();
        if homes.is_empty() {
            return None;
        }
        let surface = homes[self.rng.pick(homes.len())];
        let t = self.rng.range(surface.t_start(), surface.t_end());
        self.perch_on(surface, t, view.scale)
            .or_else(|| self.perch_on(surface, surface.midpoint_t(), view.scale))
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

    fn nearest_peer(&self, view: &WorldView<'_>) -> Option<Peer> {
        view.peers
            .iter()
            .filter(|peer| peer.id != self.id)
            .min_by(|a, b| {
                a.position
                    .distance(self.position)
                    .total_cmp(&b.position.distance(self.position))
            })
            .copied()
    }

    /// Somewhere to stand beside the nearest other pet: its own perch when this
    /// pet may use it, else the closest allowed one, a sprite's width short of
    /// it on the side this pet comes from.
    fn peer_perch(&self, view: &WorldView<'_>) -> Option<Perch> {
        let peer = self.nearest_peer(view)?;
        let beside = |perch: Perch| -> Option<Perch> {
            let index = pathfind::locate(view.surfaces, perch)?;
            let surface = &view.surfaces[index];
            let gap = self.half_width(view.scale) * 2.2 / surface.span_length().max(1.);
            let from_left = self.position.x < peer.position.x;
            let t = if from_left {
                perch.t - gap
            } else {
                perch.t + gap
            };
            let t = t.clamp(surface.t_start(), surface.t_end());
            self.perch_on(surface, t, view.scale)
                .or_else(|| self.perch_on(surface, perch.t, view.scale))
        };
        peer.perch
            .filter(|perch| {
                pathfind::locate(view.surfaces, *perch).is_some_and(|i| {
                    pathfind::allowed(self.manifest(), &view.surfaces[i], view.scale)
                })
            })
            .and_then(beside)
            .or_else(|| self.nearest_perch(view, peer.position))
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
    /// Squash and stretch about the anchor; 1 is the sprite as drawn.
    pub squash_x: f64,
    pub squash_y: f64,
    /// Degrees added to `rotation` while the pet wobbles.
    pub lean: f64,
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

    /// Where the pet is drawn: its position plus the wobble.
    pub fn drawn_position(&self) -> Point {
        Point::new(
            self.position.x + self.jelly.offset.x.x,
            self.position.y + self.jelly.offset.x.y,
        )
    }

    pub fn frame(&self, scale: f64, opacity: f64) -> Frame {
        let sprite = self.manifest().sprite;
        let drawn = self.drawn_position();
        let (squash_x, squash_y) = self
            .jelly
            .scales(self.clock, self.motion == Motion::Settled);
        Frame {
            pet_id: self.id,
            clip: self.clip().to_string(),
            frame_index: self.frame_index(),
            x: drawn.x / scale,
            y: drawn.y / scale,
            flip_x: self.flip_x,
            rotation: self.rotation,
            opacity,
            width: sprite.width * sprite.scale,
            height: sprite.height * sprite.scale,
            anchor_x: sprite.anchor_x,
            anchor_y: self.anchored_y(),
            squash_x,
            squash_y,
            lean: self.jelly.lean(scale),
        }
    }

    /// Rotation as drawn, wobble included.
    fn drawn_rotation(&self, scale: f64) -> f64 {
        self.rotation + self.jelly.lean(scale)
    }

    /// Bounding box of the drawn sprite in physical pixels, for hit-testing.
    /// The webview rotates about the anchor, so a climbing or hanging pet
    /// sits beside or below its perch rather than above it.
    pub fn sprite_rect(&self, scale: f64) -> Rect {
        let (w, h, ax, ay) = self.sprite_box(scale);
        let (sin, cos) = self.drawn_rotation(scale).to_radians().sin_cos();
        let drawn = self.drawn_position();
        let corners = [(-ax, -ay), (w - ax, -ay), (-ax, h - ay), (w - ax, h - ay)]
            .map(|(x, y)| (x * cos - y * sin, x * sin + y * cos));
        let (mut left, mut top) = (f64::INFINITY, f64::INFINITY);
        let (mut right, mut bottom) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (x, y) in corners {
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
        }
        Rect::new(drawn.x + left, drawn.y + top, right - left, bottom - top)
    }

    /// Where `point` falls within the unrotated, unflipped sprite, in physical
    /// pixels from its top-left: the inverse of the webview's transform.
    pub fn sprite_local(&self, point: Point, scale: f64) -> Point {
        let (_, _, ax, ay) = self.sprite_box(scale);
        let (sin, cos) = self.drawn_rotation(scale).to_radians().sin_cos();
        let drawn = self.drawn_position();
        let (dx, dy) = (point.x - drawn.x, point.y - drawn.y);
        let x = dx * cos + dy * sin;
        let y = -dx * sin + dy * cos;
        let x = if self.flip_x { -x } else { x };
        // Back into the unsquashed sprite. Breathing is too small to matter.
        let (sx, sy) = self.jelly.scales(self.clock, false);
        Point::new((x + ax) / sx, (y + ay) / sy)
    }

    /// Drawn width, height and anchor offset, in physical pixels.
    fn sprite_box(&self, scale: f64) -> (f64, f64, f64, f64) {
        let sprite = self.manifest().sprite;
        let (w, h) = (
            sprite.width * sprite.scale * scale,
            sprite.height * sprite.scale * scale,
        );
        let (sx, sy) = self.jelly.scales(self.clock, false);
        let (w, h) = (w * sx, h * sy);
        (w, h, w * sprite.anchor_x, h * self.anchored_y())
    }
}

fn candidates_of<'s>(view: &WorldView<'s>, manifest: &Manifest) -> Vec<(usize, &'s Surface)> {
    let mut candidates = pathfind::candidates(view.surfaces, manifest, view.scale);
    candidates.retain(|(_, s)| {
        !(s.kind == crate::world::SurfaceKind::WindowTop && view.cramped.contains(&s.owner))
    });
    // A pet with a home biome on screen lives there: its window and props are
    // the only places it chooses, though it may cross others to get there.
    let home: Vec<(usize, &'s Surface)> = candidates
        .iter()
        .copied()
        .filter(|(_, s)| view.home.contains(&s.key()))
        .collect();
    if home.is_empty() {
        candidates
    } else {
        home
    }
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
