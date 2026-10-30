//! Squash, stretch and wobble. Pure spring maths with no knowledge of pets or
//! windows, so a pet and a biome prop can share it and tests can drive it.
//!
//! The engine never moves a pet's real position for any of this: `offset` is
//! added only where the pet is drawn and hit-tested. When the window under a
//! pet jumps (window scans arrive at 10 Hz while it moves), the offset absorbs
//! the jump and a spring pulls it back to zero, which is both the smoothing
//! and the wobble.
use crate::world::Point;

/// A jump this large is a window teleporting (a snap, a space switch), not a
/// drag; following it would fling the pet across the screen.
const SNAP_DISTANCE: f64 = 500.;
/// Lean per physical pixel the drawn pet lags behind its perch, in degrees.
const LEAN_PER_PIXEL: f64 = 0.25;
const MAX_LEAN: f64 = 18.;
/// Squash velocity per pixel the perch jumped vertically: a window yanked up
/// squashes the pet, one dropped away stretches it.
const SQUASH_PER_PIXEL: f64 = 0.012;
const MAX_SQUASH: f64 = 0.35;
/// Shake decays with this half-life, in seconds.
const SHAKE_HALF_LIFE: f64 = 0.4;
/// Accumulated change of perch velocity, physical px/s, that loosens a grip
/// of 0; a grip `g` holds out to `SHAKE_UNIT / (1 - g)`.
const SHAKE_UNIT: f64 = 4000.;
/// Perch jumps further apart than this are separate movements, not one shake.
const SHAKE_GAP: f64 = 0.3;
/// What the first jump of a movement is measured over: the window scan
/// interval while something moves.
const SCAN_INTERVAL: f64 = 0.1;
const BREATH_DEPTH: f64 = 0.02;
const BREATH_PERIOD: f64 = 3.2;
const SUBSTEP: f64 = 1. / 240.;

/// How a body springs. `stiffness` is ω² in s⁻², `damping` the damping ratio
/// (1 = no overshoot), `grip` 0..1 how hard it holds on (1 never lets go).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JellyParams {
    pub stiffness: f64,
    pub damping: f64,
    pub squash: f64,
    pub grip: f64,
    pub breath: f64,
}

impl Default for JellyParams {
    fn default() -> Self {
        Self {
            stiffness: 220.,
            damping: 0.35,
            squash: 1.,
            grip: 0.6,
            breath: 1.,
        }
    }
}

impl JellyParams {
    /// Follows smoothly with no overshoot, squash, breathing or letting go:
    /// what "squish and wobble" switched off, or reduced motion, looks like.
    pub fn critically_damped(self) -> Self {
        Self {
            damping: 1.,
            squash: 0.,
            grip: 1.,
            breath: 0.,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring1 {
    pub x: f64,
    pub v: f64,
}

impl Spring1 {
    /// Toward `target`, sub-stepped so a long frame cannot blow up.
    pub fn step(&mut self, dt: f64, stiffness: f64, damping: f64, target: f64) {
        let c = 2. * damping * stiffness.sqrt();
        let mut left = dt.max(0.);
        while left > 0. {
            let h = left.min(SUBSTEP);
            self.v += (-stiffness * (self.x - target) - c * self.v) * h;
            self.x += self.v * h;
            left -= h;
        }
    }
    pub fn is_at_rest(&self, eps: f64) -> bool {
        self.x.abs() < eps && self.v.abs() < eps * 10.
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring2 {
    pub x: Point,
    pub v: Point,
}

impl Spring2 {
    pub fn step(&mut self, dt: f64, stiffness: f64, damping: f64) {
        let mut x = Spring1 {
            x: self.x.x,
            v: self.v.x,
        };
        let mut y = Spring1 {
            x: self.x.y,
            v: self.v.y,
        };
        x.step(dt, stiffness, damping, 0.);
        y.step(dt, stiffness, damping, 0.);
        self.x = Point::new(x.x, y.x);
        self.v = Point::new(x.v, y.v);
    }
    pub fn is_at_rest(&self, eps: f64) -> bool {
        self.x.x.hypot(self.x.y) < eps && self.v.x.hypot(self.v.y) < eps * 10.
    }
}

#[derive(Clone, Debug, Default)]
pub struct Jelly {
    /// Drawn position minus real position, physical pixels.
    pub offset: Spring2,
    /// Positive squashes (wider, shorter), negative stretches.
    pub squash: Spring1,
    pub params: JellyParams,
    /// How violently the perch has been changing velocity lately.
    pub shake: f64,
    /// Perch velocity over the last jump, physical px/s.
    pub perch_velocity: Point,
    /// Seconds since the perch last jumped, while that is recent.
    since_jump: Option<f64>,
    target: f64,
}

impl Jelly {
    /// The perch moved by `moved` since the last tick (zero most ticks: window
    /// scans are slower than frames).
    pub fn follow(&mut self, moved: Point, dt: f64, scale: f64) {
        let recent = self.since_jump.map(|s| s + dt).filter(|&s| s <= SHAKE_GAP);
        self.since_jump = recent;
        if recent.is_none() {
            self.perch_velocity = Point::default();
        }
        let distance = moved.x.hypot(moved.y);
        if distance < 1e-6 {
            return;
        }
        if distance > SNAP_DISTANCE * scale {
            self.offset = Spring2::default();
            self.perch_velocity = Point::default();
            self.since_jump = None;
            return;
        }
        let elapsed = recent.unwrap_or(SCAN_INTERVAL).max(dt).max(1e-3);
        let velocity = Point::new(moved.x / elapsed, moved.y / elapsed);
        let previous = self.perch_velocity;
        self.shake += (velocity.x - previous.x).hypot(velocity.y - previous.y);
        self.perch_velocity = velocity;
        self.since_jump = Some(0.);
        self.offset.x = Point::new(self.offset.x.x - moved.x, self.offset.x.y - moved.y);
        self.squash.v -= moved.y / scale.max(0.01) * SQUASH_PER_PIXEL * self.params.squash;
    }

    /// The pet stopped riding a perch: forget its motion, keep the wobble.
    pub fn detach(&mut self) {
        self.perch_velocity = Point::default();
        self.since_jump = None;
        self.shake = 0.;
    }

    /// A kick to the squash spring: positive squashes, negative stretches.
    pub fn impulse(&mut self, velocity: f64) {
        self.squash.v += velocity * self.params.squash;
    }

    /// Hold a squash for this tick (petting, a stretched throw).
    pub fn aim(&mut self, target: f64) {
        self.target = target;
    }

    pub fn step(&mut self, dt: f64, params: JellyParams) {
        self.params = params;
        self.offset.step(dt, params.stiffness, params.damping);
        let target = self.target * params.squash;
        self.squash
            .step(dt, params.stiffness * 2., params.damping, target);
        let limit = MAX_SQUASH * params.squash;
        self.squash.x = self.squash.x.clamp(-limit, limit);
        if params.squash == 0. {
            self.squash = Spring1::default();
        }
        self.shake *= 0.5_f64.powf(dt / SHAKE_HALF_LIFE);
        self.target = 0.;
    }

    pub fn shaken_off(&self, scale: f64) -> bool {
        self.params.grip < 1. && self.shake * (1. - self.params.grip) > SHAKE_UNIT * scale
    }

    /// Degrees the drawn pet leans: its body lags behind its dragged feet.
    pub fn lean(&self, scale: f64) -> f64 {
        if self.params.squash == 0. {
            return 0.;
        }
        (self.offset.x.x / scale.max(0.01) * LEAN_PER_PIXEL).clamp(-MAX_LEAN, MAX_LEAN)
    }

    /// Horizontal and vertical scale, keeping the area about the same.
    pub fn scales(&self, clock: f64, settled: bool) -> (f64, f64) {
        let mut s = self.squash.x;
        if settled {
            s += BREATH_DEPTH
                * self.params.breath
                * (std::f64::consts::TAU * clock / BREATH_PERIOD).sin();
        }
        (1. + s, 1. - s)
    }

    /// Still moving visibly, so the loop should run at full frame rate.
    pub fn is_active(&self) -> bool {
        !self.offset.is_at_rest(0.5) || !self.squash.is_at_rest(0.005)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1. / 60.;

    fn run(jelly: &mut Jelly, params: JellyParams, seconds: f64, mut each: impl FnMut(&Jelly)) {
        let mut t = 0.;
        while t < seconds {
            jelly.follow(Point::default(), DT, 1.);
            jelly.step(DT, params);
            each(jelly);
            t += DT;
        }
    }

    #[test]
    fn a_jump_overshoots_then_settles_back_to_the_perch() {
        let params = JellyParams::default();
        let mut jelly = Jelly {
            params,
            ..Default::default()
        };
        jelly.follow(Point::new(100., 0.), DT, 1.);
        assert_eq!(jelly.offset.x.x, -100., "the drawn pet stays put at first");
        let mut overshoot = f64::NEG_INFINITY;
        run(&mut jelly, params, 1.5, |j| overshoot = overshoot.max(j.offset.x.x));
        assert!(overshoot > 5., "it swings past the perch: {overshoot}");
        assert!(jelly.offset.x.x.abs() < 1., "settled: {:?}", jelly.offset);
        assert!(!jelly.is_active());
    }

    #[test]
    fn critically_damped_never_overshoots() {
        let params = JellyParams::default().critically_damped();
        let mut jelly = Jelly {
            params,
            ..Default::default()
        };
        jelly.follow(Point::new(100., 0.), DT, 1.);
        run(&mut jelly, params, 1.5, |j| {
            assert!(j.offset.x.x <= 1e-9, "overshot to {}", j.offset.x.x)
        });
        assert!(jelly.offset.x.x.abs() < 1.);
        assert_eq!(jelly.lean(1.), 0.);
        assert_eq!(jelly.scales(0.8, true), (1., 1.));
    }

    #[test]
    fn a_teleporting_window_is_not_followed() {
        let mut jelly = Jelly::default();
        jelly.follow(Point::new(900., 0.), DT, 1.);
        assert_eq!(jelly.offset.x, Point::default());
    }

    fn shake(grip: f64) -> bool {
        let params = JellyParams {
            grip,
            ..Default::default()
        };
        let mut jelly = Jelly {
            params,
            ..Default::default()
        };
        let mut off = false;
        // ±250 px every scan (0.1 s), the way a hand shakes a window.
        for i in 0..12 {
            for tick in 0..6 {
                let moved = if tick == 0 {
                    Point::new(if i % 2 == 0 { 250. } else { -250. }, 0.)
                } else {
                    Point::default()
                };
                jelly.follow(moved, DT, 1.);
                jelly.step(DT, params);
                off |= jelly.shaken_off(1.);
            }
        }
        off
    }

    #[test]
    fn a_hard_shake_loosens_a_normal_grip_but_not_a_firm_one() {
        assert!(shake(0.6));
        assert!(!shake(0.95));
        assert!(!shake(1.));
    }

    #[test]
    fn a_steady_drag_does_not_shake_anything_off() {
        let params = JellyParams {
            grip: 0.3,
            ..Default::default()
        };
        let mut jelly = Jelly {
            params,
            ..Default::default()
        };
        for tick in 0..120 {
            let moved = if tick % 6 == 0 && tick < 90 {
                Point::new(150., 0.)
            } else {
                Point::default()
            };
            jelly.follow(moved, DT, 1.);
            jelly.step(DT, params);
            assert!(!jelly.shaken_off(1.), "a 1500 px/s drag is not a shake");
        }
    }

    #[test]
    fn squash_stays_bounded_and_keeps_the_area() {
        let params = JellyParams::default();
        let mut jelly = Jelly {
            params,
            ..Default::default()
        };
        jelly.impulse(50.);
        run(&mut jelly, params, 2., |j| {
            let (sx, sy) = j.scales(0.4, false);
            assert!(sy >= 1. - MAX_SQUASH - 1e-9 && sy <= 1. + MAX_SQUASH + 1e-9);
            assert!((sx * sy - 1.).abs() < 0.13, "area {sx} × {sy}");
        });
    }

    #[test]
    fn a_settled_pet_breathes_and_a_moving_one_does_not() {
        let jelly = Jelly {
            params: JellyParams::default(),
            ..Default::default()
        };
        let (_, sy) = jelly.scales(0.8, true);
        assert!((sy - 1.).abs() > 0.01);
        assert_eq!(jelly.scales(0.8, false), (1., 1.));
    }
}
