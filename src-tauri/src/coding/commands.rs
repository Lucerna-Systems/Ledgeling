//! Tauri commands for coding mode. Everything except opening the window is
//! answered only to the `code` window, and every path is checked against the
//! worktrees of projects the user added.
use super::{
    apps, fs, git, launch, notes, ports, pty, run,
    store::{Project, StoreData},
    tools, watch, window, CodingService, SharedCoding,
};
use crate::app::Shared;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::MutexGuard,
};
use tauri::{ipc::Channel, Emitter, Manager, State, WebviewWindow};

pub(super) fn code_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == window::LABEL {
        Ok(())
    } else {
        Err("This belongs to the coding window.".into())
    }
}

fn slot<'a>(
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

fn developer_mode(app: &tauri::AppHandle) -> bool {
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
pub(super) fn worktree(state: &State<'_, SharedCoding>, path: &str) -> Result<PathBuf, String> {
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

// -------------------------------------------------------------- workspaces

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeView {
    #[serde(flatten)]
    pub tree: git::Worktree,
    pub name: String,
    pub pet_id: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    #[serde(flatten)]
    pub project: Project,
    pub worktrees: Vec<WorktreeView>,
    pub problem: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeState {
    pub projects: Vec<ProjectView>,
    pub selected: Option<String>,
    pub pinned: bool,
    pub focus: bool,
    pub sessions: Vec<pty::SessionView>,
    /// The newest pet note's id, so the window knows when to fetch more.
    pub notes: u64,
}

#[tauri::command]
pub async fn code_state(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
) -> Result<CodeState, String> {
    code_only(&window)?;
    let live = live_pets(&app);
    let (data, owners, sessions, latest): (StoreData, _, _, _) = with(&state, |s| {
        let sessions = s.ptys.iter().map(|(id, p)| p.view(*id)).collect::<Vec<_>>();
        Ok((
            s.store.data.clone(),
            s.store.resolve(&live),
            sessions,
            s.notes.latest(),
        ))
    })?;
    let mut projects = Vec::new();
    for project in data.projects {
        let (worktrees, problem) = match git::worktrees(Path::new(&project.path)) {
            Ok(trees) => (trees, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let worktrees = worktrees
            .into_iter()
            .map(|tree| {
                let canonical = Path::new(&tree.path)
                    .canonicalize()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| tree.path.clone());
                WorktreeView {
                    name: if tree.main {
                        project.name.clone()
                    } else {
                        Path::new(&tree.path)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    },
                    pet_id: owners.get(&canonical).copied(),
                    tree: git::Worktree {
                        path: canonical,
                        ..tree
                    },
                }
            })
            .collect();
        projects.push(ProjectView {
            project,
            worktrees,
            problem,
        });
    }
    let mut sessions = sessions;
    sessions.sort_by_key(|s| s.id);
    Ok(CodeState {
        projects,
        selected: data.selected,
        pinned: data.pinned,
        focus: data.focus,
        sessions,
        notes: latest,
    })
}

#[tauri::command]
pub async fn code_add_project(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
) -> Result<Option<String>, String> {
    code_only(&window)?;
    let Some(picked) = rfd::AsyncFileDialog::new()
        .set_title("Choose a project folder")
        .set_parent(&window)
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let folder = picked.path().to_path_buf();
    let root = match git::main_root(&folder) {
        Ok(root) => root,
        Err(_) => {
            // A plain folder becomes a repository so worktrees and diffs work.
            git::run(&folder, &["init", "-q"])?;
            folder.canonicalize().map_err(|e| e.to_string())?
        }
    };
    with(&state, |s| {
        s.store.add_project(&root);
        let text = root.to_string_lossy().into_owned();
        s.store.data.selected = Some(text.clone());
        s.store.save()?;
        Ok(Some(text))
    })
}

#[tauri::command]
pub async fn code_remove_project(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    path: String,
) -> Result<(), String> {
    code_only(&window)?;
    with(&state, |s| {
        s.store.remove_project(&path);
        s.store.save()
    })
}

#[tauri::command]
pub async fn code_select(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<(), String> {
    code_only(&window)?;
    with(&state, |s| {
        s.store.data.selected = Some(worktree);
        s.store.save()
    })
}

#[tauri::command]
pub async fn code_branches(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    project: String,
) -> Result<Vec<String>, String> {
    code_only(&window)?;
    let root = worktree(&state, &project)?;
    git::branches(&root)
}

#[tauri::command]
pub async fn code_worktree_create(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    project: String,
    branch: String,
    base: String,
    pet_id: Option<u64>,
) -> Result<String, String> {
    code_only(&window)?;
    let root = worktree(&state, &project)?;
    let created = git::add_worktree(&root, branch.trim(), base.trim())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let text = created.to_string_lossy().into_owned();
    let live = live_pets(&app);
    with(&state, |s| {
        let pet = match pet_id {
            Some(id) => live.iter().find(|(i, _)| *i == id).cloned(),
            None => s.store.free_pet(&live),
        };
        if let Some((id, pack)) = pet {
            s.store.bind(&text, id, &pack, &live);
        }
        s.store.data.selected = Some(text.clone());
        s.store.save()
    })?;
    Ok(text)
}

#[tauri::command]
pub async fn code_worktree_remove(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    project: String,
    path: String,
    force: bool,
) -> Result<(), String> {
    code_only(&window)?;
    let root = worktree(&state, &project)?;
    let tree = worktree(&state, &path)?;
    if tree == root {
        return Err("The main checkout is the project itself; remove the project instead.".into());
    }
    let text = tree.to_string_lossy().into_owned();
    with(&state, |s| {
        let ids: Vec<u32> = s
            .ptys
            .iter()
            .filter(|(_, p)| p.worktree == text)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(mut session) = s.ptys.remove(&id) {
                session.kill();
            }
        }
        s.watchers.remove(&tree);
        Ok(())
    })?;
    git::remove_worktree(&root, &text, force)?;
    with(&state, |s| {
        s.store.unbind(&text);
        if s.store.data.selected.as_deref() == Some(text.as_str()) {
            s.store.data.selected = Some(root.to_string_lossy().into_owned());
        }
        s.store.save()
    })
}

#[tauri::command]
pub async fn code_bind(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    pet_id: Option<u64>,
) -> Result<(), String> {
    code_only(&window)?;
    let tree = self::worktree(&state, &worktree)?
        .to_string_lossy()
        .into_owned();
    let live = live_pets(&app);
    with(&state, |s| {
        match pet_id.and_then(|id| live.iter().find(|(i, _)| *i == id)) {
            Some((id, pack)) => s.store.bind(&tree, *id, pack, &live),
            None => s.store.unbind(&tree),
        }
        s.store.save()
    })
}

// ---------------------------------------------------------------- changes

#[tauri::command]
pub async fn code_status(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<git::Status, String> {
    code_only(&window)?;
    git::status(&self::worktree(&state, &worktree)?)
}

#[tauri::command]
pub async fn code_original(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    path: String,
    index: bool,
) -> Result<String, String> {
    code_only(&window)?;
    let root = self::worktree(&state, &worktree)?;
    super::inside(&root, &path)?;
    Ok(git::original(&root, &path, index))
}

#[tauri::command]
pub async fn code_git(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    action: String,
    paths: Vec<String>,
    untracked: Vec<String>,
    message: String,
) -> Result<String, String> {
    code_only(&window)?;
    let root = self::worktree(&state, &worktree)?;
    for path in paths.iter().chain(&untracked) {
        super::inside(&root, path)?;
    }
    match action.as_str() {
        "stage" => git::stage(&root, &paths).map(|_| String::new()),
        "unstage" => git::unstage(&root, &paths).map(|_| String::new()),
        "discard" => git::discard(&root, &paths, &untracked).map(|_| String::new()),
        "commit" => git::commit(&root, &message, paths.is_empty()),
        other => Err(format!("Unknown git action `{other}`.")),
    }
}

// ------------------------------------------------------------------ files

#[tauri::command]
pub async fn code_list(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    dir: String,
) -> Result<Vec<fs::Entry>, String> {
    code_only(&window)?;
    fs::list(&self::worktree(&state, &worktree)?, &dir)
}

#[tauri::command]
pub async fn code_read(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    path: String,
) -> Result<fs::FileText, String> {
    code_only(&window)?;
    fs::read(&self::worktree(&state, &worktree)?, &path)
}

#[tauri::command]
pub async fn code_write(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    path: String,
    text: String,
    expected: u64,
) -> Result<u64, String> {
    code_only(&window)?;
    fs::write(&self::worktree(&state, &worktree)?, &path, &text, expected)
}

#[tauri::command]
pub async fn code_file_op(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    op: String,
    path: String,
    to: String,
) -> Result<(), String> {
    code_only(&window)?;
    let root = self::worktree(&state, &worktree)?;
    match op.as_str() {
        "create-file" => fs::create(&root, &path, false),
        "create-folder" => fs::create(&root, &path, true),
        "rename" => fs::rename(&root, &path, &to),
        "delete" => fs::delete(&root, &path),
        "reveal" => {
            let target = super::inside(&root, &path)?;
            let folder = if target.is_dir() {
                target
            } else {
                target.parent().map(Path::to_path_buf).unwrap_or(root)
            };
            open::that(folder).map_err(|_| "Could not open the folder.".into())
        }
        other => Err(format!("Unknown file action `{other}`.")),
    }
}

#[tauri::command]
pub async fn code_watch(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    channel: Channel<watch::Changes>,
) -> Result<(), String> {
    code_only(&window)?;
    let root = self::worktree(&state, &worktree)?;
    let watcher = watch::start(&root, channel)?;
    // One worktree is watched at a time: the one on screen.
    with(&state, |s| {
        s.watchers.clear();
        s.watchers.insert(root, watcher);
        Ok(())
    })
}

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
    let root = self::worktree(&state, &worktree)?;
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
    Ok(run::load(&self::worktree(&state, &worktree)?))
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
    run::save(&self::worktree(&state, &worktree)?, &configs)
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
    let root = self::worktree(&state, &worktree)?;
    let setup = run::load(&root);
    let config = setup
        .configs
        .get(index)
        .cloned()
        .ok_or("Save how to run this project first.")?;
    let cwd = super::inside(&root, &config.cwd)?;
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
    Ok(launch::detect(&self::worktree(&state, &worktree)?))
}

#[tauri::command]
pub async fn code_launch_open(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<(), String> {
    code_only(&window)?;
    launch::open_built(&self::worktree(&state, &worktree)?)
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
    let tree = self::worktree(&state, &worktree)?
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

// --------------------------------------------------------------- add-ons

fn scripts_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    let bundled = app.path().resource_dir().ok().map(|r| r.join("dev-addons"));
    // In development the scripts are read straight from the source tree.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/dev-addons");
    bundled
        .filter(|p| p.is_dir())
        .or_else(|| source.is_dir().then_some(source))
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

// ----------------------------------------------------------------- window

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
