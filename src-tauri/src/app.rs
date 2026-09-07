//! Tauri wiring: windows, commands, menus, and the update loop. All world and
//! pet truth lives in `engine`; this module only moves it in and out.
use crate::{
    awareness::AwarenessSummary,
    engine::Engine,
    packs::{
        builtin, diagnostics::Report, loader, manifest::Event, sprites, LocomotionClass,
        PackListing, PackSummary,
    },
    pet::Motion,
    platform::{NativePlatform, Platform},
    settings::{self, Settings},
    windows_mgr::{self, hit_test, DebugOverlay, Label, NativeSurface, PetSurface, Scene},
    world::{MonitorInfo, Point, Rect},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub type Shared = Arc<Mutex<App>>;

pub struct App {
    pub engine: Engine,
    pub config_dir: PathBuf,
    pub packs_dir: PathBuf,
    /// Notes for the companion window: recovered settings, broken packs.
    pub notes: Vec<String>,
    pub lab_active: bool,
    pub dirty: bool,
    /// Monitors whose overlay has just mounted and needs its spawn payloads
    /// re-sent: the first tick's sends land before the webview can listen.
    pub atlas_recheck: Vec<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetView {
    pub id: u64,
    pub pack_id: String,
    pub name: String,
    pub class: String,
    pub state: String,
    pub clip: String,
    pub motion: Motion,
    pub x: f64,
    pub y: f64,
    pub energy: f64,
    pub mood: f64,
    pub surface: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub pets: Vec<PetView>,
    pub packs: Vec<PackSummary>,
    pub settings: Settings,
    pub awareness: AwarenessSummary,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<crate::world::WindowInfo>,
    pub surfaces: Vec<SurfaceView>,
    pub bounds: Rect,
    pub perch_count: usize,
    pub scan_ms: f64,
    pub scan_error: Option<String>,
    pub suspended: bool,
    pub notes: Vec<String>,
    pub packs_dir: String,
    pub native: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceView {
    pub kind: &'static str,
    pub owner: u64,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl App {
    fn snapshot(&self) -> Snapshot {
        let engine = &self.engine;
        Snapshot {
            pets: engine
                .pets
                .iter()
                .map(|pet| PetView {
                    id: pet.id,
                    pack_id: pet.pack.manifest.id.clone(),
                    name: pet.pack.manifest.name.clone(),
                    class: pet.pack.manifest.locomotion.class.as_str().to_string(),
                    state: pet.state().to_string(),
                    clip: pet.clip().to_string(),
                    motion: pet.motion,
                    x: pet.position.x,
                    y: pet.position.y,
                    energy: pet.energy,
                    mood: pet.mood,
                    surface: pet.perch.map(|p| p.key.kind.as_str().to_string()),
                })
                .collect(),
            packs: loader::scan(&self.packs_dir)
                .into_iter()
                .map(|listing| listing.summary)
                .collect(),
            settings: engine.settings.clone(),
            awareness: engine.awareness_summary(),
            monitors: engine.monitors.clone(),
            windows: engine.windows.clone(),
            surfaces: engine
                .surfaces
                .iter()
                .map(|s| SurfaceView {
                    kind: s.kind.as_str(),
                    owner: s.owner,
                    x1: s.start.x,
                    y1: s.start.y,
                    x2: s.end.x,
                    y2: s.end.y,
                })
                .collect(),
            bounds: engine.bounds(),
            perch_count: engine.perch_count(),
            scan_ms: engine.scan_ms,
            scan_error: engine.scan_error.clone(),
            suspended: engine.suspended,
            notes: self.notes.clone(),
            packs_dir: self.packs_dir.to_string_lossy().to_string(),
            native: true,
        }
    }

    fn persist(&mut self) -> Result<(), String> {
        self.engine.remember_pets();
        settings::save(&self.config_dir, &self.engine.settings)
    }
}

/// The lock is only ever held for the length of one command, so a failure here
/// means another thread panicked while holding it.
fn locked<'a>(state: &'a State<'_, Shared>) -> Result<std::sync::MutexGuard<'a, App>, String> {
    state
        .inner()
        .lock()
        .map_err(|_| "Perch's state is unavailable. Restart the app.".to_string())
}

// ------------------------------------------------------------------- commands

#[tauri::command]
async fn get_snapshot(state: State<'_, Shared>) -> Result<Snapshot, String> {
    Ok(locked(&state)?.snapshot())
}

#[tauri::command]
async fn pet_action(
    kind: String,
    window: WebviewWindow,
    state: State<'_, Shared>,
) -> Result<Snapshot, String> {
    if window.label() != "main" {
        return Err("This control belongs to the companion window.".into());
    }
    let mut app = locked(&state)?;
    match kind.as_str() {
        "pause" => app.engine.settings.paused = !app.engine.settings.paused,
        "visibility" => app.engine.settings.hidden = !app.engine.settings.hidden,
        "debug" => app.engine.settings.debug = !app.engine.settings.debug,
        "relocate" => {
            app.engine.settings.paused = false;
            app.engine.settings.hidden = false;
            for pet in &mut app.engine.pets {
                pet.queue(Event::SurfaceLost);
            }
        }
        "greet" => {
            for pet in &mut app.engine.pets {
                pet.click();
            }
        }
        other => return Err(format!("Unknown action `{other}`.")),
    }
    app.dirty = true;
    Ok(app.snapshot())
}

#[tauri::command]
async fn pet_interact(kind: String, state: State<'_, Shared>) -> Result<(), String> {
    let mut app = locked(&state)?;
    let cursor = app.engine.cursor;
    let scale = cursor.map(|c| app.engine.scale_at(c)).unwrap_or(1.);
    match kind.as_str() {
        "grab" => {
            if let Some(cursor) = cursor {
                if let Some(pet) = app
                    .engine
                    .pets
                    .iter_mut()
                    .find(|pet| hit_test(pet, cursor, scale))
                {
                    pet.grab();
                }
            }
        }
        "release" => {
            for pet in &mut app.engine.pets {
                if pet.is_held() {
                    pet.release();
                }
            }
        }
        "click" => {
            if let Some(cursor) = cursor {
                if let Some(pet) = app
                    .engine
                    .pets
                    .iter_mut()
                    .find(|pet| hit_test(pet, cursor, scale))
                {
                    pet.click();
                }
            }
        }
        other => return Err(format!("Unknown interaction `{other}`.")),
    }
    Ok(())
}

#[tauri::command]
async fn set_settings(
    settings: Settings,
    window: WebviewWindow,
    state: State<'_, Shared>,
) -> Result<Snapshot, String> {
    if window.label() != "main" {
        return Err("Settings can only be changed from the companion window.".into());
    }
    let mut app = locked(&state)?;
    let mut settings = settings;
    settings.sanitize();
    app.engine
        .awareness
        .apply(settings.awareness.clone(), settings.category_map());
    let pack_ids = |pets: &[crate::settings::PetSetting]| {
        pets.iter().map(|p| p.pack_id.clone()).collect::<Vec<_>>()
    };
    let pets_changed = pack_ids(&settings.pets) != pack_ids(&app.engine.settings.pets);
    if settings.launch_at_login != app.engine.settings.launch_at_login {
        if let Ok(executable) = std::env::current_exe() {
            if let Err(problem) = crate::autostart::apply(settings.launch_at_login, &executable) {
                app.notes.push(problem);
                settings.launch_at_login = app.engine.settings.launch_at_login;
            }
        }
    }
    app.engine.settings = settings;
    if pets_changed {
        let ids: Vec<u64> = app.engine.pets.iter().map(|p| p.id).collect();
        for id in ids {
            app.engine.despawn(id);
        }
        app.engine.restore_pets();
    }
    app.dirty = true;
    let snapshot = app.snapshot();
    let _ = app.persist();
    Ok(snapshot)
}

#[tauri::command]
async fn list_packs(state: State<'_, Shared>) -> Result<Vec<PackListing>, String> {
    let dir = locked(&state)?.packs_dir.clone();
    Ok(loader::scan(&dir))
}

#[tauri::command]
async fn validate_pack(path: String, state: State<'_, Shared>) -> Result<Report, String> {
    // Pet Lab validates a folder in place, without installing it.
    let path = if path.is_empty() {
        locked(&state)?.packs_dir.clone()
    } else {
        PathBuf::from(path)
    };
    Ok(match loader::load(&path, false) {
        Ok(pack) => pack.report,
        Err(report) => report,
    })
}

#[tauri::command]
async fn install_pack(path: String, state: State<'_, Shared>) -> Result<PackSummary, String> {
    let dir = locked(&state)?.packs_dir.clone();
    let summary = loader::install(Path::new(&path), &dir)?;
    let mut app = locked(&state)?;
    app.engine.reload_packs(&dir);
    Ok(summary)
}

#[tauri::command]
async fn uninstall_pack(id: String, state: State<'_, Shared>) -> Result<(), String> {
    let mut app = locked(&state)?;
    let dir = app.packs_dir.clone();
    let builtin_folder = loader::scan(&dir)
        .into_iter()
        .find(|l| l.summary.id == id)
        .filter(|l| l.summary.builtin)
        .map(|l| {
            Path::new(&l.summary.directory)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    loader::uninstall(&dir, &id)?;
    if let Some(folder) = builtin_folder {
        // Remember the removal so the built-in is not written back on launch.
        app.engine.settings.uninstalled_builtins.push(folder);
    }
    let ids: Vec<u64> = app
        .engine
        .pets
        .iter()
        .filter(|p| p.pack.manifest.id == id)
        .map(|p| p.id)
        .collect();
    for pet in ids {
        app.engine.despawn(pet);
    }
    app.engine.settings.pets.retain(|p| p.pack_id != id);
    app.engine.reload_packs(&dir);
    app.dirty = true;
    let _ = app.persist();
    Ok(())
}

#[tauri::command]
async fn new_pack(class: String, name: String, state: State<'_, Shared>) -> Result<String, String> {
    let class = LocomotionClass::parse(&class)
        .ok_or_else(|| format!("`{class}` is not a locomotion class."))?;
    let mut app = locked(&state)?;
    let dir = app.packs_dir.clone();
    let created = crate::packs::scaffold::create(&dir, class, &name)?;
    app.engine.reload_packs(&dir);
    Ok(created.to_string_lossy().to_string())
}

#[tauri::command]
async fn export_pack(id: String, state: State<'_, Shared>) -> Result<String, String> {
    let app = locked(&state)?;
    let dir = app.packs_dir.clone();
    let listing = loader::scan(&dir)
        .into_iter()
        .find(|l| l.summary.id == id)
        .ok_or_else(|| format!("No installed pack has the id `{id}`."))?;
    let source = PathBuf::from(&listing.summary.directory);
    let destination = dir.join(format!(
        "{}.petpack",
        source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(id)
    ));
    loader::export(&source, &destination)?;
    Ok(destination.to_string_lossy().to_string())
}

#[tauri::command]
async fn spawn_pet(pack_id: String, state: State<'_, Shared>) -> Result<u64, String> {
    let mut app = locked(&state)?;
    let id = app.engine.spawn(&pack_id)?;
    app.dirty = true;
    let _ = app.persist();
    Ok(id)
}

#[tauri::command]
async fn despawn_pet(pet_id: u64, state: State<'_, Shared>) -> Result<(), String> {
    let mut app = locked(&state)?;
    if !app.engine.despawn(pet_id) {
        return Err("That pet is not on the desktop.".into());
    }
    app.dirty = true;
    let _ = app.persist();
    Ok(())
}

/// Pet Lab scenario buttons: fire an awareness event without waiting an hour.
#[tauri::command]
async fn fire_event(name: String, state: State<'_, Shared>) -> Result<(), String> {
    let event = Event::parse(&name).ok_or_else(|| format!("Unknown event `{name}`."))?;
    let mut app = locked(&state)?;
    for pet in &mut app.engine.pets {
        pet.queue(event);
    }
    Ok(())
}

#[tauri::command]
async fn set_lab(active: bool, state: State<'_, Shared>) -> Result<(), String> {
    locked(&state)?.lab_active = active;
    Ok(())
}

/// An overlay has mounted and registered its listeners; ask the update loop to
/// re-send the spawn payloads the first ticks delivered too early.
#[tauri::command]
fn pet_surface_ready(monitor: u64, state: State<'_, Shared>) {
    if let Ok(mut app) = locked(&state) {
        if !app.atlas_recheck.contains(&monitor) {
            app.atlas_recheck.push(monitor);
        }
    }
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle, window: WebviewWindow, state: State<'_, Shared>) {
    if window.label() == "main" {
        if let Ok(mut shared) = state.inner().lock() {
            let _ = shared.persist();
        }
        app.exit(0);
    }
}

// ---------------------------------------------------------------------- setup

pub fn run(context: tauri::Context<tauri::Wry>) {
    tauri::Builder::default()
        .on_menu_event(handle_menu)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            pet_action,
            pet_interact,
            set_settings,
            list_packs,
            validate_pack,
            install_pack,
            uninstall_pack,
            new_pack,
            export_pack,
            spawn_pet,
            despawn_pet,
            fire_event,
            set_lab,
            pet_surface_ready,
            quit_app
        ])
        .setup(|app| {
            let main = app
                .get_webview_window("main")
                .ok_or("The companion window is missing")?;
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let packs_dir = config_dir.join("packs");
            let (settings, note) = settings::load(&config_dir);
            let mut notes: Vec<String> = note.into_iter().collect();
            if let Err(error) = builtin::restore(&packs_dir, &settings.uninstalled_builtins) {
                notes.push(error);
            }
            let mut engine = Engine::new(settings, HashMap::new());
            for report in engine.reload_packs(&packs_dir) {
                notes.push(report);
            }
            // Monitor geometry is occasionally not ready this early; the update
            // loop re-checks every two seconds, so an empty list is survivable.
            engine.set_monitors(monitors(app.handle()).unwrap_or_default());
            if let Ok(executable) = std::env::current_exe() {
                let _ = crate::autostart::apply(engine.settings.launch_at_login, &executable);
            }
            let missing = engine.restore_pets();
            for pack_id in missing {
                notes.push(format!(
                    "The pack `{pack_id}` is no longer installed, so that pet did not come back."
                ));
            }
            let shared: Shared = Arc::new(Mutex::new(App {
                engine,
                config_dir,
                packs_dir,
                notes,
                lab_active: false,
                dirty: false,
                atlas_recheck: Vec::new(),
            }));
            app.manage(shared.clone());
            setup_tray(app)?;
            let main_close = main.clone();
            let dropped = shared.clone();
            let dropped_handle = app.handle().clone();
            main.on_window_event(move |event| match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Closing the companion leaves the pet on the desktop.
                    api.prevent_close();
                    let _ = main_close.hide();
                }
                // Dragging a .petpack or a pack folder onto the window installs it.
                tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                    install_dropped(&dropped, &dropped_handle, paths.clone());
                }
                _ => {}
            });
            main.set_focus()?;
            let handle = app.handle().clone();
            std::thread::spawn(move || run_loop(shared, handle));
            Ok(())
        })
        .run(context)
        .expect("Unable to start Perch");
}

fn install_dropped(shared: &Shared, handle: &tauri::AppHandle, paths: Vec<PathBuf>) {
    let shared = shared.clone();
    let handle = handle.clone();
    std::thread::spawn(move || {
        let Ok(mut app) = shared.lock() else { return };
        let directory = app.packs_dir.clone();
        for path in paths {
            match loader::install(&path, &directory) {
                Ok(summary) => app
                    .notes
                    .push(format!("Installed {} by {}.", summary.name, summary.author)),
                Err(problem) => app.notes.push(problem),
            }
        }
        app.engine.reload_packs(&directory);
        let _ = handle.emit("packs:changed", ());
    });
}

/// Run something on the UI thread and wait for the answer. macOS only reports
/// monitor geometry, and only creates windows, on the main thread; calling from
/// the update loop silently returns nothing.
fn on_main<T: Send + 'static>(
    handle: &tauri::AppHandle,
    work: impl FnOnce(&tauri::AppHandle) -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle_for_work = handle.clone();
    handle
        .run_on_main_thread(move || {
            let _ = sender.send(work(&handle_for_work));
        })
        .ok()?;
    receiver.recv_timeout(Duration::from_secs(2)).ok()
}

/// Monitor geometry comes from the app handle rather than a window, because a
/// hidden companion window reports no monitors at all.
fn monitors(app: &tauri::AppHandle) -> Result<Vec<MonitorInfo>, String> {
    // Ask a real window: on macOS the app-level monitor list can come back
    // empty, while the window-level one is reliable.
    let window = app.get_webview_window("main");
    let primary = match &window {
        Some(window) => window.primary_monitor(),
        None => app.primary_monitor(),
    }
    .map_err(|e| e.to_string())?;
    let primary_position = primary.as_ref().map(|m| m.position().to_owned());
    let mut out = Vec::new();
    let found = match &window {
        Some(window) => window.available_monitors(),
        None => app.available_monitors(),
    }
    .map_err(|e| e.to_string())?;
    for (index, monitor) in found.into_iter().enumerate() {
        let position = monitor.position();
        let size = monitor.size();
        let work = monitor.work_area();
        out.push(MonitorInfo {
            id: index as u64,
            bounds: Rect::new(
                position.x as f64,
                position.y as f64,
                size.width as f64,
                size.height as f64,
            ),
            work: Rect::new(
                work.position.x as f64,
                work.position.y as f64,
                work.size.width as f64,
                work.size.height as f64,
            ),
            scale: monitor.scale_factor(),
            is_primary: primary_position.as_ref() == Some(position),
        });
    }
    // An empty list is a legitimate answer while the session is locked.
    Ok(out)
}

fn build_overlays(
    app: &tauri::AppHandle,
    monitors: &[MonitorInfo],
    hidden: bool,
) -> HashMap<u64, WebviewWindow> {
    let mut windows = HashMap::new();
    for monitor in monitors {
        let label = format!("pet-{}", monitor.id);
        if let Some(existing) = app.get_webview_window(&label) {
            let _ = existing.close();
        }
        let built = WebviewWindowBuilder::new(
            app,
            &label,
            WebviewUrl::App(format!("index.html?pet&monitor={}", monitor.id).into()),
        )
        .title("Perch")
        .inner_size(
            monitor.bounds.width / monitor.scale,
            monitor.bounds.height / monitor.scale,
        )
        .position(
            monitor.bounds.x / monitor.scale,
            monitor.bounds.y / monitor.scale,
        )
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(!hidden)
        .build();
        match built {
            Ok(window) => {
                let _ = window.set_ignore_cursor_events(true);
                windows.insert(monitor.id, window);
            }
            Err(error) => eprintln!("Perch could not create an overlay: {error}"),
        }
    }
    windows
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem, Submenu},
        tray::TrayIconBuilder,
    };
    let show = MenuItem::with_id(app, "show", "Open Perch", true, Some("CmdOrCtrl+1"))?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Pause / resume",
        true,
        Some("CmdOrCtrl+Shift+P"),
    )?;
    let visibility = MenuItem::with_id(
        app,
        "visibility",
        "Show / hide pets",
        true,
        Some("CmdOrCtrl+Shift+H"),
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Perch", true, Some("CmdOrCtrl+Q"))?;
    let submenu = Submenu::with_items(app, "Perch", true, &[&show, &pause, &visibility, &quit])?;
    app.set_menu(Menu::with_items(app, &[&submenu])?)?;
    // Tray items are separate so their accelerators do not double-fire.
    let tray_show = MenuItem::with_id(app, "tray-show", "Open Perch", true, None::<&str>)?;
    let tray_pause = MenuItem::with_id(app, "tray-pause", "Pause / resume", true, None::<&str>)?;
    let tray_visibility = MenuItem::with_id(
        app,
        "tray-visibility",
        "Show / hide pets",
        true,
        None::<&str>,
    )?;
    let tray_quit = MenuItem::with_id(app, "tray-quit", "Quit Perch", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&tray_show, &tray_pause, &tray_visibility, &tray_quit],
    )?;
    TrayIconBuilder::with_id("perch")
        .icon(tauri::include_image!("icons/icon.png"))
        .tooltip("Perch · a little company")
        .menu(&menu)
        .build(app)?;
    Ok(())
}

fn handle_menu(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    let kind = event.id.as_ref().trim_start_matches("tray-").to_string();
    match kind.as_str() {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "pause" | "visibility" => {
            let shared = app.state::<Shared>().inner().clone();
            std::thread::spawn(move || {
                if let Ok(mut app) = shared.lock() {
                    match kind.as_str() {
                        "pause" => app.engine.settings.paused = !app.engine.settings.paused,
                        _ => app.engine.settings.hidden = !app.engine.settings.hidden,
                    }
                    app.dirty = true;
                }
            });
        }
        "quit" => {
            if let Ok(mut app_state) = app.state::<Shared>().inner().lock() {
                let _ = app_state.persist();
            }
            app.exit(0);
        }
        _ => {}
    }
}

// ----------------------------------------------------------------------- loop

/// Everything one tick wants to send to the overlays, computed while the
/// engine lock is held and emitted after it has been released. Emitting hops
/// to the main thread, which may itself be waiting for the lock inside a
/// command handler, so the two must never overlap.
/// Pets standing on one monitor: id paired with which build of its pack it is
/// using, so a live-reloaded pack re-sends its sprites.
type Present = Vec<(u64, usize)>;

/// One overlay's sprite bookkeeping for this tick.
struct AtlasSync {
    monitor: u64,
    present: Present,
    payloads: Vec<windows_mgr::SpawnPayload>,
}

struct RenderPlan {
    scenes: Vec<Scene>,
    sync: Vec<AtlasSync>,
    interactive: Vec<(u64, bool)>,
    sounds: Vec<String>,
    visible: bool,
    moving: bool,
}

fn run_loop(shared: Shared, handle: tauri::AppHandle) {
    let platform = NativePlatform;
    let mut renderer = NativeSurface::new(HashMap::new());
    let mut known_monitors: Vec<MonitorInfo> = Vec::new();
    let mut last = Instant::now();
    let mut last_monitor_check = Instant::now() - Duration::from_secs(10);
    let mut last_watch = Instant::now();
    let mut last_save = Instant::now();
    let mut watch_signature = String::new();
    let diagnostics = std::env::var_os("PERCH_DIAGNOSTICS").is_some();
    let mut last_diagnostic = Instant::now();
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64();
        last = now;

        if last_monitor_check.elapsed() > Duration::from_secs(2) {
            last_monitor_check = now;
            match on_main(&handle, monitors) {
                Some(Ok(found)) if found.is_empty() => {
                    // Locked session or sleeping display: keep what we know.
                    if let Ok(mut app) = shared.lock() {
                        if app.engine.screens_available && diagnostics {
                            eprintln!("Perch suspended: the screens are not readable.");
                        }
                        app.engine.screens_available = false;
                    }
                }
                Some(Ok(found)) if found != known_monitors => {
                    if diagnostics {
                        eprintln!("Perch sees {} display(s).", found.len());
                    }
                    known_monitors = found.clone();
                    platform.set_monitors(found.clone());
                    let hidden = shared
                        .lock()
                        .map(|mut app| {
                            app.engine.set_monitors(found.clone());
                            app.engine.settings.hidden
                        })
                        .unwrap_or(false);
                    // Built on the UI thread, and never while holding the lock.
                    let overlays = on_main(&handle, move |handle| {
                        build_overlays(handle, &found, hidden)
                    });
                    renderer.replace(overlays.unwrap_or_default());
                }
                Some(Err(error)) if diagnostics => {
                    eprintln!("Perch could not read monitor geometry: {error}");
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
                eprintln!("Perch could not draw: {error}");
            }
        }
        for entry in plan.sync {
            if let Err(error) = renderer.sync(entry.monitor, &entry.present, entry.payloads) {
                eprintln!("Perch could not send sprites: {error}");
            }
        }
        for (monitor, interactive) in plan.interactive {
            let _ = renderer.set_interactive(monitor, interactive);
        }
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

fn watch_interval(lab: bool) -> Duration {
    // Pet Lab wants edits on screen inside ~200 ms; otherwise polling is lazy.
    Duration::from_millis(if lab { 120 } else { 1000 })
}

/// Build this tick's render payload. Pure reads of engine state plus the
/// renderer's record of which atlases each overlay already holds.
fn plan_render(app: &mut App, renderer: &NativeSurface, resting: bool) -> RenderPlan {
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
