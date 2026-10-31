//! Biomes: little scenes of decor props standing on the top edge of an app's
//! frontmost window. The user makes one per app; the props sway and squish
//! when the window moves, a few can be perched on, and a pet can call one home.
//!
//! What is saved lives in `Settings::biomes`; everything here besides those
//! types is runtime state rebuilt from the windows on screen.
pub mod apps;

use crate::{
    jelly::{Jelly, JellyParams, Spring1},
    packs::decor::{DecorPack, PropDef},
    platform::Platform,
    world::{Point, Rect, Surface, SurfaceKey, SurfaceKind, WindowInfo, PROP_OWNER_BASE},
};
use apps::AppResolver;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

pub const MAX_BIOMES: usize = 8;
pub const MAX_PROPS: usize = 16;
/// Owner ids per biome slot; `MAX_PROPS` must fit.
const SLOT_WIDTH: u64 = 64;
/// A ledge narrower than this, in logical pixels, is widened so a pet fits:
/// a squirrel on a bush overhangs it, which reads as sitting on it.
const MIN_LEDGE: f64 = 64.;
/// Sway in degrees per logical pixel the window has run ahead of its props.
const SWAY_PER_PIXEL: f64 = 0.35;
const MAX_SWAY: f64 = 25.;
const SQUASH_PER_PIXEL: f64 = 0.01;
const MAX_PROP_SQUASH: f64 = 0.3;
/// A window this much inside a bigger window of the same app is a popup of
/// it, never a biome's host.
const POPUP_INSIDE: f64 = 0.8;
/// Squash kick a prop gets when a pet lands on it.
const LANDING_SQUASH: f64 = 2.5;

/// One app's scene, as saved.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BiomeSetting {
    pub id: u64,
    /// Executable name, lowercased and without extension: the form the
    /// platform reports. The only process name Ledgeling ever stores, and
    /// only because the user made a biome for that app.
    pub app: String,
    pub props: Vec<PlacedProp>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlacedProp {
    /// Decor pack id and the prop's name in it.
    pub decor: String,
    pub prop: String,
    /// Where along the top edge, 0 at the left end to 1 at the right.
    pub x: f64,
    pub flip: bool,
}

/// An app name worth keeping: an executable stem, nothing that could be a
/// path or a sentence.
pub fn clean_app(app: &str) -> Option<String> {
    let app = app.trim().to_lowercase();
    let ok = !app.is_empty()
        && app.chars().count() <= 64
        && !app.contains(['/', '\\'])
        && !app.chars().any(char::is_control);
    ok.then_some(app)
}

pub fn sanitize(biomes: &mut Vec<BiomeSetting>) {
    let mut seen_apps = Vec::new();
    let mut seen_ids = Vec::new();
    biomes.retain_mut(|biome| {
        let Some(app) = clean_app(&biome.app) else {
            return false;
        };
        if seen_apps.contains(&app) || seen_ids.contains(&biome.id) {
            return false;
        }
        seen_apps.push(app.clone());
        seen_ids.push(biome.id);
        biome.app = app;
        biome.props.truncate(MAX_PROPS);
        for prop in &mut biome.props {
            prop.x = if prop.x.is_finite() {
                prop.x.clamp(0., 1.)
            } else {
                0.5
            };
        }
        true
    });
    biomes.truncate(MAX_BIOMES);
}

/// One visible prop this tick, in physical pixels.
pub struct Placed<'a> {
    pub slot: usize,
    pub index: usize,
    pub biome: u64,
    /// Where the prop stands on its window's top edge.
    pub base: Point,
    pub width: f64,
    pub height: f64,
    pub def: &'a PropDef,
    pub placed: &'a PlacedProp,
    pub monitor: u64,
}

/// What the overlay draws for one prop.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropFrame {
    pub key: String,
    pub decor: String,
    pub prop: String,
    pub frame_index: u32,
    /// Physical until the app converts it to overlay-local logical pixels.
    pub x: f64,
    pub y: f64,
    /// Logical pixels.
    pub width: f64,
    pub height: f64,
    pub anchor_x: f64,
    pub rotation: f64,
    pub squash_x: f64,
    pub squash_y: f64,
    pub flip: bool,
}

#[derive(Default)]
struct BiomeMotion {
    /// Smooths the window's 10 Hz steps so planted props glide with it; its
    /// lag is also what sets them swaying.
    follow: Jelly,
    last_anchor: Option<(u64, Point)>,
    props: Vec<PropMotion>,
}

#[derive(Clone, Copy, Default)]
struct PropMotion {
    sway: Spring1,
    squash: Spring1,
}

#[derive(Default)]
pub struct Biomes {
    resolver: AppResolver,
    /// The window each biome stands on, by biome id.
    pub hosts: HashMap<u64, u64>,
    motion: HashMap<u64, BiomeMotion>,
    /// Which prop each pet stood on last tick, to notice landings.
    perched: HashMap<u64, Option<SurfaceKey>>,
    clock: f64,
}

/// Props are planted: they follow the window closely and leave the wobble
/// to their own sway springs.
fn follow_params() -> JellyParams {
    JellyParams {
        stiffness: 900.,
        ..JellyParams::default().critically_damped()
    }
}

impl Biomes {
    /// After a window scan: which window each biome stands on. A biome lives
    /// on the frontmost window of its app.
    pub fn update_hosts(
        &mut self,
        biomes: &[BiomeSetting],
        windows: &[WindowInfo],
        platform: &dyn Platform,
    ) {
        self.resolver.set_biomes(biomes);
        let owners = self.resolver.resolve(windows, platform);
        self.hosts.clear();
        for window in windows {
            let Some(biome) = owners.get(&window.id) else {
                continue;
            };
            // A dropdown, menu or popover is a window of its own, in front of
            // the window it belongs to: the browser's address suggestions,
            // say. Standing on it would put the scene on the toolbar.
            let popup = windows.iter().any(|other| {
                other.id != window.id
                    && owners.get(&other.id) == Some(biome)
                    && area(other.rect) > area(window.rect)
                    && overlap(window.rect, other.rect) >= POPUP_INSIDE * area(window.rect)
            });
            if !popup {
                self.hosts.entry(*biome).or_insert(window.id);
            }
        }
    }

    /// Biomes switched off: nothing stands anywhere.
    pub fn clear(&mut self) {
        self.hosts.clear();
        self.motion.clear();
        self.perched.clear();
        self.resolver.set_biomes(&[]);
    }

    /// Every prop that can be seen: on a hosted biome, from an installed decor
    /// pack, and standing on a part of the top edge no other window covers.
    pub fn layout<'a>(
        &self,
        biomes: &'a [BiomeSetting],
        decor: &'a HashMap<String, Arc<DecorPack>>,
        surfaces: &[Surface],
        scale: f64,
    ) -> Vec<Placed<'a>> {
        let mut out = Vec::new();
        for (slot, biome) in biomes.iter().enumerate() {
            let Some(&host) = self.hosts.get(&biome.id) else {
                continue;
            };
            let tops: Vec<&Surface> = surfaces
                .iter()
                .filter(|s| s.owner == host && s.kind == SurfaceKind::WindowTop)
                .collect();
            let Some(edge) = tops.first() else {
                continue;
            };
            for (index, placed) in biome.props.iter().enumerate() {
                let Some(def) = decor
                    .get(&placed.decor)
                    .and_then(|pack| pack.manifest.props.get(&placed.prop))
                else {
                    continue;
                };
                let base = edge.span_start.lerp(edge.span_end, placed.x);
                let visible = tops
                    .iter()
                    .any(|s| base.x >= s.start.x.min(s.end.x) && base.x <= s.start.x.max(s.end.x));
                if visible {
                    out.push(Placed {
                        slot,
                        index,
                        biome: biome.id,
                        base,
                        width: def.width * scale,
                        height: def.height * scale,
                        def,
                        placed,
                        monitor: edge.monitor,
                    });
                }
            }
        }
        out
    }

    /// The ledges of perchable props, as surfaces for the graph.
    pub fn prop_surfaces(placed: &[Placed], scale: f64) -> Vec<Surface> {
        placed
            .iter()
            .filter_map(|p| {
                let ledge = p.def.perch?;
                let anchor = if p.placed.flip {
                    1. - p.def.anchor_x
                } else {
                    p.def.anchor_x
                };
                let (from, to) = if p.placed.flip {
                    (1. - ledge.to, 1. - ledge.from)
                } else {
                    (ledge.from, ledge.to)
                };
                let left = p.base.x - anchor * p.width;
                let y = p.base.y - p.height * (1. - ledge.y);
                let (mut x0, mut x1) = (left + from * p.width, left + to * p.width);
                let min = MIN_LEDGE * scale;
                if x1 - x0 < min {
                    let middle = (x0 + x1) / 2.;
                    (x0, x1) = (middle - min / 2., middle + min / 2.);
                }
                let (start, end) = (Point::new(x0, y), Point::new(x1, y));
                Some(Surface {
                    owner: owner_of(p.slot, p.index),
                    kind: SurfaceKind::PropTop,
                    index: 0,
                    start,
                    end,
                    span_start: start,
                    span_end: end,
                    monitor: p.monitor,
                })
            })
            .collect()
    }

    /// Where a pet whose home is this biome likes to be: the window's top
    /// edge and any prop it can sit on.
    pub fn home_keys(
        &self,
        biome: u64,
        biomes: &[BiomeSetting],
        surfaces: &[Surface],
    ) -> Vec<SurfaceKey> {
        let Some(&host) = self.hosts.get(&biome) else {
            return Vec::new();
        };
        let slot = biomes.iter().position(|b| b.id == biome);
        surfaces
            .iter()
            .filter(|s| {
                (s.owner == host && s.kind == SurfaceKind::WindowTop)
                    || (s.kind == SurfaceKind::PropTop && slot_of(s.owner) == slot)
            })
            .map(Surface::key)
            .collect()
    }

    /// Each pet's current perch, so a pet landing on a prop gives it a
    /// little squash.
    pub fn track_landings(
        &mut self,
        perches: &[(u64, Option<SurfaceKey>)],
        biomes: &[BiomeSetting],
        jelly: bool,
    ) {
        for (pet, perch) in perches {
            let before = self.perched.insert(*pet, *perch).flatten();
            match perch {
                Some(key) if key.kind == SurfaceKind::PropTop && Some(*key) != before && jelly => {
                    self.landed(key.owner, biomes);
                }
                _ => {}
            }
        }
        self.perched
            .retain(|pet, _| perches.iter().any(|(id, _)| id == pet));
    }

    /// Step every spring.
    pub fn tick(
        &mut self,
        dt: f64,
        biomes: &[BiomeSetting],
        decor: &HashMap<String, Arc<DecorPack>>,
        surfaces: &[Surface],
        jelly: bool,
        scale: f64,
    ) {
        self.clock += dt;
        self.motion.retain(|id, _| self.hosts.contains_key(id));
        for biome in biomes {
            let Some(&host) = self.hosts.get(&biome.id) else {
                continue;
            };
            let Some(edge) = surfaces
                .iter()
                .find(|s| s.owner == host && s.kind == SurfaceKind::WindowTop)
            else {
                continue;
            };
            let anchor = edge.span_start;
            let motion = self.motion.entry(biome.id).or_default();
            let moved = match motion.last_anchor {
                Some((last_host, last)) if last_host == host => {
                    Point::new(anchor.x - last.x, anchor.y - last.y)
                }
                _ => Point::default(),
            };
            motion.last_anchor = Some((host, anchor));
            motion.follow.follow(moved, dt, scale);
            motion.follow.step(dt, follow_params());
            motion
                .props
                .resize(biome.props.len(), PropMotion::default());
            let lag = motion.follow.offset.x;
            for (placed, prop) in biome.props.iter().zip(motion.props.iter_mut()) {
                if !jelly {
                    *prop = PropMotion::default();
                    continue;
                }
                let Some(def) = decor
                    .get(&placed.decor)
                    .and_then(|pack| pack.manifest.props.get(&placed.prop))
                else {
                    continue;
                };
                let sway = def.sway;
                let lean =
                    (lag.x / scale * SWAY_PER_PIXEL * sway.softness).clamp(-MAX_SWAY, MAX_SWAY);
                let squash = (-lag.y / scale * SQUASH_PER_PIXEL * sway.softness)
                    .clamp(-MAX_PROP_SQUASH, MAX_PROP_SQUASH);
                prop.sway.step(dt, sway.stiffness, sway.damping, lean);
                prop.squash
                    .step(dt, sway.stiffness * 2., sway.damping, squash);
                prop.squash.x = prop.squash.x.clamp(-MAX_PROP_SQUASH, MAX_PROP_SQUASH);
            }
        }
    }

    fn landed(&mut self, owner: u64, biomes: &[BiomeSetting]) {
        let (Some(slot), Some(index)) = (slot_of(owner), index_of(owner)) else {
            return;
        };
        let Some(biome) = biomes.get(slot) else {
            return;
        };
        if let Some(prop) = self
            .motion
            .get_mut(&biome.id)
            .and_then(|m| m.props.get_mut(index))
        {
            prop.squash.v += LANDING_SQUASH;
        }
    }

    /// Something is still swinging, so the loop should run at full rate.
    pub fn is_active(&self) -> bool {
        self.motion.values().any(|m| {
            m.follow.is_active()
                || m.props
                    .iter()
                    .any(|p| !p.sway.is_at_rest(0.2) || !p.squash.is_at_rest(0.005))
        })
    }

    pub fn frames(&self, placed: &[Placed]) -> Vec<PropFrame> {
        placed
            .iter()
            .map(|p| {
                let motion = self.motion.get(&p.biome);
                let lag = motion.map(|m| m.follow.offset.x).unwrap_or_default();
                let spring = motion
                    .and_then(|m| m.props.get(p.index))
                    .copied()
                    .unwrap_or_default();
                let def = p.def;
                let frame_index = if def.frames > 1 && def.fps > 0. {
                    (self.clock * def.fps) as u32 % def.frames
                } else {
                    0
                };
                PropFrame {
                    key: format!("{}:{}", p.biome, p.index),
                    decor: p.placed.decor.clone(),
                    prop: p.placed.prop.clone(),
                    frame_index,
                    x: p.base.x + lag.x,
                    y: p.base.y + lag.y,
                    width: def.width,
                    height: def.height,
                    anchor_x: def.anchor_x,
                    rotation: spring.sway.x,
                    squash_x: 1. + spring.squash.x,
                    squash_y: 1. - spring.squash.x,
                    flip: p.placed.flip,
                }
            })
            .collect()
    }
}

fn area(r: Rect) -> f64 {
    r.width.max(0.) * r.height.max(0.)
}

fn overlap(a: Rect, b: Rect) -> f64 {
    let w = a.right().min(b.right()) - a.x.max(b.x);
    let h = a.bottom().min(b.bottom()) - a.y.max(b.y);
    w.max(0.) * h.max(0.)
}

fn owner_of(slot: usize, index: usize) -> u64 {
    PROP_OWNER_BASE + slot as u64 * SLOT_WIDTH + index as u64
}

fn slot_of(owner: u64) -> Option<usize> {
    owner
        .checked_sub(PROP_OWNER_BASE)
        .map(|o| (o / SLOT_WIDTH) as usize)
}

fn index_of(owner: u64) -> Option<usize> {
    owner
        .checked_sub(PROP_OWNER_BASE)
        .map(|o| (o % SLOT_WIDTH) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_one_clean_biome_per_app_within_limits() {
        let prop = |x: f64| PlacedProp {
            decor: "d".into(),
            prop: "p".into(),
            x,
            flip: false,
        };
        let mut biomes = vec![
            BiomeSetting {
                id: 1,
                app: " Safari ".into(),
                props: (0..20).map(|i| prop(i as f64 / 10.)).collect(),
            },
            BiomeSetting {
                id: 2,
                app: "safari".into(),
                props: vec![],
            },
            BiomeSetting {
                id: 3,
                app: "/Applications/Mail.app".into(),
                props: vec![],
            },
            BiomeSetting {
                id: 4,
                app: "slack".into(),
                props: vec![prop(f64::NAN)],
            },
        ];
        sanitize(&mut biomes);
        assert_eq!(
            biomes.iter().map(|b| b.app.as_str()).collect::<Vec<_>>(),
            ["safari", "slack"]
        );
        assert_eq!(biomes[0].props.len(), MAX_PROPS);
        assert!(biomes[0].props.iter().all(|p| (0. ..=1.).contains(&p.x)));
        assert_eq!(biomes[1].props[0].x, 0.5);
    }

    #[test]
    fn prop_owners_round_trip_and_stay_clear_of_monitors() {
        let owner = owner_of(MAX_BIOMES - 1, MAX_PROPS - 1);
        assert!(owner > crate::world::MONITOR_OWNER_BASE + 1_000_000);
        assert_eq!(slot_of(owner), Some(MAX_BIOMES - 1));
        assert_eq!(index_of(owner), Some(MAX_PROPS - 1));
        assert_eq!(slot_of(42), None);
    }
}
