//! The renderer boundary. One transparent overlay on the single pet screen:
//! no window is moved per frame and the overlay always uses its own
//! monitor's scale, so mixed DPI stays correct.
use crate::{
    pet::{Frame, Pet},
    world::{MonitorInfo, Point, Rect},
};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{Emitter, WebviewWindow};

/// What one overlay is told to draw this tick.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    pub monitor: u64,
    pub frames: Vec<Frame>,
    /// Biome props, drawn beneath the pets.
    pub props: Vec<crate::biome::PropFrame>,
    /// Decorate mode, when it is on: the overlay takes every click.
    pub edit: Option<crate::biome::edit::EditView>,
    pub debug: Option<DebugOverlay>,
}

/// One decor pack's images, sent once per overlay before its props are drawn.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorPayload {
    pub decor: String,
    pub atlas: std::collections::BTreeMap<String, crate::packs::decor::PropAtlas>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugOverlay {
    /// Logical, monitor-local coordinates, ready to draw.
    pub surfaces: Vec<[f64; 4]>,
    pub jumps: Vec<[f64; 4]>,
    pub paths: Vec<Vec<[f64; 2]>>,
    pub labels: Vec<Label>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub x: f64,
    pub y: f64,
    pub text: String,
}

/// Told to an overlay once per pet, so the renderer has the sprite images it
/// needs before the first frame referring to them arrives.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnPayload {
    pub pet_id: u64,
    pub pack_id: String,
    pub name: String,
    pub atlas: std::collections::BTreeMap<String, crate::packs::ClipAtlas>,
}

pub trait PetSurface: Send {
    fn render(&mut self, scene: &Scene) -> tauri::Result<()>;
    fn set_interactive(&mut self, monitor: u64, interactive: bool) -> tauri::Result<()>;
    fn set_visible(&mut self, visible: bool) -> tauri::Result<()>;
}

pub struct NativeSurface {
    windows: HashMap<u64, WebviewWindow>,
    last: HashMap<u64, Scene>,
    interactive: HashMap<u64, bool>,
    /// Per overlay, the pets it has sprite data for and which pack build it
    /// came from, so a live-reloaded pack re-sends its images.
    known: HashMap<u64, HashMap<u64, usize>>,
    /// Per overlay, the decor packs whose images it already has.
    decor_known: HashMap<u64, std::collections::HashSet<String>>,
    visible: bool,
}

impl NativeSurface {
    pub fn new(windows: HashMap<u64, WebviewWindow>) -> Self {
        Self {
            windows,
            last: HashMap::new(),
            interactive: HashMap::new(),
            known: HashMap::new(),
            decor_known: HashMap::new(),
            visible: true,
        }
    }
    pub fn replace(&mut self, windows: HashMap<u64, WebviewWindow>) {
        self.windows = windows;
        self.last.clear();
        self.interactive.clear();
        self.known.clear();
        self.decor_known.clear();
    }
    /// A freshly mounted overlay missed earlier spawn payloads; drop the
    /// record of what it holds so the next tick re-sends them.
    pub fn request_atlases(&mut self, monitor: u64) {
        self.known.remove(&monitor);
        self.decor_known.remove(&monitor);
    }
    pub fn windows(&self) -> impl Iterator<Item = (&u64, &WebviewWindow)> {
        self.windows.iter()
    }
    /// Whether the renderer already knows the overlay for a monitor — the
    /// update loop uses this to adopt a window that exists but was never
    /// handed over (a timed-out build, say).
    pub fn has(&self, monitor: u64) -> bool {
        self.windows.contains_key(&monitor)
    }
}

impl NativeSurface {
    /// Which pets on this monitor still need their sprite atlas sent. Called
    /// while the engine lock is held; the sending itself is not, because
    /// emitting hops to the main thread and must never wait on the lock.
    pub fn missing_atlases(&self, monitor: u64, pets: &[(u64, usize)]) -> Vec<u64> {
        let known = self.known.get(&monitor);
        pets.iter()
            .filter(|(id, build)| known.and_then(|k| k.get(id)) != Some(build))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Which decor packs this overlay still needs images for.
    pub fn missing_decor(&self, monitor: u64, decor: &[String]) -> Vec<String> {
        let known = self.decor_known.get(&monitor);
        decor
            .iter()
            .filter(|id| !known.is_some_and(|k| k.contains(*id)))
            .cloned()
            .collect()
    }

    pub fn sync_decor(&mut self, monitor: u64, payloads: Vec<DecorPayload>) -> tauri::Result<()> {
        let Some(window) = self.windows.get(&monitor) else {
            return Ok(());
        };
        let known = self.decor_known.entry(monitor).or_default();
        for payload in payloads {
            window.emit("decor:spawn", &payload)?;
            known.insert(payload.decor);
        }
        Ok(())
    }

    /// Emit `pet:spawn` / `pet:despawn` so each overlay holds exactly the
    /// sprite atlases for the pets currently standing on its monitor.
    pub fn sync(
        &mut self,
        monitor: u64,
        present: &[(u64, usize)],
        payloads: Vec<SpawnPayload>,
    ) -> tauri::Result<()> {
        let Some(window) = self.windows.get(&monitor) else {
            return Ok(());
        };
        let known = self.known.entry(monitor).or_default();
        for payload in payloads {
            let build = present
                .iter()
                .find(|(id, _)| *id == payload.pet_id)
                .map(|(_, build)| *build);
            window.emit("pet:spawn", &payload)?;
            if let Some(build) = build {
                known.insert(payload.pet_id, build);
            }
        }
        let gone: Vec<u64> = known
            .keys()
            .copied()
            .filter(|id| !present.iter().any(|(present, _)| present == id))
            .collect();
        for id in gone {
            known.remove(&id);
            window.emit("pet:despawn", id)?;
        }
        Ok(())
    }
}

impl PetSurface for NativeSurface {
    fn render(&mut self, scene: &Scene) -> tauri::Result<()> {
        let Some(window) = self.windows.get(&scene.monitor) else {
            return Ok(());
        };
        // Only emit when something actually changed: idle pets cost nothing.
        if self.last.get(&scene.monitor) == Some(scene) {
            return Ok(());
        }
        window.emit("pet:scene", scene)?;
        #[cfg(target_os = "linux")]
        if crate::platform::layer::active() {
            crate::platform::layer::set_input_boxes(window, input_boxes(&scene.frames));
        }
        self.last.insert(scene.monitor, scene.clone());
        Ok(())
    }
    fn set_interactive(&mut self, monitor: u64, interactive: bool) -> tauri::Result<()> {
        if self.interactive.get(&monitor) == Some(&interactive) {
            return Ok(());
        }
        if let Some(window) = self.windows.get(&monitor) {
            crate::platform::pet_ui::click_through(window, !interactive)?;
        }
        self.interactive.insert(monitor, interactive);
        Ok(())
    }
    fn set_visible(&mut self, visible: bool) -> tauri::Result<()> {
        if self.visible == visible {
            return Ok(());
        }
        for (monitor, window) in &self.windows {
            if visible {
                window.show()?;
                let interactive = self.interactive.get(monitor).copied().unwrap_or(false);
                crate::platform::pet_ui::restore_overlay(window, !interactive);
            } else {
                window.hide()?;
            }
        }
        self.visible = visible;
        Ok(())
    }
}

/// Where a Wayland layer overlay should catch the pointer: each sprite's
/// box, rotated, padded and snapped outward to a coarse grid in the overlay's
/// logical pixels. The grid keeps a walking pet from changing the region on
/// every frame; the padding means a pet stepping away from a resting pointer
/// still reports it leaving. Transparent margins inside a box catch clicks the
/// alpha hit test then ignores, which is the cost of not knowing the pointer.
pub fn input_boxes(frames: &[crate::pet::Frame]) -> Vec<[i32; 4]> {
    const GRID: f64 = 8.;
    const PAD: f64 = 4.;
    frames
        .iter()
        .filter(|f| f.opacity > 0. && f.width > 0. && f.height > 0.)
        .map(|f| {
            let (sin, cos) = (f.rotation + f.lean).to_radians().sin_cos();
            let (w, h) = (f.width * f.squash_x, f.height * f.squash_y);
            let (ax, ay) = (w * f.anchor_x, h * f.anchor_y);
            let corners = [(-ax, -ay), (w - ax, -ay), (-ax, h - ay), (w - ax, h - ay)]
                .map(|(x, y)| (x * cos - y * sin, x * sin + y * cos));
            let left = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
            let right = corners
                .iter()
                .map(|c| c.0)
                .fold(f64::NEG_INFINITY, f64::max);
            let top = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
            let bottom = corners
                .iter()
                .map(|c| c.1)
                .fold(f64::NEG_INFINITY, f64::max);
            let x0 = ((f.x + left - PAD) / GRID).floor() * GRID;
            let y0 = ((f.y + top - PAD) / GRID).floor() * GRID;
            let x1 = ((f.x + right + PAD) / GRID).ceil() * GRID;
            let y1 = ((f.y + bottom + PAD) / GRID).ceil() * GRID;
            [x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32]
        })
        .collect()
}

/// Is the cursor over an opaque pixel of this pet? Empty space around the
/// sprite must stay clickable, so this reads the pack's real alpha channel.
pub fn hit_test(pet: &Pet, cursor: Point, scale: f64) -> bool {
    let rect = pet.sprite_rect(scale);
    if !rect.contains(cursor) {
        return false;
    }
    let Some(strip) = pet.pack.strip(pet.clip()) else {
        return true;
    };
    let Some(mask) = strip.masks.get(pet.frame_index() as usize) else {
        return true;
    };
    let sprite = pet.manifest().sprite;
    let texel = (sprite.scale * scale).max(0.01);
    let local = pet.sprite_local(cursor, scale);
    let (x, y) = (local.x / texel, local.y / texel);
    if x < 0. || y < 0. {
        return false;
    }
    mask.opaque(x as u32, y as u32)
}

/// Monitor-local logical coordinates, which is what an overlay draws in.
pub fn to_local(point: Point, monitor: &MonitorInfo) -> Point {
    Point::new(
        (point.x - monitor.bounds.x) / monitor.scale,
        (point.y - monitor.bounds.y) / monitor.scale,
    )
}

pub fn local_rect(rect: Rect, monitor: &MonitorInfo) -> Rect {
    Rect::new(
        (rect.x - monitor.bounds.x) / monitor.scale,
        (rect.y - monitor.bounds.y) / monitor.scale,
        rect.width / monitor.scale,
        rect.height / monitor.scale,
    )
}

#[cfg(test)]
mod tests {
    use super::input_boxes;
    use crate::pet::Frame;

    fn frame(x: f64, y: f64, rotation: f64) -> Frame {
        Frame {
            pet_id: 1,
            clip: "idle".into(),
            frame_index: 0,
            x,
            y,
            flip_x: false,
            rotation,
            opacity: 1.,
            width: 32.,
            height: 24.,
            anchor_x: 0.5,
            anchor_y: 1.,
            squash_x: 1.,
            squash_y: 1.,
            lean: 0.,
        }
    }

    #[test]
    fn input_boxes_cover_the_sprite_snapped_outward() {
        let [x, y, width, height] = input_boxes(&[frame(100., 200., 0.)])[0];
        // The sprite spans 84..116 × 176..200; padded and snapped to 8 px.
        assert_eq!([x, y, width, height], [80, 168, 40, 40]);
    }

    #[test]
    fn small_moves_keep_the_same_input_box() {
        assert_eq!(
            input_boxes(&[frame(102., 200., 0.)]),
            input_boxes(&[frame(103.5, 200., 0.)])
        );
    }

    #[test]
    fn a_pet_on_a_wall_gets_a_rotated_box() {
        let [_, _, width, height] = input_boxes(&[frame(100., 200., 90.)])[0];
        assert!(height > width, "{width}×{height}");
    }

    #[test]
    fn invisible_pets_catch_nothing() {
        let mut faded = frame(100., 200., 0.);
        faded.opacity = 0.;
        assert!(input_boxes(&[faded]).is_empty());
    }
}
