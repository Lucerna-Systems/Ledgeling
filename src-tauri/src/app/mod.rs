pub mod commands;
pub mod loop_runner;
pub mod menus;
pub mod screens;

use crate::{
    awareness::AwarenessSummary,
    engine::Engine,
    packs::{builtin, loader, PackSummary},
    pet::{Attend, Motion},
    settings::{self, Settings},
    tasks::commands::*,
    world::{MonitorInfo, Rect},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};

pub use screens::{on_main, Screen};

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
    /// Whether this build includes coding mode at all.
    pub coding_available: bool,
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
    pub fn snapshot(&self) -> Snapshot {
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
            coding_available: cfg!(feature = "coding"),
        }
    }

    pub fn persist(&mut self) -> Result<(), String> {
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

// ---------------------------------------------------------------------- setup

pub fn run(context: tauri::Context<tauri::Wry>) {
    crate::platform::prepare_process();
    tauri::Builder::default()
        .on_menu_event(menus::handle_menu)
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
            task_provider_terms,
            task_add_provider,
            task_remove_provider,
            task_apple_open_settings,
            task_account,
            task_pick,
            task_open_url,
            open_assistant,
            crate::terminal::commands::terminal_state,
            crate::terminal::commands::terminal_spawn,
            crate::terminal::commands::terminal_io,
            crate::terminal::commands::terminal_attach,
            crate::terminal::commands::terminal_layout_save,
            crate::terminal::commands::terminal_open_url,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_open,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_state,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_add_project,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_remove_project,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_select,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_branches,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_worktree_create,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_worktree_remove,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_bind,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_status,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_original,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_git,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_list,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_read,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_write,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_file_op,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_watch,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_pty_spawn,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_pty,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_pty_attach,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_run_setup,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_run_save,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_run_prompt,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_run_start,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_live,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_activate,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_launch,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_launch_open,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_layouts,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_notes,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_note,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_layout_save,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_open_url,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_tools,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_install,
            #[cfg(feature = "coding")]
            crate::coding::commands::code_window,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_status,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_list,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_view,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_diff,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_branch_pr,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_meta,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_act,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_pr_draft,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_create_pr,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_checkout,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_projects,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_board,
            #[cfg(feature = "coding")]
            crate::coding::gh_commands::code_gh_board_act,
            commands::get_snapshot,
            commands::pet_action,
            commands::pet_interact,
            commands::set_settings,
            commands::set_app_icon,
            commands::list_packs,
            commands::validate_pack,
            commands::install_pack,
            commands::uninstall_pack,
            commands::new_pack,
            commands::save_studio_pack,
            commands::export_pack,
            commands::spawn_pet,
            commands::despawn_pet,
            commands::fire_event,
            commands::set_lab,
            commands::pet_surface_ready,
            commands::quit_app
        ])
        .setup(|app| {
            let main = app
                .get_webview_window("main")
                .ok_or("The companion window is missing")?;
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            // Keys are only looked for on the launch that moves the folder, so a
            // denied keychain prompt is asked once rather than on every start.
            let mut migration_note = None;
            match crate::migrate::move_app_dir(&config_dir) {
                Ok(true) => crate::tasks::providers::migrate_legacy_secrets(),
                Ok(false) => {}
                Err(error) => migration_note = Some(error),
            }
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
                    #[cfg(feature = "coding")]
                    let _ =
                        task_handle.emit_to(crate::coding::window::LABEL, "task:changed", &task);
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
            let _ = main.set_theme(commands::theme_to_option(settings.theme));
            let mut notes: Vec<String> = note.into_iter().chain(migration_note).collect();
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
            let found = screens::screens(app.handle()).unwrap_or_default();
            let chosen = screens::chosen_screen(&found, engine.settings.pet_screen.as_deref());
            let initial = chosen
                .map(|screen| vec![screens::pet_monitor(screen)])
                .unwrap_or_default();
            engine.set_monitors(initial);
            let pet_screen = chosen.map(|screen| screen.name.clone());
            if let Ok(executable) = std::env::current_exe() {
                let _ = crate::autostart::apply(engine.settings.launch_at_login, &executable);
            }
            let missing = engine.restore_pets();
            crate::platform::pet_ui::seed_folders(engine.pet_folders());
            for pack_id in missing {
                notes.push(format!(
                    "The pack `{pack_id}` is no longer installed, so that pet did not come back."
                ));
            }
            let developer = engine.settings.developer_mode;
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
            // Terminal mode: empty until a pet opens its terminal. Event files
            // from an earlier run belong to sessions that no longer exist.
            crate::terminal::agent::reset(
                &shared
                    .lock()
                    .map(|a| a.config_dir.clone())
                    .unwrap_or_default(),
            );
            app.manage::<crate::terminal::SharedTerminal>(Default::default());
            // Empty until Developer tools are on and the window is first opened.
            #[cfg(feature = "coding")]
            app.manage::<crate::coding::SharedCoding>(Default::default());
            menus::setup_tray(app, developer)?;
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
            std::thread::spawn(move || loop_runner::run_loop(shared, handle));
            Ok(())
        })
        .build(context)
        .expect("Unable to start Ledgeling")
        .run(|handle, event| {
            // `fm serve` is a child process holding the on-device model, and
            // coding mode may have dev servers running; none may outlive Ledgeling.
            if matches!(event, tauri::RunEvent::Exit) {
                crate::tasks::apple::shutdown();
                #[cfg(feature = "coding")]
                crate::coding::commands::shut_down(handle);
                #[cfg(not(feature = "coding"))]
                let _ = handle;
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
