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
    pub debug: Option<DebugOverlay>,
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
    visible: bool,
}

impl NativeSurface {
    pub fn new(windows: HashMap<u64, WebviewWindow>) -> Self {
        Self {
            windows,
            last: HashMap::new(),
            interactive: HashMap::new(),
            known: HashMap::new(),
            visible: true,
        }
    }
    pub fn replace(&mut self, windows: HashMap<u64, WebviewWindow>) {
        self.windows = windows;
        self.last.clear();
        self.interactive.clear();
        self.known.clear();
    }
    /// A freshly mounted overlay missed earlier spawn payloads; drop the
    /// record of what it holds so the next tick re-sends them.
    pub fn request_atlases(&mut self, monitor: u64) {
        self.known.remove(&monitor);
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
        self.last.insert(scene.monitor, scene.clone());
        Ok(())
    }
    fn set_interactive(&mut self, monitor: u64, interactive: bool) -> tauri::Result<()> {
        if self.interactive.get(&monitor) == Some(&interactive) {
            return Ok(());
        }
        if let Some(window) = self.windows.get(&monitor) {
            window.set_ignore_cursor_events(!interactive)?;
        }
        self.interactive.insert(monitor, interactive);
        Ok(())
    }
    fn set_visible(&mut self, visible: bool) -> tauri::Result<()> {
        if self.visible == visible {
            return Ok(());
        }
        for window in self.windows.values() {
            if visible {
                window.show()?;
            } else {
                window.hide()?;
            }
        }
        self.visible = visible;
        Ok(())
    }
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
    // Position within the drawn sprite, in texture pixels.
    let mut x = (cursor.x - rect.x) / texel;
    let mut y = (cursor.y - rect.y) / texel;
    let (w, h) = (mask.width as f64, mask.height as f64);
    let rotation = ((pet.rotation.round() as i64 % 360) + 360) % 360;
    (x, y) = match rotation {
        90 => (y, h - x),
        180 => (w - x, h - y),
        270 => (w - y, x),
        _ => (x, y),
    };
    if pet.flip_x {
        x = w - x;
    }
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
