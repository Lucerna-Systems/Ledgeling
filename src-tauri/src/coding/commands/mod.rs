pub mod fs_ops;
pub mod process;
pub mod projects;

use super::{git, notes, window, CodingService, SharedCoding};
use crate::app::Shared;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::MutexGuard,
};
use tauri::{Emitter, Manager, State, WebviewWindow};

// Re-export command functions so existing registrations remain intact
pub use fs_ops::*;
pub use process::*;
pub use projects::*;

pub(super) fn code_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == window::LABEL {
        Ok(())
    } else {
        Err("This belongs to the coding window.".into())
    }
}

pub(super) fn slot<'a>(
    state: &'a State<'_, SharedCoding>,
) -> Result<MutexGuard<'a, Option<CodingService>>, String> {
    state
        .inner()
        .lock()
        .map_err(|_| "Coding mode is unavailable. Restart Ledgeling.".to_string())
}

pub(super) fn with<T>(
    state: &State<'_, SharedCoding>,
    work: impl FnOnce(&mut CodingService) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = slot(state)?;
    let service = guard
        .as_mut()
        .ok_or("Turn on Developer tools in Settings to use coding mode.")?;
    work(service)
}

pub(super) fn developer_mode(app: &tauri::AppHandle) -> bool {
    app.try_state::<Shared>()
        .and_then(|s| s.lock().ok().map(|a| a.engine.settings.developer_mode))
        .unwrap_or(false)
}

/// Live pets as (id, pack), read and released before coding state is touched.
pub(super) fn live_pets(app: &tauri::AppHandle) -> Vec<(u64, String)> {
    app.try_state::<Shared>()
        .and_then(|s| {
            s.lock().ok().map(|a| {
                a.engine
                    .pets
                    .iter()
                    .map(|p| (p.id, p.pack.manifest.id.clone()))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// A worktree the user added, as a canonical path. Anything else is refused,
/// so the window cannot be pointed at arbitrary folders.
pub(crate) fn resolve_worktree(
    state: &State<'_, SharedCoding>,
    path: &str,
) -> Result<PathBuf, String> {
    let projects = with(state, |s| Ok(s.store.data.projects.clone()))?;
    let wanted = Path::new(path)
        .canonicalize()
        .map_err(|_| "That worktree no longer exists.".to_string())?;
    for project in projects {
        if Path::new(&project.path) == wanted {
            return Ok(wanted);
        }
        if let Ok(trees) = git::worktrees(Path::new(&project.path)) {
            if trees
                .iter()
                .any(|t| Path::new(&t.path).canonicalize().ok().as_deref() == Some(&wanted))
            {
                return Ok(wanted);
            }
        }
    }
    Err("That folder is not one of your coding projects.".into())
}

pub(crate) use resolve_worktree as worktree;

pub(super) fn scripts_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    let bundled = app.path().resource_dir().ok().map(|r| r.join("dev-addons"));
    // In development the scripts are read straight from the source tree.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/dev-addons");
    bundled
        .filter(|p| p.is_dir())
        .or_else(|| source.is_dir().then_some(source))
}

// ------------------------------------------------------------------ opening

/// Build the service if needed and show the window. Callable from the
/// companion, the pet bubble and the tray, but only with Developer tools on.
pub fn open_window(
    app: &tauri::AppHandle,
    focus: Option<(PathBuf, Option<u64>)>,
) -> Result<(), String> {
    if !developer_mode(app) {
        return Err("Turn on Developer tools in Settings to use coding mode.".into());
    }
    let coding = app.state::<SharedCoding>().inner().clone();
    let config_dir = app
        .state::<Shared>()
        .lock()
        .map_err(|_| "Ledgeling's state is unavailable.")?
        .config_dir
        .clone();
    let started = {
        let mut guard = coding.lock().map_err(|_| "Coding mode is unavailable.")?;
        let fresh = guard.is_none();
        if fresh {
            *guard = Some(CodingService::new(&config_dir));
        }
        fresh
    };
    if started {
        start_attention_loop(app.clone(), coding.clone());
    }
    if let Some((folder, pet)) = focus {
        let root = git::main_root(&folder)?;
        let tree = git::run(&folder, &["rev-parse", "--show-toplevel"])
            .map(|t| PathBuf::from(t.trim()))
            .ok()
            .and_then(|p| p.canonicalize().ok())
            .unwrap_or_else(|| root.clone());
        let live = live_pets(app);
        let mut guard = coding.lock().map_err(|_| "Coding mode is unavailable.")?;
        if let Some(service) = guard.as_mut() {
            service.store.add_project(&root);
            let tree_text = tree.to_string_lossy().into_owned();
            if let Some(pet) = pet.and_then(|id| live.iter().find(|(i, _)| *i == id)) {
                service.store.bind(&tree_text, pet.0, &pet.1, &live);
            }
            service.store.data.selected = Some(tree_text);
            let _ = service.store.save();
        }
    }
    let (saved, pinned, focus) = coding
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|s| {
                let data = &s.store.data;
                (data.window, data.pinned, data.focus)
            })
        })
        .unwrap_or((None, false, false));
    let (screens, pet_screen) = {
        let shared = app.state::<Shared>();
        let state = shared
            .lock()
            .map_err(|_| "Ledgeling's state is unavailable.")?;
        let screens: Vec<_> = state.screens.iter().map(|s| s.info).collect();
        let pet = state
            .engine
            .monitors
            .first()
            .copied()
            .or_else(|| screens.first().copied())
            .ok_or("No screen is available right now.")?;
        (screens, pet)
    };
    let (rect, scale) = window::placement(saved, &screens, &pet_screen);
    let handle = app.clone();
    let made = crate::app::on_main(app, move |app| {
        window::open(app, coding, rect, scale, pinned, focus).map(|_| ())
    })
    .ok_or("The coding window could not be opened.")?;
    made?;
    let _ = handle.emit_to(window::LABEL, "code:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn code_open(app: tauri::AppHandle, pet_id: Option<u64>) -> Result<(), String> {
    let folder = pet_id
        .and_then(crate::platform::pet_ui::folder)
        .map(PathBuf::from);
    open_window(&app, folder.map(|f| (f, pet_id)))
}

/// Turning Developer tools off stops everything coding mode started.
pub fn shut_down(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(window::LABEL) {
        let _ = window.destroy();
    }
    let coding = app.state::<SharedCoding>().inner().clone();
    let service = coding.lock().ok().and_then(|mut g| g.take());
    if let Some(service) = service {
        let _ = service.store.save();
        drop(service);
    }
}

/// Pets show what their worktree's agent is doing: working while it prints,
/// asking when it rings the bell. Only pets this loop marked are released,
/// so a pet's chat task keeps its own state.
fn start_attention_loop(app: tauri::AppHandle, coding: SharedCoding) {
    std::thread::spawn(move || {
        let mut marked: HashMap<u64, crate::pet::Attend> = HashMap::new();
        let mut seen: HashMap<u32, notes::Seen> = HashMap::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(700));
            let live = live_pets(&app);
            let wanted = {
                let Ok(mut guard) = coding.lock() else { break };
                let Some(service) = guard.as_mut() else { break };
                // The pets' side chats hear what happened in their terminals.
                let mut now_seen = HashMap::new();
                for (id, session) in service.ptys.iter() {
                    let now = notes::Seen::of(session);
                    if let Some((text, tone)) = notes::change(seen.get(id), &now) {
                        service.notes.add(&session.worktree, text, tone);
                    }
                    now_seen.insert(*id, now);
                }
                seen = now_seen;
                let owners = service.store.resolve(&live);
                let mut wanted: HashMap<u64, crate::pet::Attend> = HashMap::new();
                for session in service.ptys.values() {
                    let (Some(attend), Some(pet)) =
                        (session.attention(), owners.get(&session.worktree))
                    else {
                        continue;
                    };
                    let entry = wanted.entry(*pet).or_insert(attend);
                    if attend == crate::pet::Attend::NeedsUser {
                        *entry = attend;
                    }
                }
                wanted
            };
            if wanted == marked {
                continue;
            }
            let Some(shared) = app.try_state::<Shared>() else {
                break;
            };
            if let Ok(mut state) = shared.lock() {
                for (pet, _) in marked.iter().filter(|(p, _)| !wanted.contains_key(p)) {
                    state.engine.set_work(*pet, None);
                }
                for (pet, attend) in &wanted {
                    if marked.get(pet) != Some(attend) {
                        state.engine.set_work(*pet, Some(*attend));
                    }
                }
            }
            marked = wanted;
        }
        // Coding mode was turned off: hand every marked pet back.
        if let Some(shared) = app.try_state::<Shared>() {
            if let Ok(mut state) = shared.lock() {
                for pet in marked.keys() {
                    state.engine.set_work(*pet, None);
                }
            }
        }
    });
}
