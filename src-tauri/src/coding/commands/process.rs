use super::{code_only, resolve_worktree, scripts_dir, with};
use crate::{
    app::Shared,
    coding::{apps, launch, notes, ports, pty, run, tools, window, SharedCoding},
};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{ipc::Channel, Manager, State, WebviewWindow};

// -------------------------------------------------------------- terminals

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn code_pty_spawn(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    kind: pty::Kind,
    command: Option<String>,
    title: String,
    cols: u16,
    rows: u16,
    channel: Channel<pty::Output>,
) -> Result<u32, String> {
    code_only(&window)?;
    if kind == pty::Kind::Run {
        return Err("Run commands start from the Run tab.".into());
    }
    let root = resolve_worktree(&state, &worktree)?;
    // An agent that can report its own events does, so its pet shows what it
    // is really doing rather than what its output suggests.
    let (command, env, events) = match command.as_deref() {
        Some(line) if kind == pty::Kind::Agent => {
            let config_dir = window
                .app_handle()
                .state::<Shared>()
                .lock()
                .map_err(|_| "Ledgeling's state is unavailable.")?
                .config_dir
                .clone();
            let prepared = crate::terminal::agent::prepare(line, &config_dir);
            (Some(prepared.command), prepared.env, prepared.events)
        }
        other => (other.map(str::to_string), Vec::new(), None),
    };
    let session = pty::spawn(
        pty::Spawn {
            cwd: &root,
            command: command.as_deref(),
            kind,
            worktree: root.to_string_lossy().into_owned(),
            title,
            cols,
            rows,
            env,
            events,
        },
        channel,
    )?;
    with(&state, |s| Ok(s.ptys.add(session)))
}

#[tauri::command]
pub async fn code_pty(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    id: u32,
    action: String,
    data: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    code_only(&window)?;
    with(&state, |s| match action.as_str() {
        "write" => s
            .ptys
            .get_mut(&id)
            .ok_or("That terminal has closed.")?
            .write(&data),
        "resize" => s
            .ptys
            .get(&id)
            .ok_or("That terminal has closed.")?
            .resize(cols, rows),
        "kill" => {
            if let Some(mut session) = s.ptys.remove(&id) {
                session.kill();
            }
            Ok(())
        }
        "rename" => {
            s.ptys
                .get_mut(&id)
                .ok_or("That terminal has closed.")?
                .title = crate::terminal::commands::clean_title(&data);
            Ok(())
        }
        other => Err(format!("Unknown terminal action `{other}`.")),
    })
}

#[tauri::command]
pub async fn code_pty_attach(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    id: u32,
    channel: Channel<pty::Output>,
) -> Result<(), String> {
    code_only(&window)?;
    with(&state, |s| {
        s.ptys
            .get(&id)
            .ok_or("That terminal has closed.")?
            .attach(channel);
        Ok(())
    })
}

// -------------------------------------------------------------------- run

#[tauri::command]
pub async fn code_run_setup(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<run::RunSetup, String> {
    code_only(&window)?;
    Ok(run::load(&resolve_worktree(&state, &worktree)?))
}

#[tauri::command]
pub async fn code_run_save(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    configs: Vec<run::RunConfig>,
) -> Result<(), String> {
    code_only(&window)?;
    let text = serde_json::to_string(&configs).map_err(|e| e.to_string())?;
    run::validate(&text).map_err(|problems| {
        problems
            .iter()
            .map(|p| {
                format!(
                    "{} {}: {} Expected {}.",
                    p.file, p.path, p.problem, p.expected
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    run::save(&resolve_worktree(&state, &worktree)?, &configs)
}

#[tauri::command]
pub fn code_run_prompt() -> &'static str {
    run::AI_PROMPT
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn code_run_start(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    index: usize,
    cols: u16,
    rows: u16,
    channel: Channel<pty::Output>,
) -> Result<u32, String> {
    code_only(&window)?;
    let root = resolve_worktree(&state, &worktree)?;
    let setup = run::load(&root);
    let config = setup
        .configs
        .get(index)
        .cloned()
        .ok_or("Save how to run this project first.")?;
    let cwd = crate::coding::inside(&root, &config.cwd)?;
    let session = pty::spawn(
        pty::Spawn {
            cwd: &cwd,
            command: Some(&config.command),
            kind: pty::Kind::Run,
            worktree: root.to_string_lossy().into_owned(),
            title: config.name.clone(),
            cols,
            rows,
            env: config.env.into_iter().collect(),
            events: None,
        },
        channel,
    )?;
    with(&state, |s| {
        let id = s.next_pty_id();
        s.ptys.insert(id, session);
        Ok(id)
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortView {
    #[serde(flatten)]
    pub listener: ports::Listener,
    /// The terminal or run tab in this window that owns the port.
    pub session: Option<u32>,
}

/// A desktop app running under one of this window's terminals.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub session: u32,
    pub pid: u32,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    pub ports: Vec<PortView>,
    pub apps: Vec<AppView>,
}

/// Each live terminal's process tree, from one snapshot of the process table.
fn trees(
    state: &State<'_, SharedCoding>,
    table: &[ports::Process],
) -> Result<Vec<(u32, std::collections::HashSet<u32>)>, String> {
    let roots: Vec<(u32, u32)> = with(state, |s| {
        Ok(s.ptys
            .iter()
            .filter(|(_, p)| p.alive())
            .filter_map(|(id, p)| p.pid.map(|pid| (*id, pid)))
            .collect())
    })?;
    Ok(roots
        .into_iter()
        .map(|(session, pid)| (session, ports::descendants(table, pid)))
        .collect())
}

/// What the window's terminals have opened: listening ports, and desktop
/// apps such as one under `tauri dev`, each credited to its terminal.
#[tauri::command]
pub async fn code_live(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
) -> Result<Live, String> {
    code_only(&window)?;
    let listeners = ports::listeners()?;
    let table = ports::processes();
    let trees = trees(&state, &table)?;
    let mut owners: HashMap<u32, u32> = HashMap::new();
    for (session, pids) in &trees {
        for pid in pids {
            owners.insert(*pid, *session);
        }
    }
    let windowed = apps::windowed(&owners.keys().copied().collect());
    let mut apps: Vec<AppView> = windowed
        .into_iter()
        .map(|pid| AppView {
            session: owners[&pid],
            pid,
            name: table
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default(),
        })
        .collect();
    apps.sort_by_key(|a| (a.session, a.pid));
    Ok(Live {
        ports: listeners
            .into_iter()
            .map(|listener| PortView {
                session: owners.get(&listener.pid).copied(),
                listener,
            })
            .collect(),
        apps,
    })
}

/// Bring forward an app one of this window's terminals started. Other
/// processes are refused, so the window cannot raise arbitrary apps.
#[tauri::command]
pub async fn code_activate(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    pid: u32,
) -> Result<(), String> {
    code_only(&window)?;
    let table = ports::processes();
    if !trees(&state, &table)?
        .iter()
        .any(|(_, pids)| pids.contains(&pid))
    {
        return Err("That app was not started here.".into());
    }
    crate::app::on_main(&app, move |_| apps::activate(pid))
        .ok_or("The app could not be brought forward.")?
}

#[tauri::command]
pub async fn code_launch(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<Option<launch::NativeApp>, String> {
    code_only(&window)?;
    Ok(launch::detect(&resolve_worktree(&state, &worktree)?))
}

#[tauri::command]
pub async fn code_launch_open(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<(), String> {
    code_only(&window)?;
    launch::open_built(&resolve_worktree(&state, &worktree)?)
}

#[tauri::command]
pub async fn code_notes(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    after: u64,
) -> Result<Vec<notes::Note>, String> {
    code_only(&window)?;
    with(&state, |s| Ok(s.notes.for_worktree(&worktree, after)))
}

/// A note the window noticed itself: a port opening, a pet taking over.
#[tauri::command]
pub async fn code_note(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    text: String,
    tone: notes::Tone,
) -> Result<(), String> {
    code_only(&window)?;
    let tree = resolve_worktree(&state, &worktree)?
        .to_string_lossy()
        .into_owned();
    let text: String = text.chars().take(300).collect();
    with(&state, |s| {
        s.notes.add(&tree, text, tone);
        Ok(())
    })
}

#[tauri::command]
pub async fn code_layouts(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
) -> Result<HashMap<String, serde_json::Value>, String> {
    code_only(&window)?;
    with(&state, |s| Ok(s.layouts.clone()))
}

#[tauri::command]
pub async fn code_layout_save(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    layout: serde_json::Value,
) -> Result<(), String> {
    code_only(&window)?;
    with(&state, |s| {
        if layout.is_null() {
            s.layouts.remove(&worktree);
        } else {
            s.layouts.insert(worktree, layout);
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn code_open_url(window: WebviewWindow, url: String) -> Result<(), String> {
    code_only(&window)?;
    crate::tasks::commands::open_web_url(&url)
}

#[tauri::command]
pub async fn code_tools(
    app: tauri::AppHandle,
    window: WebviewWindow,
) -> Result<Vec<tools::Tool>, String> {
    code_only(&window)?;
    Ok(tools::detect(scripts_dir(&app)))
}

/// Runs a bundled install script in a visible terminal. The user pressed
/// Install on a row that shows the script; nothing is installed silently.
#[tauri::command]
pub async fn code_install(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    tool: String,
    cols: u16,
    rows: u16,
    channel: Channel<pty::Output>,
) -> Result<u32, String> {
    code_only(&window)?;
    let script = tools::detect(scripts_dir(&app))
        .into_iter()
        .find(|t| t.id == tool)
        .and_then(|t| t.installer)
        .ok_or("There is no installer for that tool.")?;
    let line = if cfg!(windows) {
        format!("powershell -NoProfile -ExecutionPolicy Bypass -File \"{script}\"")
    } else {
        format!("/bin/sh '{}'", script.replace('\'', "'\\''"))
    };
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let session = pty::spawn(
        pty::Spawn {
            cwd: &home,
            command: Some(&line),
            kind: pty::Kind::Shell,
            worktree: String::new(),
            title: format!("Install {tool}"),
            cols,
            rows,
            env: Vec::new(),
            events: None,
        },
        channel,
    )?;
    with(&state, |s| {
        let id = s.next_pty_id();
        s.ptys.insert(id, session);
        Ok(id)
    })
}

#[tauri::command]
pub async fn code_window(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    action: String,
) -> Result<bool, String> {
    code_only(&window)?;
    match action.as_str() {
        "pin" => {
            let pinned = with(&state, |s| {
                s.store.data.pinned = !s.store.data.pinned;
                s.store.save()?;
                Ok(s.store.data.pinned)
            })?;
            window
                .set_always_on_top(pinned)
                .map_err(|e| e.to_string())?;
            Ok(pinned)
        }
        "focus" => {
            let focus = with(&state, |s| {
                s.store.data.focus = !s.store.data.focus;
                s.store.save()?;
                Ok(s.store.data.focus)
            })?;
            window::set_focus_mode(&window, focus)?;
            Ok(focus)
        }
        "minimize" => window.minimize().map(|_| false).map_err(|e| e.to_string()),
        "close" => window.close().map(|_| false).map_err(|e| e.to_string()),
        other => Err(format!("Unknown window action `{other}`.")),
    }
}
