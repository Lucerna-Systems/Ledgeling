use super::{menus::set_menus, App, Shared, Snapshot};
use crate::{
    packs::{
        diagnostics::Report, loader, manifest::Event, LocomotionClass, PackListing, PackSummary,
    },
    settings::{Settings, ThemeMode},
    windows_mgr::hit_test,
};
use std::path::{Path, PathBuf};
use tauri::{Manager, State, WebviewWindow};

pub fn locked<'a>(state: &'a State<'_, Shared>) -> Result<std::sync::MutexGuard<'a, App>, String> {
    state
        .inner()
        .lock()
        .map_err(|_| "Ledgeling's state is unavailable. Restart the app.".to_string())
}

pub fn theme_to_option(theme: ThemeMode) -> Option<tauri::Theme> {
    match theme {
        ThemeMode::Light => Some(tauri::Theme::Light),
        ThemeMode::Dark => Some(tauri::Theme::Dark),
        ThemeMode::System => None,
    }
}

// ------------------------------------------------------------------- commands

#[tauri::command]
pub async fn get_snapshot(state: State<'_, Shared>) -> Result<Snapshot, String> {
    Ok(locked(&state)?.snapshot())
}

#[tauri::command]
pub async fn pet_action(
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
pub async fn pet_interact(kind: String, state: State<'_, Shared>) -> Result<(), String> {
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
pub fn set_app_icon(png: Vec<u8>, window: WebviewWindow) -> Result<(), String> {
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
pub async fn set_settings(
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
    let developer_changed = settings.developer_mode != app.engine.settings.developer_mode;
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
        crate::platform::pet_ui::seed_folders(app.engine.pet_folders());
    }
    app.dirty = true;
    let snapshot = app.snapshot();
    let _ = app.persist();
    let developer = app.engine.settings.developer_mode;
    drop(app);
    if developer_changed {
        let handle = window.app_handle();
        #[cfg(feature = "coding")]
        if !developer {
            crate::coding::commands::shut_down(handle);
        }
        if let Err(error) = set_menus(handle, developer) {
            eprintln!("Ledgeling could not update its menus: {error}");
        }
    }
    if theme_changed {
        // `System` hands control back to the OS appearance, not a stored copy
        // of whatever it happened to be when the user opened the menu.
        let _ = window.set_theme(theme_to_option(theme));
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn list_packs(state: State<'_, Shared>) -> Result<Vec<PackListing>, String> {
    let dir = locked(&state)?.packs_dir.clone();
    Ok(loader::scan(&dir))
}

#[tauri::command]
pub async fn validate_pack(path: String, state: State<'_, Shared>) -> Result<Report, String> {
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
pub async fn install_pack(path: String, state: State<'_, Shared>) -> Result<PackSummary, String> {
    let dir = locked(&state)?.packs_dir.clone();
    let summary = loader::install(Path::new(&path), &dir)?;
    let mut app = locked(&state)?;
    app.engine.reload_packs(&dir);
    Ok(summary)
}

#[tauri::command]
pub async fn uninstall_pack(id: String, state: State<'_, Shared>) -> Result<(), String> {
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
pub async fn new_pack(
    class: String,
    name: String,
    state: State<'_, Shared>,
) -> Result<String, String> {
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
pub async fn save_studio_pack(
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
pub async fn export_pack(id: String, state: State<'_, Shared>) -> Result<String, String> {
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
pub async fn spawn_pet(pack_id: String, state: State<'_, Shared>) -> Result<u64, String> {
    let mut app = locked(&state)?;
    let id = app.engine.spawn(&pack_id)?;
    app.dirty = true;
    let _ = app.persist();
    Ok(id)
}

#[tauri::command]
pub async fn despawn_pet(pet_id: u64, state: State<'_, Shared>) -> Result<(), String> {
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
pub async fn fire_event(name: String, state: State<'_, Shared>) -> Result<(), String> {
    let event = Event::parse(&name).ok_or_else(|| format!("Unknown event `{name}`."))?;
    let mut app = locked(&state)?;
    for pet in &mut app.engine.pets {
        pet.queue(event);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_lab(active: bool, state: State<'_, Shared>) -> Result<(), String> {
    locked(&state)?.lab_active = active;
    Ok(())
}

/// An overlay has mounted and registered its listeners; ask the update loop to
/// re-send the spawn payloads the first ticks delivered too early.
#[tauri::command]
pub fn pet_surface_ready(monitor: u64, state: State<'_, Shared>) {
    if let Ok(mut app) = locked(&state) {
        if !app.atlas_recheck.contains(&monitor) {
            app.atlas_recheck.push(monitor);
        }
    }
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle, window: WebviewWindow, state: State<'_, Shared>) {
    if window.label() == "main" {
        if let Ok(mut shared) = state.inner().lock() {
            let _ = shared.persist();
        }
        app.exit(0);
    }
}
