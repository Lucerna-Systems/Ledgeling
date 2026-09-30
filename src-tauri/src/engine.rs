//! The headless core: world scanning, pet updates, packs and awareness with no
//! Tauri and no windowing. Everything interesting can therefore be tested by
//! replaying a window-layout timeline through a mock platform.
use crate::{
    awareness::{Awareness, AwarenessSummary},
    packs::{loader, manifest::Event, LocomotionClass, Pack},
    pet::{fsm::Rng, spawn_point, Pet, WorldView},
    platform::Platform,
    settings::{PetSetting, Settings},
    world::{
        self, virtual_desktop, MonitorInfo, Point, Rect, Surface, SurfaceChange, SurfaceGraph,
        SurfaceTracker, WindowInfo,
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Baseline window poll, bumped for a couple of seconds after any change.
const SCAN_INTERVAL: f64 = 0.5;
const FAST_SCAN_INTERVAL: f64 = 0.1;
const FAST_SCAN_WINDOW: f64 = 2.;
/// A window edge moving faster than this looks alarming to a pet on it.
const STARTLING_SPEED: f64 = 900.;
const MIN_SEGMENT: f64 = 48.;
/// Two pets on the same surface this close (logical pixels) notice each other.
const PET_NEARBY: f64 = 140.;

pub struct Engine {
    pub packs: HashMap<String, Arc<Pack>>,
    pub pets: Vec<Pet>,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
    pub surfaces: Vec<Surface>,
    pub graph: SurfaceGraph,
    pub awareness: Awareness,
    pub settings: Settings,
    pub suspended: bool,
    /// False while the screens cannot be read: a locked session or a sleeping
    /// display. Ledgeling suspends rather than pretending the desktop is empty.
    pub screens_available: bool,
    pub scan_ms: f64,
    pub scan_error: Option<String>,
    pub cursor: Option<Point>,
    /// Frontmost window, used only to let a nudging pet move somewhere the user
    /// is actually looking. Never its title, never its contents.
    pub attention: Option<u64>,
    pub sounds: Vec<(u64, String)>,
    tracker: SurfaceTracker,
    /// Pairs of pet ids currently close together, so `pet_nearby` fires once
    /// per meeting rather than every frame.
    neighbours: HashSet<(u64, u64)>,
    next_id: u64,
    clock: f64,
    last_scan: f64,
    fast_until: f64,
    last_suspend_check: f64,
    rng: Rng,
}

impl Engine {
    pub fn new(settings: Settings, packs: HashMap<String, Arc<Pack>>) -> Self {
        let awareness = Awareness::new(settings.awareness.clone(), settings.category_map());
        Self {
            packs,
            pets: Vec::new(),
            monitors: Vec::new(),
            windows: Vec::new(),
            surfaces: Vec::new(),
            graph: SurfaceGraph::default(),
            awareness,
            settings,
            suspended: false,
            screens_available: true,
            scan_ms: 0.,
            scan_error: None,
            cursor: None,
            attention: None,
            sounds: Vec::new(),
            tracker: SurfaceTracker::default(),
            neighbours: HashSet::new(),
            next_id: 1,
            clock: 0.,
            last_scan: f64::NEG_INFINITY,
            fast_until: 0.,
            last_suspend_check: f64::NEG_INFINITY,
            rng: Rng::new(0x5EED_1234),
        }
    }

    pub fn bounds(&self) -> Rect {
        virtual_desktop(&self.monitors)
    }

    pub fn monitor_for(&self, point: Point) -> Option<&MonitorInfo> {
        world::monitor_at(&self.monitors, point)
    }

    pub fn scale_at(&self, point: Point) -> f64 {
        self.monitor_for(point).map(|m| m.scale).unwrap_or(1.)
    }

    /// An empty list means the screens are not readable — the session is
    /// locked, or the display is asleep — not that the desktop has vanished.
    /// The last known geometry is kept so pets are where they were on return.
    pub fn set_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        self.screens_available = !monitors.is_empty();
        if !monitors.is_empty() && monitors != self.monitors {
            self.monitors = monitors;
            // Cached window rects were scaled for the previous pet screen
            // (platform::macos::engine_scale() bakes in whichever monitor was
            // current when they were captured). Rebuilding against the new
            // bounds before a fresh, correctly-scaled scan lands would mix two
            // different DPI scales into one surface graph on any screen swap
            // between differently-scaled displays — window edges land in the
            // wrong place relative to the monitor edges, breaking pathing and
            // stranding pets until something else forces a rescan. Dropping
            // the stale windows and forcing an immediate rescan means the
            // rebuild only ever sees one scale at a time.
            self.windows.clear();
            self.last_scan = f64::NEG_INFINITY;
            self.rebuild();
            self.rehome_strays();
        }
    }

    fn rebuild(&mut self) {
        self.surfaces = world::surfaces(&self.windows, &self.monitors, MIN_SEGMENT);
        self.graph = SurfaceGraph::build(&self.surfaces);
    }

    /// A screen swap strands pets where the new screen is not — the pick
    /// falls back to another display when its hardware is unplugged — and a
    /// pet beside or below the new bounds would otherwise park at the clamp
    /// margin with nothing under it and never recover. Out-of-bounds pets
    /// re-enter the new world the way a fresh spawn does.
    fn rehome_strays(&mut self) {
        if self.monitors.is_empty() {
            return;
        }
        let bounds = self.bounds();
        let scale = self.monitors.first().map(|m| m.scale).unwrap_or(1.);
        for pet in &mut self.pets {
            if bounds.contains(pet.position) || pet.is_held() {
                continue;
            }
            let pack = pet.pack.clone();
            let (position, perch) =
                spawn_point(&self.surfaces, &pack.manifest, bounds, scale, &mut self.rng);
            pet.rehome(position, perch);
        }
    }

    // ------------------------------------------------------------------ pets

    pub fn spawn(&mut self, pack_id: &str) -> Result<u64, String> {
        let pack = self
            .packs
            .get(pack_id)
            .cloned()
            .ok_or_else(|| format!("No pack with the id `{pack_id}` is installed."))?;
        let bounds = self.bounds();
        let scale = self.monitors.first().map(|m| m.scale).unwrap_or(1.);
        let (position, perch) =
            spawn_point(&self.surfaces, &pack.manifest, bounds, scale, &mut self.rng);
        let id = self.next_id;
        self.next_id += 1;
        let mut pet = Pet::new(id, pack, position);
        pet.perch = perch;
        pet.home = perch;
        if perch.is_some() {
            pet.motion = crate::pet::Motion::Settled;
        }
        self.pets.push(pet);
        Ok(id)
    }

    /// The bubble opened on one pet; there is only one bubble, so opening it
    /// elsewhere releases whoever had it.
    pub fn listen(&mut self, id: u64) {
        for pet in &mut self.pets {
            pet.listen(pet.id == id);
        }
    }

    /// The bubble closed. Named rather than blanket, because the close of an
    /// old bubble can be reported after a new one has already opened on
    /// another pet, and that pet must keep its place.
    pub fn stop_listening(&mut self, id: u64) {
        if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
            pet.listen(false);
        }
    }

    /// What a pet's task is doing. Pets without a task are left alone, so a
    /// task for one companion never stops another from wandering.
    pub fn set_work(&mut self, id: u64, work: Option<crate::pet::Attend>) {
        if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
            pet.work(work);
        }
    }

    pub fn despawn(&mut self, id: u64) -> bool {
        let before = self.pets.len();
        self.pets.retain(|p| p.id != id);
        self.pets.len() != before
    }

    /// Restore the pets recorded in settings, dropping any whose pack is gone.
    pub fn restore_pets(&mut self) -> Vec<String> {
        let mut missing = Vec::new();
        let wanted = self.settings.pets.clone();
        let bounds = self.bounds();
        for setting in wanted {
            match self.spawn(&setting.pack_id) {
                Ok(id) => {
                    // A saved spot only applies while it is still on the
                    // screens pets have: coordinates from a disconnected
                    // display would drop the pet outside the world it was
                    // restored into.
                    if let (Some(x), Some(y)) = (setting.x, setting.y) {
                        if bounds.contains(Point::new(x, y)) {
                            if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
                                pet.position = Point::new(x, y);
                            }
                        }
                    }
                }
                Err(_) => missing.push(setting.pack_id.clone()),
            }
        }
        if !missing.is_empty() {
            self.settings.pets.retain(|p| !missing.contains(&p.pack_id));
        }
        missing
    }

    pub fn remember_pets(&mut self) {
        self.settings.pets = self
            .pets
            .iter()
            .map(|pet| PetSetting {
                pack_id: pet.pack.manifest.id.clone(),
                x: pet.position.x.is_finite().then_some(pet.position.x),
                y: pet.position.y.is_finite().then_some(pet.position.y),
            })
            .collect();
    }

    // ------------------------------------------------------------------ tick

    pub fn tick(&mut self, dt: f64, platform: &dyn Platform) {
        self.clock += dt;
        if self.clock - self.last_suspend_check > 2. {
            self.last_suspend_check = self.clock;
            // Suspending in a fullscreen app is the difference between a
            // companion and something you uninstall during a film.
            self.suspended = platform.fullscreen_app_active().unwrap_or(false);
        }
        self.suspended = self.suspended || !self.screens_available || self.monitors.is_empty();
        if self.suspended || self.settings.paused || self.settings.hidden {
            return;
        }
        self.scan(platform);
        let mut events = self.awareness.tick(dt, platform);
        if self.awareness.nudge.tier() >= 2 {
            self.attention = self.windows.first().map(|w| w.id);
        } else {
            self.attention = None;
        }
        self.cursor = platform.cursor();
        let base = self.awareness.signals();
        let bounds = self.bounds();
        let held_by_mouse = platform.mouse_down();
        let mut pets = std::mem::take(&mut self.pets);
        for pet in &mut pets {
            if pet.is_held() && held_by_mouse == Some(false) {
                pet.release();
                events.push(Event::Released);
            }
            let view = WorldView {
                surfaces: &self.surfaces,
                graph: &self.graph,
                bounds,
                cursor: self.cursor,
                scale: self.scale_at(pet.position),
                speed_scale: self.settings.speed_scale.clamp(0.25, 3.),
                attention: self.attention,
            };
            let mut pet_events = events.clone();
            pet.tick(dt, &view, base, &mut pet_events);
            if let Some(sound) = pet.sound_request.take() {
                self.sounds.push((pet.id, sound));
            }
        }
        self.pets = pets;
        self.notice_neighbours();
    }

    /// Pets sharing a surface and standing close notice each other. Purely
    /// geometric: only ids and positions are compared.
    fn notice_neighbours(&mut self) {
        let mut near = HashSet::new();
        for (i, a) in self.pets.iter().enumerate() {
            for b in &self.pets[i + 1..] {
                let (Some(pa), Some(pb)) = (a.perch, b.perch) else {
                    continue;
                };
                if pa.key != pb.key || a.is_held() || b.is_held() {
                    continue;
                }
                let scale = self.scale_at(a.position);
                if a.position.distance(b.position) / scale < PET_NEARBY {
                    near.insert((a.id.min(b.id), a.id.max(b.id)));
                }
            }
        }
        for &(x, y) in near.difference(&self.neighbours) {
            for pet in self.pets.iter_mut().filter(|p| p.id == x || p.id == y) {
                pet.queue(Event::PetNearby);
            }
        }
        self.neighbours = near;
    }

    fn scan(&mut self, platform: &dyn Platform) {
        let interval = if self.clock < self.fast_until {
            FAST_SCAN_INTERVAL
        } else {
            SCAN_INTERVAL
        };
        if self.clock - self.last_scan < interval {
            return;
        }
        let elapsed = (self.clock - self.last_scan).min(5.);
        self.last_scan = self.clock;
        let started = std::time::Instant::now();
        match platform.enumerate_windows() {
            Ok(windows) => {
                if windows != self.windows {
                    self.fast_until = self.clock + FAST_SCAN_WINDOW;
                }
                self.windows = windows;
                self.scan_error = None;
            }
            Err(error) => {
                self.windows.clear();
                self.scan_error = Some(error);
            }
        }
        self.rebuild();
        self.scan_ms = started.elapsed().as_secs_f64() * 1000.;
        for change in self.tracker.diff(&self.surfaces, elapsed) {
            if let SurfaceChange::Moved { key, speed } = change {
                if speed > STARTLING_SPEED {
                    for pet in &mut self.pets {
                        if pet.perch.is_some_and(|p| p.key == key) {
                            pet.startle();
                        }
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------- reporting

    pub fn awareness_summary(&self) -> AwarenessSummary {
        let (energy, mood) = self
            .pets
            .first()
            .map(|p| (p.energy, p.mood))
            .unwrap_or((1., 0.7));
        self.awareness.summary(energy, mood)
    }

    pub fn perch_count(&self) -> usize {
        self.surfaces.iter().filter(|s| s.kind.is_window()).count()
    }

    /// Reload every pack from disk, keeping the pets that still have one.
    pub fn reload_packs(&mut self, packs_dir: &std::path::Path) -> Vec<String> {
        let mut packs = HashMap::new();
        let mut broken = Vec::new();
        for listing in loader::scan(packs_dir) {
            match loader::load(
                std::path::Path::new(&listing.summary.directory),
                listing.summary.builtin,
            ) {
                Ok(pack) => {
                    packs.insert(pack.manifest.id.clone(), Arc::new(pack));
                }
                Err(report) => broken.push(report.text()),
            }
        }
        self.packs = packs;
        let live: Vec<u64> = self
            .pets
            .iter()
            .filter(|p| !self.packs.contains_key(&p.pack.manifest.id))
            .map(|p| p.id)
            .collect();
        for id in live {
            self.despawn(id);
        }
        // Pets whose pack changed are respawned so edits appear immediately.
        let existing: Vec<(u64, String, Point)> = self
            .pets
            .iter()
            .map(|p| (p.id, p.pack.manifest.id.clone(), p.position))
            .collect();
        for (id, pack_id, position) in existing {
            if let Some(pack) = self.packs.get(&pack_id).cloned() {
                if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
                    pet.adopt(pack, position);
                }
            }
        }
        broken
    }

    pub fn class_of(&self, pack_id: &str) -> Option<LocomotionClass> {
        self.packs.get(pack_id).map(|p| p.manifest.locomotion.class)
    }
}
