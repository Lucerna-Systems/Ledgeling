//! The headless core: world scanning, pet updates, packs and awareness with no
//! Tauri and no windowing. Everything interesting can therefore be tested by
//! replaying a window-layout timeline through a mock platform.
use crate::{
    awareness::{Awareness, AwarenessSummary},
    biome::{
        edit::{self, Edit, EditView},
        Biomes, PropFrame,
    },
    packs::{
        decor::{self, DecorPack},
        loader, LocomotionClass, Pack,
    },
    pet::{fsm::Rng, spawn_point, Bond, Peer, Pet, WorldView},
    platform::{Platform, ResizeAccess},
    room::{Need, Room},
    settings::{clean_name, PetSetting, Settings},
    world::{
        self, virtual_desktop, MonitorInfo, Point, Rect, Surface, SurfaceChange, SurfaceGraph,
        SurfaceKey, SurfaceTracker, WindowInfo,
    },
};
use chrono::NaiveDate;
use std::{collections::HashMap, sync::Arc};

/// Baseline window poll, bumped for a couple of seconds after any change.
const SCAN_INTERVAL: f64 = 0.5;
const FAST_SCAN_INTERVAL: f64 = 0.1;
const FAST_SCAN_WINDOW: f64 = 2.;
/// A window edge moving faster than this looks alarming to a pet on it.
const STARTLING_SPEED: f64 = 900.;
const MIN_SEGMENT: f64 = 48.;
/// Room above a window's top edge, logical pixels, below which a pet standing
/// there would be cut off by the top of the screen.
const CRAMPED_HEIGHT: f64 = 56.;

/// One change to the biome being decorated.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Decoration {
    Place { decor: String, prop: String, x: f64 },
    Move { index: usize, x: f64 },
    Flip { index: usize },
    Remove { index: usize },
}

pub struct Engine {
    pub packs: HashMap<String, Arc<Pack>>,
    /// Decor packs by id: the props biomes are built from.
    pub decor: HashMap<String, Arc<DecorPack>>,
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
    /// The OS asks for reduced motion; treated as squish and wobble off.
    pub reduced_motion: bool,
    pub biomes: Biomes,
    /// Decorate mode, while the user is building a biome.
    pub edit: Option<Edit>,
    /// Why the last Decorate step did not happen, in plain words.
    pub edit_error: Option<String>,
    /// A click the overlay reported while picking a window, served on the
    /// next tick, where the platform is at hand.
    pending_pick: Option<Point>,
    pub room: Room,
    /// Whether windows may be moved, as the platform last said. Only asked
    /// while "make room at the top" is on.
    pub resize_access: ResizeAccess,
    /// The user asked to be shown the system's permission prompt.
    access_requested: bool,
    /// Windows with no room above them that could not be given any.
    pub cramped: Vec<u64>,
    tracker: SurfaceTracker,
    next_id: u64,
    clock: f64,
    last_scan: f64,
    fast_until: f64,
    last_suspend_check: f64,
    rng: Rng,
    pending_welcome: bool,
    /// Set when the engine changed something that must reach `settings.json`.
    settings_changed: bool,
    today: NaiveDate,
}

impl Engine {
    pub fn new(settings: Settings, packs: HashMap<String, Arc<Pack>>) -> Self {
        let awareness = Awareness::new(settings.awareness.clone(), settings.category_map());
        Self {
            packs,
            decor: HashMap::new(),
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
            reduced_motion: false,
            biomes: Biomes::default(),
            edit: None,
            edit_error: None,
            pending_pick: None,
            room: Room::default(),
            resize_access: ResizeAccess::Unsupported,
            access_requested: false,
            cramped: Vec::new(),
            tracker: SurfaceTracker::default(),
            next_id: 1,
            clock: 0.,
            last_scan: f64::NEG_INFINITY,
            fast_until: 0.,
            last_suspend_check: f64::NEG_INFINITY,
            rng: Rng::new(0x5EED_1234),
            pending_welcome: false,
            settings_changed: false,
            today: NaiveDate::default(),
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
        self.cramped = self.find_cramped();
        if self.settings.effects.biomes {
            let scale = self.pet_scale();
            let placed = self.biomes.layout(
                &self.settings.biomes,
                &self.decor,
                &self.surfaces,
                scale,
                &self.cramped,
            );
            let ledges = Biomes::prop_surfaces(&placed, scale);
            self.surfaces.extend(ledges);
        }
        self.graph = SurfaceGraph::build(&self.surfaces);
    }

    /// Windows against the top of the screen whose top edge cannot be given
    /// room. Only while the user wants room made: without it, nothing changes.
    fn find_cramped(&self) -> Vec<u64> {
        if !self.settings.effects.make_room {
            return Vec::new();
        }
        let Some(monitor) = self.monitors.first() else {
            return Vec::new();
        };
        let room_for = CRAMPED_HEIGHT * monitor.scale;
        self.windows
            .iter()
            .filter(|w| w.rect.y - monitor.work.y < room_for)
            .filter(|w| self.resize_access != ResizeAccess::Granted || self.room.refused(w.id))
            .map(|w| w.id)
            .collect()
    }

    /// Make room above windows that need it, or put them back.
    fn make_room(&mut self, platform: &dyn Platform) {
        if !self.settings.effects.make_room {
            self.room.restore_all(&self.windows, platform);
            return;
        }
        let Some(monitor) = self.monitors.first().copied() else {
            return;
        };
        let mut needs: Vec<Need> = Vec::new();
        let mut need =
            |window: u64, height: f64| match needs.iter_mut().find(|n| n.window == window) {
                Some(n) => n.height = n.height.max(height),
                None => needs.push(Need { window, height }),
            };
        if self.settings.effects.biomes {
            for biome in &self.settings.biomes {
                if let Some(&host) = self.biomes.hosts.get(&biome.id) {
                    need(host, Biomes::height(biome, &self.decor, monitor.scale));
                }
            }
        }
        for pet in &self.pets {
            if let Some(perch) = pet
                .perch
                .filter(|p| p.key.kind == world::SurfaceKind::WindowTop)
            {
                let sprite = pet.manifest().sprite;
                need(
                    perch.key.owner,
                    sprite.height * sprite.scale * monitor.scale,
                );
            }
        }
        needs.retain(|n| n.height > 0.);
        self.room.update(
            self.clock,
            &self.windows,
            monitor.work.y,
            monitor.scale,
            &needs,
            self.resize_access,
            platform.mouse_down() == Some(true),
            platform,
        );
    }

    /// Ledgeling is quitting: windows it moved go back where they were.
    pub fn release_windows(&mut self, platform: &dyn Platform) {
        self.room.restore_all(&self.windows, platform);
    }

    /// Show the system's permission prompt on the next tick.
    pub fn request_resize_access(&mut self) {
        self.access_requested = true;
    }

    /// Pets live on one screen, so one scale serves every biome.
    fn pet_scale(&self) -> f64 {
        self.monitors.first().map_or(1., |m| m.scale)
    }

    /// What the overlay draws for every visible biome prop, positions still
    /// in physical pixels.
    pub fn prop_frames(&self) -> Vec<PropFrame> {
        if !self.settings.effects.biomes {
            return Vec::new();
        }
        let placed = self.biomes.layout(
            &self.settings.biomes,
            &self.decor,
            &self.surfaces,
            self.pet_scale(),
            &self.cramped,
        );
        self.biomes.frames(&placed)
    }

    /// Start Decorate mode: the next click on a window picks it.
    pub fn start_decorating(&mut self) {
        self.edit = Some(Edit::Picking);
        self.edit_error = None;
        self.pending_pick = None;
    }

    pub fn end_decorating(&mut self) {
        self.edit = None;
        self.edit_error = None;
        self.pending_pick = None;
    }

    /// The user clicked here while picking a window, in physical pixels.
    pub fn request_pick(&mut self, point: Point) {
        if self.edit == Some(Edit::Picking) {
            self.pending_pick = Some(point);
        }
    }

    /// Find the app of the clicked window and open its biome, making one if
    /// needed. Reading the app's name is what the user asked for here.
    fn serve_pick(&mut self, platform: &dyn Platform) {
        let Some(point) = self.pending_pick.take() else {
            return;
        };
        let Some(window) = self
            .windows
            .iter()
            .find(|w| !w.is_minimized && w.rect.contains(point))
        else {
            self.edit_error =
                Some("There's no window there. Click the window you want to decorate.".into());
            return;
        };
        let Some(app) = crate::biome::apps::app_of(window, platform) else {
            self.edit_error = Some(
                "This desktop doesn't say which app owns that window, so it can't hold a biome."
                    .into(),
            );
            return;
        };
        match edit::adopt(&mut self.settings.biomes, &app) {
            Ok(biome) => {
                self.edit = Some(Edit::Editing { biome });
                self.edit_error = None;
                self.settings.effects.biomes = true;
                self.settings_changed = true;
                self.biomes
                    .update_hosts(&self.settings.biomes, &self.windows, platform);
                self.rebuild();
            }
            Err(error) => self.edit_error = Some(error),
        }
    }

    /// Apply one Decorate change to the biome being edited.
    pub fn decorate(&mut self, change: Decoration) -> Result<(), String> {
        let Some(Edit::Editing { biome }) = self.edit else {
            return Err("Decorate mode isn't on.".into());
        };
        let biomes = &mut self.settings.biomes;
        let result = match change {
            Decoration::Place { decor, prop, x } => {
                edit::place(biomes, &self.decor, biome, &decor, &prop, x).map(|_| ())
            }
            Decoration::Move { index, x } => edit::move_to(biomes, biome, index, x),
            Decoration::Flip { index } => edit::flip(biomes, biome, index),
            Decoration::Remove { index } => edit::remove(biomes, biome, index),
        };
        self.edit_error = result.as_ref().err().cloned();
        if result.is_ok() {
            self.settings_changed = true;
            self.rebuild();
        }
        result
    }

    /// What the overlay shows while decorating, positions in physical pixels.
    pub fn edit_view(&self) -> Option<EditView> {
        let edit = self.edit?;
        let palette = edit::palette(&self.decor);
        let mut view = EditView {
            picking: edit == Edit::Picking,
            biome: None,
            app: None,
            edge_x: 0.,
            edge_y: 0.,
            edge_width: 0.,
            props: Vec::new(),
            palette,
            error: self.edit_error.clone(),
        };
        if let Edit::Editing { biome } = edit {
            let setting = self.settings.biomes.iter().find(|b| b.id == biome)?;
            let host = self.biomes.hosts.get(&biome)?;
            let edge = self
                .surfaces
                .iter()
                .find(|s| s.owner == *host && s.kind == world::SurfaceKind::WindowTop)?;
            view.biome = Some(biome);
            view.app = Some(setting.app.clone());
            view.edge_x = edge.span_start.x;
            view.edge_y = edge.span_start.y;
            view.edge_width = edge.span_end.x - edge.span_start.x;
            view.props = setting.props.clone();
        }
        Some(view)
    }

    /// Make a biome this pet's home, or none.
    pub fn set_pet_home(&mut self, pet: u64, biome: Option<u64>) {
        let biome = biome.filter(|id| self.settings.biomes.iter().any(|b| b.id == *id));
        if let Some(pet) = self.pets.iter_mut().find(|p| p.id == pet) {
            pet.home_biome = biome;
            self.settings_changed = true;
        }
    }

    pub fn delete_biome(&mut self, id: u64) {
        self.settings.biomes.retain(|b| b.id != id);
        for pet in &mut self.pets {
            if pet.home_biome == Some(id) {
                pet.home_biome = None;
            }
        }
        self.settings_changed = true;
        self.rebuild();
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
        pet.bond = Bond::new(self.today);
        pet.perch = perch;
        pet.home = perch;
        if perch.is_some() {
            pet.motion = crate::pet::Motion::Settled;
        }
        self.pets.push(pet);
        Ok(id)
    }

    /// Tell the engine what day it is. Kept out of the pets so bond logic needs
    /// no clock: a later day than a pet was last seen is one more day together.
    pub fn set_today(&mut self, day: NaiveDate) {
        self.today = day;
        for pet in &mut self.pets {
            pet.roll_day(day);
        }
    }

    pub fn today(&self) -> NaiveDate {
        self.today
    }

    /// Rename a pet; an empty name goes back to the pack's own.
    pub fn rename_pet(&mut self, id: u64, name: &str) -> bool {
        match self.pets.iter_mut().find(|p| p.id == id) {
            Some(pet) => {
                pet.bond.name = clean_name(name);
                true
            }
            None => false,
        }
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
                    if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
                        pet.bond = bond_from(&setting, self.today);
                        pet.roll_day(self.today);
                        pet.home_biome = setting.home_biome;
                    }
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
                    // A folder that was moved or deleted since is forgotten
                    // rather than failing every action the pet is asked for.
                    if let Some(folder) =
                        setting.folder.filter(|f| std::path::Path::new(f).is_dir())
                    {
                        self.set_pet_folder(id, Some(folder));
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

    /// Ask for the first-run welcome: a pet dropped onto the window the user is
    /// looking at. Only a fresh install qualifies, and only once.
    pub fn request_welcome(&mut self) {
        if !self.settings.welcomed && self.pets.is_empty() {
            self.pending_welcome = true;
        }
    }

    /// Whether a welcome is still waiting for a window to land on.
    pub fn welcome_pending(&self) -> bool {
        self.pending_welcome
    }

    /// Whether the engine changed settings since this was last asked. The app
    /// saves when it is true, so a first run is remembered even if the app is
    /// killed before a clean quit.
    pub fn take_settings_changed(&mut self) -> bool {
        std::mem::take(&mut self.settings_changed)
    }

    fn welcome(&mut self) {
        self.pending_welcome = false;
        self.settings.welcomed = true;
        self.settings_changed = true;
        let preferred = "com.perch.squirrel-red";
        let pack_id = if self.packs.contains_key(preferred) {
            Some(preferred.to_string())
        } else {
            self.packs.keys().min().cloned()
        };
        let Some(id) = pack_id.and_then(|pack| self.spawn(&pack).ok()) else {
            return;
        };
        let bounds = self.bounds();
        let Some(window) = self.windows.first() else {
            return;
        };
        let x = window
            .rect
            .center()
            .x
            .clamp(bounds.x, bounds.x + bounds.width);
        let y = bounds.y;
        if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
            pet.position = Point::new(x, y);
            pet.drop_in();
        }
    }

    /// Gives a pet its default working folder, or takes it away.
    pub fn set_pet_folder(&mut self, id: u64, folder: Option<String>) {
        if let Some(pet) = self.pets.iter_mut().find(|p| p.id == id) {
            pet.folder = folder;
        }
    }

    /// Every pet that has a working folder, for the bubble to read.
    pub fn pet_folders(&self) -> Vec<(u64, String)> {
        self.pets
            .iter()
            .filter_map(|p| p.folder.clone().map(|f| (p.id, f)))
            .collect()
    }

    pub fn remember_pets(&mut self) {
        self.settings.pets = self
            .pets
            .iter()
            .map(|pet| PetSetting {
                pack_id: pet.pack.manifest.id.clone(),
                x: pet.position.x.is_finite().then_some(pet.position.x),
                y: pet.position.y.is_finite().then_some(pet.position.y),
                folder: pet.folder.clone(),
                name: pet.bond.name.clone(),
                met: Some(pet.bond.met.to_string()),
                last_seen: Some(pet.bond.last_seen.to_string()),
                days: pet.bond.days,
                affection: pet.bond.affection,
                home_biome: pet.home_biome,
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
            if self.settings.effects.make_room {
                self.resize_access = platform.resize_access();
            }
        }
        if std::mem::take(&mut self.access_requested) {
            platform.request_resize_access();
            self.resize_access = platform.resize_access();
        }
        self.suspended = self.suspended || !self.screens_available || self.monitors.is_empty();
        if self.suspended || self.settings.paused || self.settings.hidden {
            return;
        }
        self.scan(platform);
        self.serve_pick(platform);
        // Decorating a biome whose window has gone is over.
        if let Some(Edit::Editing { biome }) = self.edit {
            if !self.biomes.hosts.contains_key(&biome) {
                self.end_decorating();
            }
        }
        if self.pending_welcome && !self.surfaces.is_empty() {
            self.welcome();
        }
        let events = self.awareness.tick(dt, platform);
        if self.awareness.nudge.tier() >= 2 {
            self.attention = self.windows.first().map(|w| w.id);
        } else {
            self.attention = None;
        }
        self.cursor = platform.cursor();
        let base = self.awareness.signals();
        let bounds = self.bounds();
        let held_by_mouse = platform.mouse_down();
        let peers: Vec<Peer> = self
            .pets
            .iter()
            .map(|pet| Peer {
                id: pet.id,
                position: pet.position,
                perch: pet.perch,
            })
            .collect();
        let jelly = self.settings.effects.jelly && !self.reduced_motion;
        let homes: HashMap<u64, Vec<SurfaceKey>> = self
            .settings
            .biomes
            .iter()
            .map(|b| {
                let keys = self
                    .biomes
                    .home_keys(b.id, &self.settings.biomes, &self.surfaces);
                (b.id, keys)
            })
            .collect();
        let mut pets = std::mem::take(&mut self.pets);
        for pet in &mut pets {
            if (pet.is_held() || pet.is_tended()) && held_by_mouse == Some(false) {
                pet.release();
            }
            let view = WorldView {
                peers: &peers,
                surfaces: &self.surfaces,
                graph: &self.graph,
                bounds,
                cursor: self.cursor,
                scale: self.scale_at(pet.position),
                speed_scale: self.settings.speed_scale.clamp(0.25, 3.),
                attention: self.attention,
                jelly,
                home: pet
                    .home_biome
                    .and_then(|id| homes.get(&id))
                    .map_or(&[], Vec::as_slice),
                cramped: &self.cramped,
            };
            let mut pet_events = events.clone();
            pet.tick(dt, &view, base, &mut pet_events);
            if let Some(sound) = pet.sound_request.take() {
                self.sounds.push((pet.id, sound));
            }
        }
        self.pets = pets;
        if self.settings.effects.biomes {
            let perches: Vec<(u64, Option<SurfaceKey>)> = self
                .pets
                .iter()
                .map(|p| (p.id, p.perch.map(|perch| perch.key)))
                .collect();
            let scale = self.pet_scale();
            self.biomes
                .track_landings(&perches, &self.settings.biomes, jelly);
            self.biomes.tick(
                dt,
                &self.settings.biomes,
                &self.decor,
                &self.surfaces,
                jelly,
                scale,
            );
        }
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
                if self.settings.effects.biomes {
                    self.biomes
                        .update_hosts(&self.settings.biomes, &self.windows, platform);
                } else {
                    self.biomes.clear();
                }
            }
            Err(error) => {
                self.windows.clear();
                self.scan_error = Some(error);
            }
        }
        self.rebuild();
        self.scan_ms = started.elapsed().as_secs_f64() * 1000.;
        self.make_room(platform);
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
        let (decor, decor_broken) = decor::scan(packs_dir);
        self.decor = decor
            .into_iter()
            .map(|pack| (pack.manifest.id.clone(), Arc::new(pack)))
            .collect();
        broken.extend(decor_broken.iter().map(|report| report.text()));
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

/// A pet's bond as saved. A long absence cools it a little (0.02 a missed day,
/// at most 0.2) and never resets it.
fn bond_from(setting: &PetSetting, today: NaiveDate) -> Bond {
    let parse = |d: &Option<String>| {
        d.as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    };
    let last_seen = parse(&setting.last_seen).unwrap_or(today);
    let missed = (today - last_seen).num_days().max(0) as f64;
    Bond {
        name: setting.name.clone(),
        met: parse(&setting.met).unwrap_or(today),
        last_seen,
        days: setting.days.max(1),
        affection: (setting.affection - (missed * 0.02).min(0.2)).max(0.),
    }
}
