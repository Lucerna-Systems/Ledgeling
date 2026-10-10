//! Terminal mode: a pet's terminal window, from the pet's own menu.
use super::{agent, pty, window, Context, SharedTerminal, Terminal};
use crate::app::Shared;
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf};
use tauri::{ipc::Channel, Emitter, Manager, State, WebviewWindow};

fn slot<'a>(
    state: &'a State<'_, SharedTerminal>,
) -> Result<std::sync::MutexGuard<'a, Terminal>, String> {
    state
        .inner()
        .lock()
        .map_err(|_| "Terminal mode is unavailable. Restart Ledgeling.".to_string())
}

/// Only the terminal window and the pet bubble, which shows a terminal in its
/// own panel, may drive terminals.
fn terminal_only(window: &WebviewWindow) -> Result<(), String> {
    if matches!(window.label(), window::LABEL | "bubble") {
        Ok(())
    } else {
        Err("Terminals can only be used from a pet's terminal.".into())
    }
}

fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.state::<Shared>()
        .lock()
        .map(|state| state.config_dir.clone())
        .map_err(|_| "Ledgeling's state is unavailable.".into())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn pet_name(app: &tauri::AppHandle, pet: u64) -> String {
    app.try_state::<Shared>()
        .and_then(|shared| {
            shared.lock().ok().and_then(|state| {
                state
                    .engine
                    .pets
                    .iter()
                    .find(|p| p.id == pet)
                    .map(|p| p.pack.manifest.name.clone())
            })
        })
        .unwrap_or_else(|| "Your pet".into())
}

/// Where a pet's terminals start: its working folder, or the home folder when
/// it has none. Remembers the folder as one a terminal may start in, and makes
/// sure the pet will show what its agents are doing.
pub fn context_for(app: &tauri::AppHandle, pet: u64) -> Result<Context, String> {
    let folder = crate::platform::pet_ui::folder(pet)
        .map(PathBuf::from)
        .or_else(home)
        .ok_or("No folder is available to start a terminal in.")?;
    let folder = folder
        .canonicalize()
        .map_err(|_| "That folder is no longer available.".to_string())?;
    let context = Context {
        pet_id: pet,
        pet_name: pet_name(app, pet),
        folder: folder.to_string_lossy().into_owned(),
    };
    let terminal = app.state::<SharedTerminal>().inner().clone();
    let start_watching = {
        let mut guard = terminal
            .lock()
            .map_err(|_| "Terminal mode is unavailable.")?;
        guard.folders.insert(context.folder.clone(), pet);
        !std::mem::replace(&mut guard.watching, true)
    };
    if start_watching {
        watch_agents(app.clone(), terminal);
    }
    Ok(context)
}

/// Opens the terminal window for a pet: the focused version of the terminal in
/// its bubble.
pub fn open_window(app: &tauri::AppHandle, pet: u64) -> Result<(), String> {
    let context = context_for(app, pet)?;
    let title = format!("{} · Terminal", context.pet_name);
    app.state::<SharedTerminal>()
        .lock()
        .map_err(|_| "Terminal mode is unavailable.")?
        .context = Some(context);
    let handle = app.clone();
    crate::app::on_main(app, move |app| window::open(app, &title).map(|_| ()))
        .ok_or("The terminal window could not be opened.")??;
    let _ = handle.emit_to(window::LABEL, "terminal:changed", ());
    Ok(())
}

/// Pets show what their agent is doing: working while it works, asking when it
/// needs an answer. Only pets this loop marked are released, so a pet's chat
/// task keeps its own state.
fn watch_agents(app: tauri::AppHandle, terminal: SharedTerminal) {
    std::thread::spawn(move || {
        let mut marked: HashMap<u64, crate::pet::Attend> = HashMap::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(700));
            let wanted = {
                let Ok(guard) = terminal.lock() else { break };
                let mut wanted: HashMap<u64, crate::pet::Attend> = HashMap::new();
                for session in guard.sessions.values() {
                    let (Some(attend), Some(pet)) =
                        (session.attention(), guard.folders.get(&session.worktree))
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
                for pet in marked.keys().filter(|p| !wanted.contains_key(p)) {
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
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalState {
    pub context: Option<Context>,
    pub sessions: Vec<pty::SessionView>,
    /// The coding agents installed on the user's PATH, for one-click starts.
    pub agents: Vec<agent::Installed>,
    pub layouts: HashMap<String, serde_json::Value>,
}

#[tauri::command]
pub async fn terminal_state(
    window: WebviewWindow,
    state: State<'_, SharedTerminal>,
    pet: Option<u64>,
) -> Result<TerminalState, String> {
    terminal_only(&window)?;
    // The bubble names its pet; the terminal window follows whichever pet
    // opened it last.
    let asked = match pet {
        Some(pet) => Some(context_for(window.app_handle(), pet)?),
        None => None,
    };
    let (context, sessions, layouts) = {
        let guard = slot(&state)?;
        let sessions = guard.sessions.iter().map(|(id, s)| s.view(*id)).collect();
        (
            asked.or_else(|| guard.context.clone()),
            sessions,
            guard.layouts.clone(),
        )
    };
    Ok(TerminalState {
        context,
        sessions,
        agents: agent::installed(),
        layouts,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn terminal_spawn(
    window: WebviewWindow,
    state: State<'_, SharedTerminal>,
    folder: String,
    kind: pty::Kind,
    command: Option<String>,
    title: String,
    cols: u16,
    rows: u16,
    channel: Channel<pty::Output>,
) -> Result<u32, String> {
    terminal_only(&window)?;
    if kind == pty::Kind::Run {
        return Err("Run commands start from coding mode.".into());
    }
    if !slot(&state)?.folders.contains_key(&folder) {
        return Err("Open the terminal from a pet to use that folder.".into());
    }
    let (command, env, events) = match command.as_deref() {
        Some(line) if kind == pty::Kind::Agent => {
            let prepared = agent::prepare(line, &config_dir(window.app_handle())?);
            (Some(prepared.command), prepared.env, prepared.events)
        }
        other => (other.map(str::to_string), Vec::new(), None),
    };
    let session = pty::spawn(
        pty::Spawn {
            cwd: std::path::Path::new(&folder),
            command: command.as_deref(),
            kind,
            worktree: folder.clone(),
            title,
            cols,
            rows,
            env,
            events,
        },
        channel,
    )?;
    Ok(slot(&state)?.sessions.add(session))
}

#[tauri::command]
pub async fn terminal_io(
    window: WebviewWindow,
    state: State<'_, SharedTerminal>,
    id: u32,
    action: String,
    data: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    terminal_only(&window)?;
    let mut guard = slot(&state)?;
    match action.as_str() {
        "write" => guard
            .sessions
            .get_mut(&id)
            .ok_or("That terminal has closed.")?
            .write(&data),
        "resize" => guard
            .sessions
            .get(&id)
            .ok_or("That terminal has closed.")?
            .resize(cols, rows),
        "kill" => {
            if let Some(mut session) = guard.sessions.remove(&id) {
                session.kill();
            }
            Ok(())
        }
        // Tabs can be renamed; the name is kept here so it survives the
        // bubble closing and reopening.
        "rename" => {
            guard
                .sessions
                .get_mut(&id)
                .ok_or("That terminal has closed.")?
                .title = clean_title(&data);
            Ok(())
        }
        other => Err(format!("Unknown terminal action `{other}`.")),
    }
}

#[tauri::command]
pub async fn terminal_attach(
    window: WebviewWindow,
    state: State<'_, SharedTerminal>,
    id: u32,
    channel: Channel<pty::Output>,
) -> Result<(), String> {
    terminal_only(&window)?;
    slot(&state)?
        .sessions
        .get(&id)
        .ok_or("That terminal has closed.")?
        .attach(channel);
    Ok(())
}

#[tauri::command]
pub async fn terminal_layout_save(
    window: WebviewWindow,
    state: State<'_, SharedTerminal>,
    folder: String,
    layout: Option<serde_json::Value>,
) -> Result<(), String> {
    terminal_only(&window)?;
    let mut guard = slot(&state)?;
    match layout {
        Some(layout) => guard.layouts.insert(folder, layout),
        None => guard.layouts.remove(&folder),
    };
    Ok(())
}

/// A tab name: trimmed, one line, and short enough to fit in a tab.
pub fn clean_title(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(40)
        .collect()
}

/// A link clicked in a terminal. Web links only, as everywhere else.
#[tauri::command]
pub async fn terminal_open_url(window: WebviewWindow, url: String) -> Result<(), String> {
    terminal_only(&window)?;
    crate::tasks::commands::open_web_url(&url)
}
