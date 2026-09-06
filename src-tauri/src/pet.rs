use crate::world::{Point, Rect, Surface};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Perched,
    Flying,
    Greeting,
    Held,
}
#[derive(Clone, Copy, Debug)]
struct Anchor {
    owner: u64,
    t: f64,
}
#[derive(Clone, Copy)]
struct Flight {
    from: Point,
    to: Anchor,
    elapsed: f64,
    duration: f64,
}

pub struct Pet {
    pub position: Point,
    pub activity: Activity,
    pub flip_x: bool,
    pub paused: bool,
    pub visible: bool,
    pub speed: f64,
    anchor: Option<Anchor>,
    flight: Option<Flight>,
    timer: f64,
    clock: f64,
    rng: u64,
}
impl Pet {
    pub fn new(work: Rect) -> Self {
        Self {
            position: Point {
                x: work.x + work.width * 0.6,
                y: work.bottom(),
            },
            activity: Activity::Perched,
            flip_x: false,
            paused: false,
            visible: true,
            speed: 220.,
            anchor: Some(Anchor { owner: 0, t: 0.6 }),
            flight: None,
            timer: 4.,
            clock: 0.,
            rng: 0xA51C_974F,
        }
    }
    pub fn frame_index(&self) -> u32 {
        (self.clock
            * if self.activity == Activity::Flying {
                12.
            } else {
                4.
            }) as u32
            % 4
    }
    pub fn clip(&self) -> &'static str {
        if self.activity == Activity::Flying {
            "move"
        } else {
            "idle"
        }
    }
    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 10000) as f64 / 10000.
    }
    fn find(anchor: Anchor, surfaces: &[Surface], scale: f64) -> Option<Point> {
        surfaces
            .iter()
            .find(|s| s.owner == anchor.owner && s.fits(anchor.t, 36. * scale))
            .map(|s| s.point(anchor.t))
    }
    pub fn relocate(&mut self, surfaces: &[Surface], scale: f64, home: bool) {
        let candidates: Vec<_> = surfaces
            .iter()
            .filter(|s| {
                s.end - s.start >= 80. * scale
                    && (!home || s.owner == 0)
                    && (home || self.anchor.is_none_or(|a| a.owner != s.owner))
            })
            .collect();
        let candidates = if candidates.is_empty() {
            surfaces.iter().collect()
        } else {
            candidates
        };
        if candidates.is_empty() {
            return;
        }
        let s = candidates[(self.random() * candidates.len() as f64) as usize % candidates.len()];
        let to = Anchor {
            owner: s.owner,
            t: s.midpoint_t(),
        };
        let target = s.point(to.t);
        let distance = (target.x - self.position.x).hypot(target.y - self.position.y);
        self.flight = Some(Flight {
            from: self.position,
            to,
            elapsed: 0.,
            duration: (distance / (self.speed * scale)).clamp(0.5, 6.),
        });
        self.flip_x = target.x < self.position.x;
        self.activity = Activity::Flying;
        self.anchor = None;
    }
    pub fn greet(&mut self) {
        if self.activity != Activity::Flying && self.activity != Activity::Held {
            self.activity = Activity::Greeting;
            self.timer = 2.;
        }
    }
    pub fn grab(&mut self) {
        if !self.paused {
            self.flight = None;
            self.anchor = None;
            self.activity = Activity::Held;
        }
    }
    pub fn release(&mut self, surfaces: &[Surface], scale: f64) {
        if self.activity == Activity::Held {
            self.relocate(surfaces, scale, false);
        }
    }
    pub fn tick(
        &mut self,
        dt: f64,
        surfaces: &[Surface],
        work: Rect,
        scale: f64,
        cursor: Option<Point>,
    ) {
        if self.paused || !self.visible {
            return;
        }
        let dt = dt.clamp(0., 0.1);
        self.clock += dt;
        if self.activity == Activity::Held {
            if let Some(p) = cursor {
                self.position = Point {
                    x: p.x,
                    y: p.y + 35. * scale,
                };
            }
        } else if let Some(mut f) = self.flight {
            if let Some(target) = Self::find(f.to, surfaces, scale) {
                f.elapsed += dt;
                let t = (f.elapsed / f.duration).min(1.);
                let ease = t * t * (3. - 2. * t);
                let arc = (std::f64::consts::PI * t).sin() * 70. * scale;
                self.position = Point {
                    x: f.from.x + (target.x - f.from.x) * ease,
                    y: f.from.y + (target.y - f.from.y) * ease - arc,
                };
                if t >= 1. {
                    self.anchor = Some(f.to);
                    self.flight = None;
                    self.activity = Activity::Perched;
                    self.timer = 6. + self.random() * 10.;
                } else {
                    self.flight = Some(f);
                }
            } else {
                self.flight = None;
                self.relocate(surfaces, scale, false);
            }
        } else if let Some(anchor) = self.anchor {
            if let Some(p) = Self::find(anchor, surfaces, scale) {
                self.position = p;
                self.timer -= dt;
                if self.timer <= 0. {
                    if self.activity == Activity::Greeting {
                        self.activity = Activity::Perched;
                        self.timer = 4.;
                    } else {
                        self.relocate(surfaces, scale, false);
                    }
                }
            } else {
                self.relocate(surfaces, scale, false);
            }
        } else {
            self.relocate(surfaces, scale, true);
        }
        self.position.x = self.position.x.clamp(
            work.x + 40. * scale,
            (work.right() - 40. * scale).max(work.x + 40. * scale),
        );
        self.position.y = self.position.y.clamp(
            work.y + 112. * scale,
            work.bottom().max(work.y + 112. * scale),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{surfaces, WindowInfo};
    fn work() -> Rect {
        Rect {
            x: 0.,
            y: 0.,
            width: 1200.,
            height: 900.,
        }
    }
    fn window(x: f64, y: f64) -> WindowInfo {
        WindowInfo {
            id: 1,
            rect: Rect {
                x,
                y,
                width: 600.,
                height: 300.,
            },
        }
    }
    fn land(p: &mut Pet, s: &[Surface]) {
        p.relocate(s, 1., false);
        for _ in 0..500 {
            p.tick(0.02, s, work(), 1., None);
            if p.activity == Activity::Perched {
                return;
            }
        }
        panic!("bird did not land");
    }
    #[test]
    fn lands_and_rides_window() {
        let s = surfaces(&[window(50., 300.)], work(), 1.);
        let mut p = Pet::new(work());
        land(&mut p, &s);
        assert_eq!(p.position.y, 300.);
        let old = p.position.x;
        let moved = surfaces(&[window(150., 400.)], work(), 1.);
        p.tick(0.01, &moved, work(), 1., None);
        assert_eq!(
            p.position,
            Point {
                x: old + 100.,
                y: 400.
            }
        );
    }
    #[test]
    fn closed_window_recovers_to_floor() {
        let s = surfaces(&[window(50., 300.)], work(), 1.);
        let mut p = Pet::new(work());
        land(&mut p, &s);
        let floor = surfaces(&[], work(), 1.);
        p.tick(0.02, &floor, work(), 1., None);
        assert_eq!(p.activity, Activity::Flying);
        for _ in 0..300 {
            p.tick(0.02, &floor, work(), 1., None);
        }
        assert_eq!(p.position.y, 900.);
    }
    #[test]
    fn lost_destination_replans_midflight() {
        let mut p = Pet::new(work());
        let s = surfaces(&[window(50., 300.)], work(), 1.);
        p.relocate(&s, 1., false);
        let floor = surfaces(&[], work(), 1.);
        for _ in 0..300 {
            p.tick(0.02, &floor, work(), 1., None);
        }
        assert_eq!(p.activity, Activity::Perched);
        assert!(p.position.x.is_finite());
    }
    #[test]
    fn paused_pet_does_not_advance() {
        let mut p = Pet::new(work());
        p.paused = true;
        let old = p.position;
        let s = surfaces(&[], work(), 1.);
        p.tick(10., &s, work(), 1., None);
        assert_eq!(p.position, old);
        assert_eq!(p.frame_index(), 0);
    }
    #[test]
    fn dragged_pet_is_clamped_and_release_flies() {
        let mut p = Pet::new(work());
        let s = surfaces(&[], work(), 1.);
        p.grab();
        p.tick(
            0.02,
            &s,
            work(),
            1.,
            Some(Point {
                x: -9999.,
                y: -9999.,
            }),
        );
        assert!(work().contains(p.position));
        p.release(&s, 1.);
        assert_eq!(p.activity, Activity::Flying);
    }
    #[test]
    fn shrinking_or_occluded_perch_recovers() {
        let s = surfaces(&[window(50., 300.)], work(), 1.);
        let mut p = Pet::new(work());
        land(&mut p, &s);
        let s2 = surfaces(
            &[
                WindowInfo {
                    id: 2,
                    rect: Rect {
                        x: 300.,
                        y: 200.,
                        width: 200.,
                        height: 300.,
                    },
                },
                window(50., 300.),
            ],
            work(),
            1.,
        );
        p.tick(0.02, &s2, work(), 1., None);
        assert_eq!(p.activity, Activity::Flying);
    }
}
