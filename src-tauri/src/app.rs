use crate::tasks::commands::*;
// Tauri wiring: windows, commands, menus, and the update loop. All world and
// pet truth lives in `engine`; this module only moves it in and out.
use crate::{
    awareness::AwarenessSummary,
    engine::Engine,
    packs::{
        builtin, diagnostics::Report, loader, manifest::Event, sprites, LocomotionClass,
        PackListing, PackSummary,
    },
    pet::{Attend, Motion},
    platform::{NativePlatform, Platform},
    settings::{self, Settings, ThemeMode},
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
    /// Every connected screen, for the picker. Only one of them — the pet
    /// screen — is ever given to the engine or the platform.
    pub screens: Vec<Screen>,
    /// The hardware name of the screen pets are actually on, which is the
    /// user's pick when that display is connected and the main display
    /// otherwise. Snapshot readers need it to show the truth, not the intent.
    pub pet_screen: Option<String>,
    /// Set when the user picks a different pet screen, so the update loop
    /// re-reads the display list immediately instead of up to two seconds later.
    pub rescan_screens: bool,
}

/// A connected screen plus the stable name the pet-screen setting keys on.
/// Tauri derives it from the display's model number, so it survives unplug,
/// replug and list reshuffles; `MonitorInfo::id` does not (it is a list index).
#[derive(Clone, Debug)]
pub struct Screen {
    pub info: MonitorInfo,
    pub name: String,
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
    /// Set while the pet is with the user rather than wandering: its bubble is
    /// open, or it has a task running or waiting on an answer.
    pub attend: Option<Attend>,
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
    /// Every connected screen for the picker; `monitors` is the single one
    /// pets live on.
    pub screens: Vec<ScreenView>,
    /// The screen pets are actually on (the user's pick when it is connected,
    /// else the main display).
    pub pet_screen: Option<String>,
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
pub struct ScreenView {
    #[serde(flatten)]
    pub info: MonitorInfo,
    pub name: String,
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
                    attend: pet.attend(),
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
            screens: self
                .screens
                .iter()
                .map(|screen| ScreenView {
                    info: screen.info,
                    name: screen.name.clone(),
                })
                .collect(),
            pet_screen: self.pet_screen.clone(),
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

/// How a task's status reads on the pet carrying it. Only a task that is
/// genuinely stuck on the user counts as needing attention: a finished or
/// failed task is history, and a pet pinned by history would never move again.
pub fn task_attention(status: &str) -> Option<Attend> {
    match status {
        "queued" | "running" | "cancelling" | "undoing" => Some(Attend::Busy),
        "waiting" => Some(Attend::NeedsUser),
        _ => None,
    }
}

/// The lock is only ever held for the length of one command, so a failure here
/// means another thread panicked while holding it.
/// The pack and name of a pet that is out right now, for stamping on its tasks.
pub fn task_pet(
    app: &tauri::AppHandle,
    pet_id: Option<u64>,
) -> Option<crate::tasks::types::TaskPet> {
    let shared = app.try_state::<Shared>()?;
    let state = shared.lock().ok()?;
    let pet = state.engine.pets.iter().find(|p| Some(p.id) == pet_id)?;
    Some(crate::tasks::types::TaskPet {
        pack_id: pet.pack.manifest.id.clone(),
        name: pet.pack.manifest.name.clone(),
    })
}

fn locked<'a>(state: &'a State<'_, Shared>) -> Result<std::sync::MutexGuard<'a, App>, String> {
    state
        .inner()
        .lock()
        .map_err(|_| "Ledgeling's state is unavailable. Restart the app.".to_string())
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

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn perch_set_dock_icon(png: *const u8, length: std::ffi::c_ulong);
}

/// Repaints the running app's Dock/taskbar icon in the user's accent; icons
/// shown while the app is closed never change.
#[tauri::command]
fn set_app_icon(png: Vec<u8>, window: WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("The app icon can only be changed from the companion window.".into());
    }
    #[cfg(target_os = "macos")]
    {
        window
            .run_on_main_thread(move || unsafe {
                perch_set_dock_icon(png.as_ptr(), png.len() as std::ffi::c_ulong)
            })
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let icon = decode_icon(&png)?;
        window.set_icon(icon).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn decode_icon(bytes: &[u8]) -> Result<tauri::image::Image<'static>, String> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba {
        return Err("The app icon must be an RGBA PNG.".into());
    }
    buffer.truncate(info.buffer_size());
    Ok(tauri::image::Image::new_owned(
        buffer,
        info.width,
        info.height,
    ))
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
    let screen_changed = settings.pet_screen != app.engine.settings.pet_screen;
    let theme_changed = settings.theme != app.engine.settings.theme;
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
    let theme = app.engine.settings.theme;
    if screen_changed {
        app.rescan_screens = true;
    }
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
    drop(app);
    if theme_changed {
        // `System` hands control back to the OS appearance, not a stored copy
        // of whatever it happened to be when the user opened the menu.
        let _ = window.set_theme(theme_to_option(theme));
    }
    Ok(snapshot)
}

fn theme_to_option(theme: ThemeMode) -> Option<tauri::Theme> {
    match theme {
        ThemeMode::Light => Some(tauri::Theme::Light),
        ThemeMode::Dark => Some(tauri::Theme::Dark),
        ThemeMode::System => None,
    }
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

/// Pet Studio: write the editor's design as a pack. Files are written outside
/// the engine lock; only the reload takes it.
#[tauri::command]
async fn save_studio_pack(
    manifest: String,
    strips: Vec<crate::packs::studio::StudioImage>,
    preview: crate::packs::studio::StudioImage,
    window: WebviewWindow,
    state: State<'_, Shared>,
) -> Result<PackSummary, String> {
    if window.label() != "main" {
        return Err("Pets can only be saved from the companion window.".into());
    }
    let dir = locked(&state)?.packs_dir.clone();
    let summary = crate::packs::studio::save(&dir, &manifest, &strips, &preview)?;
    locked(&state)?.engine.reload_packs(&dir);
    Ok(summary)
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
    crate::platform::prepare_process();
    tauri::Builder::default()
        .on_menu_event(handle_menu)
        .invoke_handler(tauri::generate_handler![
            crate::platform::pet_ui::pet_folder,
            crate::platform::pet_ui::pet_folder_drag,
            crate::platform::pet_ui::show_pet_bubble,
            crate::platform::pet_ui::pet_bubble,
            task_list,
            task_start,
            task_rename,
            task_cancel,
            task_approve,
            task_undo,
            task_connections,
            task_save_connections,
            task_secret,
            task_models,
            task_detect_providers,
            task_test_connection,
            task_apple_terms,
            task_apple_open_settings,
            task_account,
            task_pick,
            task_open_url,
            open_assistant,
            get_snapshot,
            pet_action,
            pet_interact,
            set_settings,
            set_app_icon,
            list_packs,
            validate_pack,
            install_pack,
            uninstall_pack,
            new_pack,
            save_studio_pack,
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
            crate::platform::pet_ui::init(app.handle());
            let task_handle = app.handle().clone();
            let task_service =
                crate::tasks::TaskService::new(&config_dir.join("assistant"), move |task| {
                    // The pet whose task this is stops wandering and shows what
                    // the task is doing. The bubble may well be closed, so this
                    // is the only thing telling the user it needs them. The
                    // engine lock is released before anything is emitted.
                    if let Some(pet_id) = task.request.pet_id {
                        if let Some(shared) = task_handle.try_state::<Shared>() {
                            if let Ok(mut app) = shared.lock() {
                                app.engine.set_work(pet_id, task_attention(&task.status));
                            }
                        }
                    }
                    crate::platform::pet_ui::update(&task_handle, &task);
                    let _ = task_handle.emit_to("main", "task:changed", &task);
                    let _ = task_handle.emit(
                        "task:indicator",
                        serde_json::json!({"petId":task.request.pet_id,"status":task.status}),
                    );
                })?;
            app.manage(task_service);
            if let Ok(resources) = app.path().resource_dir() {
                let bundled = resources.join("sidecar");
                if bundled.join("bridge.mjs").exists() {
                    crate::tasks::runtimes::set_bridge(bundled);
                }
            }
            let packs_dir = config_dir.join("packs");
            let (settings, note) = settings::load(&config_dir);
            // Applied before the window can be shown so native scrollbars and
            // form controls never flash the wrong appearance.
            let _ = main.set_theme(theme_to_option(settings.theme));
            let mut notes: Vec<String> = note.into_iter().collect();
            if let Err(error) = builtin::restore(&packs_dir, &settings.uninstalled_builtins) {
                notes.push(error);
            }
            let mut engine = Engine::new(settings, HashMap::new());
            for report in engine.reload_packs(&packs_dir) {
                notes.push(report);
            }
            // Screen geometry is occasionally not ready this early; the
            // update loop re-checks every two seconds, so an empty list is
            // survivable. Only the chosen pet screen goes to the engine, so
            // restored pets land on it from the first tick.
            let found = screens(app.handle()).unwrap_or_default();
            let chosen = chosen_screen(&found, engine.settings.pet_screen.as_deref());
            let initial = chosen
                .map(|screen| vec![pet_monitor(screen)])
                .unwrap_or_default();
            engine.set_monitors(initial);
            let pet_screen = chosen.map(|screen| screen.name.clone());
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
                screens: found,
                pet_screen,
                rescan_screens: false,
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
                    let packs: Vec<_> = paths
                        .iter()
                        .filter(|p| {
                            p.extension().is_some_and(|e| e == "petpack")
                                || p.join("manifest.json").is_file()
                        })
                        .cloned()
                        .collect();
                    let attachments: Vec<_> = paths
                        .iter()
                        .filter(|p| !packs.contains(p))
                        .cloned()
                        .collect();
                    if !packs.is_empty() {
                        install_dropped(&dropped, &dropped_handle, packs);
                    }
                    if !attachments.is_empty() {
                        let _ = dropped_handle.emit_to("main", "assistant:drop", attachments);
                    }
                }
                _ => {}
            });
            // A server orphaned by a previous crash would still hold the model.
            crate::tasks::apple::reap_stale();
            // The dashboard is opened explicitly from the tray or Connections.
            main.hide()?;
            let handle = app.handle().clone();
            std::thread::spawn(move || run_loop(shared, handle));
            Ok(())
        })
        .build(context)
        .expect("Unable to start Ledgeling")
        .run(|_, event| {
            // `fm serve` is a child process holding the on-device model; it
            // must not outlive Ledgeling.
            if matches!(event, tauri::RunEvent::Exit) {
                crate::tasks::apple::shutdown();
            }
        });
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

/// Screen geometry comes from the app handle rather than a window, because a
/// hidden companion window reports no monitors at all.
fn screens(app: &tauri::AppHandle) -> Result<Vec<Screen>, String> {
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
        out.push(Screen {
            info: MonitorInfo {
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
            },
            name: monitor
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Display {}", index + 1)),
        });
    }
    // An empty list is a legitimate answer while the session is locked.
    Ok(out)
}

/// The one screen pets live on: the user's pick when it is connected, else
/// the main display, else the only one there is.
fn chosen_screen<'a>(screens: &'a [Screen], wanted: Option<&str>) -> Option<&'a Screen> {
    wanted
        .and_then(|name| screens.iter().find(|s| s.name == name))
        .or_else(|| screens.iter().find(|s| s.info.is_primary))
        .or_else(|| screens.first())
}

/// Canonical engine form of the chosen screen. The id is forced to 0 because
/// Tauri's monitor ids are list indices — a reshuffle when a display connects
/// would otherwise read as a world change. With a single monitor in engine
/// space, window-edge surfaces outside its work area are clipped away, so
/// confinement falls out of the existing geometry rules rather than a new one.
fn pet_monitor(screen: &Screen) -> MonitorInfo {
    MonitorInfo {
        id: 0,
        ..screen.info
    }
}

/// The one overlay window, on the pet screen. It is created once and moved
/// with `set_position`/`set_size` afterwards: recreating it reloads the
/// webview (a visible flash) and closing the old one is asynchronous, so a
/// rebuild-per-change storm leaves several dying overlays each still showing
/// its last painted scene — pets appeared duplicated across screens.
/// Windows treats a topmost window that exactly covers a monitor as a
/// fullscreen app and stops an auto-hidden taskbar from revealing. One
/// physical pixel short on the right edge is enough to avoid that.
#[cfg(target_os = "windows")]
const OVERLAY_INSET: f64 = 1.;
#[cfg(not(target_os = "windows"))]
const OVERLAY_INSET: f64 = 0.;

fn ensure_overlay(
    app: &tauri::AppHandle,
    monitor: &MonitorInfo,
    hidden: bool,
) -> Option<(WebviewWindow, bool)> {
    const LABEL: &str = "pet-0";
    if let Some(window) = app.get_webview_window(LABEL) {
        // Logical coordinates are pre-converted against the target screen's
        // scale because tao converts `set_size` with the window's *current*
        // screen scale, and the move is applied only after both calls return.
        // Position must land before size so that conversion is the right one.
        let scale = monitor.scale;
        let position =
            tauri::LogicalPosition::new(monitor.bounds.x / scale, monitor.bounds.y / scale);
        let size = tauri::LogicalSize::new(
            (monitor.bounds.width - OVERLAY_INSET) / scale,
            monitor.bounds.height / scale,
        );
        if let Err(error) = window.set_position(position) {
            eprintln!("Ledgeling could not move the overlay: {error}");
        }
        if let Err(error) = window.set_size(size) {
            eprintln!("Ledgeling could not resize the overlay: {error}");
        }
        return Some((window, false));
    }
    let built = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("index.html?pet&monitor=0".into()),
    )
    .title("Ledgeling")
    .inner_size(
        (monitor.bounds.width - OVERLAY_INSET) / monitor.scale,
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
    .accept_first_mouse(true)
    .visible_on_all_workspaces(true)
    .focusable(false)
    .visible(!hidden)
    .build();
    match built {
        Ok(window) => {
            crate::platform::pet_ui::overlay(&window);
            let _ = window.set_ignore_cursor_events(true);
            Some((window, true))
        }
        Err(error) => {
            eprintln!("Ledgeling could not create an overlay: {error}");
            None
        }
    }
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem, Submenu},
        tray::TrayIconBuilder,
    };
    let show = MenuItem::with_id(app, "show", "Open Ledgeling", true, Some("CmdOrCtrl+1"))?;
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
    let quit = MenuItem::with_id(app, "quit", "Quit Ledgeling", true, Some("CmdOrCtrl+Q"))?;
    let submenu =
        Submenu::with_items(app, "Ledgeling", true, &[&show, &pause, &visibility, &quit])?;
    app.set_menu(Menu::with_items(app, &[&submenu])?)?;
    // Tray items are separate so their accelerators do not double-fire.
    let tray_show = MenuItem::with_id(app, "tray-show", "Open Ledgeling", true, None::<&str>)?;
    let tray_pause = MenuItem::with_id(app, "tray-pause", "Pause / resume", true, None::<&str>)?;
    let tray_visibility = MenuItem::with_id(
        app,
        "tray-visibility",
        "Show / hide pets",
        true,
        None::<&str>,
    )?;
    let tray_quit = MenuItem::with_id(app, "tray-quit", "Quit Ledgeling", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&tray_show, &tray_pause, &tray_visibility, &tray_quit],
    )?;
    // macOS tints a template image to match the menu bar, like Apple's own
    // icons; elsewhere a black silhouette would vanish on a dark taskbar.
    #[cfg(target_os = "macos")]
    let tray = TrayIconBuilder::with_id("perch")
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let tray = TrayIconBuilder::with_id("perch").icon(tauri::include_image!("icons/icon.png"));
    tray.tooltip("Ledgeling · a little company")
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
    let diagnostics = std::env::var_os("PERCH_DIAGNOSTICS").is_some();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_live_task_holds_its_pet() {
        assert_eq!(task_attention("running"), Some(Attend::Busy));
        assert_eq!(task_attention("queued"), Some(Attend::Busy));
        assert_eq!(task_attention("undoing"), Some(Attend::Busy));
        assert_eq!(task_attention("waiting"), Some(Attend::NeedsUser));
        // Anything finished lets the pet go, including the statuses a restart
        // recovers tasks into; otherwise one old task pins a pet forever.
        for over in [
            "completed",
            "failed",
            "cancelled",
            "interrupted",
            "undone",
            "",
        ] {
            assert_eq!(task_attention(over), None, "{over} is over");
        }
    }
}
