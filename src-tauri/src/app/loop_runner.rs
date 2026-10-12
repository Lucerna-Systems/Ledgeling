use super::{
    screens::{chosen_screen, ensure_overlay, on_main, pet_monitor, screens},
    App, Shared,
};
use crate::{
    engine::Engine,
    packs::{loader, sprites},
    pet::Motion,
    platform::{NativePlatform, Platform},
    windows_mgr::{self, hit_test, DebugOverlay, Label, NativeSurface, PetSurface, Scene},
    world::{MonitorInfo, Point},
};
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::Emitter;

/// Everything one tick wants to send to the overlays, computed while the
/// engine lock is held and emitted after it has been released. Emitting hops
/// to the main thread, which may itself be waiting for the lock inside a
/// command handler, so the two must never overlap.
/// Pets standing on one monitor: id paired with which build of its pack it is
/// using, so a live-reloaded pack re-sends its sprites.
pub type Present = Vec<(u64, usize)>;

/// One overlay's sprite bookkeeping for this tick.
pub struct AtlasSync {
    pub monitor: u64,
    pub present: Present,
    pub payloads: Vec<windows_mgr::SpawnPayload>,
}

pub struct RenderPlan {
    pub scenes: Vec<Scene>,
    pub sync: Vec<AtlasSync>,
    pub interactive: Vec<(u64, bool)>,
    pub sounds: Vec<String>,
    pub visible: bool,
    pub moving: bool,
}

pub fn run_loop(shared: Shared, handle: tauri::AppHandle) {
    // A unit struct everywhere but Linux, whose adapter holds its X
    // connection and idle source.
    #[allow(clippy::default_constructed_unit_structs)]
    let platform = NativePlatform::default();
    let mut renderer = NativeSurface::new(HashMap::new());
    // The pet screen the engine and the overlay currently reflect, in
    // canonical form (id 0). `None` means no overlay window exists yet.
    let mut active: Option<MonitorInfo> = None;
    let mut last = Instant::now();
    let mut last_monitor_check = Instant::now() - Duration::from_secs(10);
    let mut last_watch = Instant::now();
    let mut last_save = Instant::now();
    let mut watch_signature = String::new();
    let diagnostics = std::env::var_os("LEDGELING_DIAGNOSTICS")
        .or_else(|| std::env::var_os("PERCH_DIAGNOSTICS"))
        .is_some();
    let mut last_diagnostic = Instant::now();
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64();
        last = now;

        // The pet screen can also be re-picked from a command, which sets a
        // flag rather than waiting out the two-second poll.
        let mut rescan = false;
        if let Ok(mut app) = shared.lock() {
            rescan = app.rescan_screens;
            app.rescan_screens = false;
        }
        if last_monitor_check.elapsed() > Duration::from_secs(2) || rescan {
            last_monitor_check = now;
            match on_main(&handle, screens) {
                Some(Ok(found)) if found.is_empty() => {
                    // Locked session or sleeping display: keep what we know.
                    if let Ok(mut app) = shared.lock() {
                        if app.engine.screens_available && diagnostics {
                            eprintln!("Ledgeling suspended: the screens are not readable.");
                        }
                        app.engine.screens_available = false;
                    }
                }
                Some(Ok(found)) => {
                    let count = found.len();
                    let wanted = shared
                        .lock()
                        .map(|app| app.engine.settings.pet_screen.clone())
                        .ok()
                        .flatten();
                    let chosen = chosen_screen(&found, wanted.as_deref())
                        .map(|screen| (screen.name.clone(), pet_monitor(screen)));
                    if let Some((name, monitor)) = chosen {
                        let changed = active != Some(monitor);
                        if let Ok(mut app) = shared.lock() {
                            app.screens = found;
                            app.pet_screen = Some(name.clone());
                            // Readable again after a lock or display sleep. Only
                            // `set_monitors` used to clear this, and it runs
                            // only when the screen changed, so pets stayed
                            // suspended for good once the session came back.
                            app.engine.screens_available = true;
                            if changed {
                                app.engine.set_monitors(vec![monitor]);
                            }
                            // A pick naming hardware that is gone is adopted
                            // over by the screen pets actually fell back to, so
                            // the setting, the picker and the overlay always
                            // agree on one connected display.
                            if wanted.as_deref().is_some_and(|wanted| wanted != name) {
                                app.engine.settings.pet_screen = Some(name.clone());
                                app.dirty = true;
                            }
                        }
                        if changed {
                            if diagnostics {
                                eprintln!(
                                    "Ledgeling's screen: {name} ({count} display(s) connected)."
                                );
                            }
                            platform.set_monitors(vec![monitor]);
                            let hidden = shared
                                .lock()
                                .map(|app| app.engine.settings.hidden)
                                .unwrap_or(false);
                            // Moved or built on the UI thread, never while
                            // holding the lock.
                            let overlay = on_main(&handle, move |handle| {
                                ensure_overlay(handle, &monitor, hidden)
                            });
                            match overlay.flatten() {
                                Some((window, created)) => {
                                    if created || !renderer.has(0) {
                                        renderer.replace(HashMap::from([(0u64, window)]));
                                    }
                                    active = Some(monitor);
                                }
                                None => renderer.replace(HashMap::new()),
                            }
                        }
                    }
                }
                Some(Err(error)) if diagnostics => {
                    eprintln!("Ledgeling could not read screen geometry: {error}");
                }
                _ => {}
            }
        }

        let Ok(mut app) = shared.lock() else {
            break;
        };
        // The lock is held only for engine state; the renderer is owned by
        // this thread, so bookkeeping on it cannot deadlock.
        for monitor in std::mem::take(&mut app.atlas_recheck) {
            renderer.request_atlases(monitor);
        }
        app.engine.tick(dt, &platform);
        let resting =
            app.engine.suspended || app.engine.settings.paused || app.engine.settings.hidden;
        let plan = plan_render(&mut app, &renderer, resting);
        let bubble = crate::platform::pet_ui::open_pet()
            .and_then(|pet| crate::platform::pet_ui::anchor(&app.engine, pet));
        let packs_dir = app.packs_dir.clone();
        let watch_due = last_watch.elapsed() > watch_interval(app.lab_active);
        let save_due = app.dirty && last_save.elapsed() > Duration::from_secs(5);
        if save_due {
            app.dirty = false;
            let _ = app.persist();
        }
        if diagnostics && last_diagnostic.elapsed() >= Duration::from_secs(5) {
            last_diagnostic = now;
            eprintln!(
                "{}",
                serde_json::json!({
                    "monitors": app.engine.monitors.len(),
                    "pets": app.engine.pets.len(),
                    "windows": app.engine.windows.len(),
                    "surfaces": app.engine.surfaces.len(),
                    "perches": app.engine.perch_count(),
                    "kinds": kind_counts(&app.engine.surfaces),
                    "scanMs": app.engine.scan_ms,
                    "scanError": app.engine.scan_error,
                    "grab": grab_points(&app.engine),
                    "suspended": app.engine.suspended,
                    "screens": app.engine.screens_available,
                })
            );
        }
        drop(app);

        // From here on the lock is released, so emitting cannot deadlock.
        let _ = renderer.set_visible(plan.visible);
        for scene in &plan.scenes {
            if let Err(error) = renderer.render(scene) {
                eprintln!("Ledgeling could not draw: {error}");
            }
        }
        for entry in plan.sync {
            if let Err(error) = renderer.sync(entry.monitor, &entry.present, entry.payloads) {
                eprintln!("Ledgeling could not send sprites: {error}");
            }
        }
        for (monitor, interactive) in plan.interactive {
            let _ = renderer.set_interactive(monitor, interactive);
        }
        crate::platform::pet_ui::follow(&handle, bubble);
        for sound in plan.sounds {
            let _ = handle.emit("pet:sound", sound);
        }
        if last_save.elapsed() > Duration::from_secs(5) {
            last_save = now;
        }
        if watch_due {
            last_watch = now;
            let signature = directory_signature(&packs_dir);
            if signature != watch_signature {
                let first = watch_signature.is_empty();
                watch_signature = signature;
                if !first {
                    if let Ok(mut app) = shared.lock() {
                        app.engine.reload_packs(&packs_dir);
                    }
                    let _ = handle.emit("packs:changed", ());
                }
            }
        }
        if resting {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        // 60 Hz only while something is actually moving; 30 Hz otherwise.
        let budget = Duration::from_secs_f64(if plan.moving { 1. / 60. } else { 1. / 30. });
        std::thread::sleep(budget.saturating_sub(now.elapsed()));
    }
}

/// Surfaces per kind, for the diagnostics line only.
fn kind_counts(surfaces: &[crate::world::Surface]) -> serde_json::Value {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for surface in surfaces {
        *counts.entry(surface.kind.as_str()).or_default() += 1;
    }
    serde_json::json!(counts)
}

/// A point on each pet that catches the pointer, for the diagnostics line
/// only: it lets a scripted run hover a pet without seeing the screen.
fn grab_points(engine: &Engine) -> Vec<[f64; 2]> {
    engine
        .pets
        .iter()
        .filter_map(|pet| {
            let scale = engine.scale_at(pet.position);
            let rect = pet.sprite_rect(scale);
            (1..8)
                .flat_map(|i| (1..8).map(move |j| (i, j)))
                .map(|(i, j)| {
                    Point::new(
                        rect.x + rect.width * i as f64 / 8.,
                        rect.y + rect.height * j as f64 / 8.,
                    )
                })
                .find(|point| hit_test(pet, *point, scale))
                .map(|point| [point.x.round(), point.y.round()])
        })
        .collect()
}

fn watch_interval(lab: bool) -> Duration {
    // Pet Lab wants edits on screen inside ~200 ms; otherwise polling is lazy.
    Duration::from_millis(if lab { 120 } else { 1000 })
}

/// Build this tick's render payload. Pure reads of engine state plus the
/// renderer's record of which atlases each overlay already holds.
pub fn plan_render(app: &mut App, renderer: &NativeSurface, resting: bool) -> RenderPlan {
    let sounds = if app.engine.settings.sounds {
        let queued = std::mem::take(&mut app.engine.sounds);
        queued
            .into_iter()
            .filter_map(|(pet_id, relative)| sound_data_uri(app, pet_id, &relative))
            .collect()
    } else {
        app.engine.sounds.clear();
        Vec::new()
    };
    let mut plan = RenderPlan {
        scenes: Vec::new(),
        sync: Vec::new(),
        interactive: Vec::new(),
        sounds,
        visible: !(app.engine.settings.hidden || app.engine.suspended),
        moving: false,
    };
    if resting {
        return plan;
    }
    let engine = &app.engine;
    let cursor = engine.cursor;
    plan.moving = engine
        .pets
        .iter()
        .any(|pet| !matches!(pet.motion, Motion::Settled));
    for monitor in &engine.monitors {
        let here: Vec<&crate::pet::Pet> = engine
            .pets
            .iter()
            .filter(|pet| {
                engine
                    .monitor_for(pet.position)
                    .map(|m| m.id == monitor.id)
                    .unwrap_or(monitor.is_primary)
            })
            .collect();
        let present: Present = here
            .iter()
            .map(|pet| (pet.id, Arc::as_ptr(&pet.pack) as usize))
            .collect();
        let needed = renderer.missing_atlases(monitor.id, &present);
        let payloads = here
            .iter()
            .filter(|pet| needed.contains(&pet.id))
            .map(|pet| windows_mgr::SpawnPayload {
                pet_id: pet.id,
                pack_id: pet.pack.manifest.id.clone(),
                name: pet.pack.manifest.name.clone(),
                atlas: pet.pack.atlas(),
            })
            .collect();
        plan.sync.push(AtlasSync {
            monitor: monitor.id,
            present,
            payloads,
        });

        let mut frames = Vec::new();
        let mut interactive = false;
        for pet in &here {
            let mut frame = pet.frame(monitor.scale, 1.);
            let local = windows_mgr::to_local(pet.position, monitor);
            frame.x = local.x;
            frame.y = local.y;
            frames.push(frame);
            if let Some(cursor) = cursor {
                if pet.is_held() || hit_test(pet, cursor, monitor.scale) {
                    interactive = true;
                }
            }
        }
        plan.interactive.push((monitor.id, interactive));
        plan.scenes.push(Scene {
            monitor: monitor.id,
            frames,
            debug: engine.settings.debug.then(|| debug_overlay(app, monitor)),
        });
    }
    plan
}

fn debug_overlay(app: &App, monitor: &MonitorInfo) -> DebugOverlay {
    let engine = &app.engine;
    let local = |p: Point| windows_mgr::to_local(p, monitor);
    let surfaces = engine
        .surfaces
        .iter()
        .filter(|s| s.monitor == monitor.id)
        .map(|s| {
            let (a, b) = (local(s.start), local(s.end));
            [a.x, a.y, b.x, b.y]
        })
        .collect();
    let jumps = engine
        .graph
        .edges
        .iter()
        .filter(|e| e.kind == crate::world::EdgeKind::Jumpable)
        .filter(|e| {
            engine
                .surfaces
                .get(e.a)
                .is_some_and(|s| s.monitor == monitor.id)
        })
        .map(|e| {
            let (a, b) = (local(e.from), local(e.to));
            [a.x, a.y, b.x, b.y]
        })
        .collect();
    let paths = engine
        .pets
        .iter()
        .filter_map(|pet| {
            let plan = pet.plan.as_ref()?;
            let mut points = vec![local(pet.position)];
            for step in &plan.steps[plan.index.min(plan.steps.len())..] {
                if let Some(index) = crate::pet::pathfind::find_key(&engine.surfaces, step.key) {
                    let surface = &engine.surfaces[index];
                    points.push(local(surface.point(step.from_t)));
                    points.push(local(surface.point(step.to_t)));
                }
            }
            Some(points.into_iter().map(|p| [p.x, p.y]).collect())
        })
        .collect();
    let labels = engine
        .pets
        .iter()
        .filter(|p| {
            engine
                .monitor_for(p.position)
                .map(|m| m.id == monitor.id)
                .unwrap_or(false)
        })
        .map(|pet| {
            let p = local(pet.position);
            Label {
                x: p.x,
                y: p.y,
                text: format!(
                    "{} · {} · e{:.0}% m{:.0}%",
                    pet.pack.manifest.name,
                    pet.state(),
                    pet.energy * 100.,
                    pet.mood * 100.
                ),
            }
        })
        .collect();
    DebugOverlay {
        surfaces,
        jumps,
        paths,
        labels,
    }
}

fn sound_data_uri(app: &App, pet_id: u64, relative: &str) -> Option<String> {
    let pet = app.engine.pets.iter().find(|p| p.id == pet_id)?;
    let path = loader::safe_join(&pet.pack.directory, relative)?;
    let bytes = std::fs::read(&path).ok()?;
    let mime = if relative.ends_with(".wav") {
        "audio/wav"
    } else if relative.ends_with(".mp3") {
        "audio/mpeg"
    } else {
        "audio/ogg"
    };
    Some(format!("data:{mime};base64,{}", sprites::base64(&bytes)))
}

/// Cheap change detector for the packs directory: names, sizes and mtimes.
fn directory_signature(directory: &Path) -> String {
    fn walk(directory: &Path, out: &mut Vec<String>, depth: usize) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        let mut rows: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                walk(&path, out, depth + 1);
            } else {
                let stamp = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                rows.push(format!("{}:{}:{stamp}", path.display(), meta.len()));
            }
        }
        rows.sort();
        out.extend(rows);
    }
    let mut rows = Vec::new();
    walk(directory, &mut rows, 0);
    rows.join("|")
}
