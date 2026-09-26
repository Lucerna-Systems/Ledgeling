//! Explicit pet interaction UI: the speech bubble a right-click opens, on every
//! platform. The bubble is a small webview window (`src/bubble`) that renders
//! what it is told and reports what the user did; which pet it belongs to,
//! where it stands and what a request becomes are decided here.
use crate::{
    app::Shared,
    engine::Engine,
    tasks::{
        types::{effective_connection, ProviderKind, Task, TaskRequest},
        TaskService,
    },
    world::Rect,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static FOLDERS: OnceLock<Mutex<HashMap<u64, String>>> = OnceLock::new();
pub fn folder(pet: u64) -> Option<String> {
    FOLDERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .get(&pet)
        .cloned()
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn perch_ui_init(callback: extern "C" fn(*const std::ffi::c_char));
    fn perch_overlay_panel(window: *mut std::ffi::c_void);
    fn perch_bubble_panel(window: *mut std::ffi::c_void);
    fn perch_bubble_focus(window: *mut std::ffi::c_void);
    fn perch_begin_folder_drag(
        window: *mut std::ffi::c_void,
        pet: u64,
        image: *const std::ffi::c_char,
        frames: i32,
    ) -> i32;
}
pub fn init(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
    #[cfg(target_os = "macos")]
    unsafe {
        perch_ui_init(event);
    }
}
/// The native Finder handoff reports its drop here.
#[cfg(target_os = "macos")]
extern "C" fn event(raw: *const std::ffi::c_char) {
    if raw.is_null() {
        return;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }.to_string_lossy();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
        deliver(value);
    }
}
/// An event from native pet UI: the macOS bubble and folder drag, or the
/// Windows folder drag.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
pub(crate) fn deliver(value: serde_json::Value) {
    let Some(app) = APP.get().cloned() else {
        return;
    };
    // Callbacks never hold the UI thread while accessing task storage or engine.
    std::thread::spawn(move || {
        if let Err(error) = handle_event(&app, value) {
            message(&app, &error);
        }
    });
}

// --------------------------------------------------------------------- bubble

/// The window label. There is one bubble, reused for every pet.
const BUBBLE: &str = "bubble";
/// The bubble's one fixed dimension, in logical pixels; its height follows
/// what is on show and is reported by the page.
pub const BUBBLE_WIDTH: f64 = 370.;
/// Transparent room around the drawn bubble for its shadow. The tail tip sits
/// on this margin's inner edge.
pub const BUBBLE_MARGIN: f64 = 12.;
/// A first guess at the height, used until the page reports the real one.
const DEFAULT_HEIGHT: f64 = 320.;
/// Kept between the bubble and the edges of the work area.
const EDGE_GAP: f64 = 8.;

/// Where the bubble window goes, in logical screen coordinates, and where its
/// tail points in bubble-local x. `tail_up` puts the tail on the top edge, for
/// when there was no room above the pet and the bubble went below it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub tail_x: f64,
    pub tail_up: bool,
}

/// Put a bubble of `height` next to the pet drawn in `anchor`, inside `work`.
/// Above the pet by default, tail down; below it, tail up, when the top of the
/// work area is in the way. Both are clamped on-screen, which can push the
/// window off-centre, so the tail is aimed at the pet rather than fixed.
pub fn place(anchor: Rect, work: Rect, height: f64) -> Placement {
    let width = BUBBLE_WIDTH + BUBBLE_MARGIN * 2.;
    let centre = anchor.x + anchor.width / 2.;
    // The window's shadow margin may hang past the work area; the bubble may not.
    let min_x = work.x + EDGE_GAP - BUBBLE_MARGIN;
    let max_x = (work.right() - EDGE_GAP + BUBBLE_MARGIN - width).max(min_x);
    // The tail sits near the bubble's leading corner, the way a speech bubble
    // is drawn, rather than dead centre.
    let x = (centre - BUBBLE_MARGIN - 56.).clamp(min_x, max_x);
    let min_y = work.y + EDGE_GAP - BUBBLE_MARGIN;
    let max_y = (work.bottom() - EDGE_GAP + BUBBLE_MARGIN - height).max(min_y);
    let above = anchor.y - height + BUBBLE_MARGIN;
    let tail_up = above < min_y;
    let y = if tail_up {
        anchor.bottom() - BUBBLE_MARGIN
    } else {
        above
    }
    .clamp(min_y, max_y);
    Placement {
        x,
        y,
        width,
        height,
        tail_x: centre - x - BUBBLE_MARGIN,
        tail_up,
    }
}

/// The pet's drawn rectangle and its screen's work area, in the logical
/// coordinates the window API takes. `None` when the pet is gone.
pub fn anchor(engine: &Engine, pet: u64) -> Option<(Rect, Rect)> {
    let monitor = engine.monitors.first()?;
    let pet = engine.pets.iter().find(|p| p.id == pet)?;
    let scale = monitor.scale.max(0.01);
    let logical = |r: Rect| Rect::new(r.x / scale, r.y / scale, r.width / scale, r.height / scale);
    Some((
        logical(pet.sprite_rect(monitor.scale)),
        logical(monitor.work),
    ))
}

/// What the bubble is doing. Its lock is never held while taking the engine
/// lock, and never across a window call or an emit.
struct State {
    /// The pet the bubble is open on, `None` while it is closed.
    pet: Option<u64>,
    /// Bumped on every open, so the close of an earlier opening, reported
    /// late, cannot close — or unpin the pet of — the current one.
    session: u64,
    height: f64,
    anchor: Option<(Rect, Rect)>,
    placement: Option<Placement>,
    /// The last opening, re-sent when the page (re)loads and asks for it.
    shown: Option<serde_json::Value>,
    /// A folder dialog is up: the bubble losing focus to it is not a dismissal.
    picking: bool,
}
fn bubble_state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(|| {
            Mutex::new(State {
                pet: None,
                session: 0,
                height: DEFAULT_HEIGHT,
                anchor: None,
                placement: None,
                shown: None,
                picking: false,
            })
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
/// The pet the bubble is open on, for the update loop to follow.
pub fn open_pet() -> Option<u64> {
    bubble_state().pet
}

fn bubble_window(app: &tauri::AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(BUBBLE)
}

/// Move and size the window. Position lands before size, as for the overlay:
/// logical sizes are converted with the window's current screen's scale.
fn apply(window: &WebviewWindow, placement: Placement) {
    let _ = window.set_position(tauri::LogicalPosition::new(placement.x, placement.y));
    let _ = window.set_size(tauri::LogicalSize::new(placement.width, placement.height));
}

fn tail(app: &tauri::AppHandle, placement: Placement) {
    let _ = app.emit_to(
        BUBBLE,
        "bubble:place",
        serde_json::json!({"tailX": placement.tail_x, "tailUp": placement.tail_up}),
    );
}

/// Re-place the bubble at `height` against its stored anchor.
fn resize(app: &tauri::AppHandle, height: f64) {
    let placement = {
        let mut bubble = bubble_state();
        bubble.height = height;
        let Some((anchor, work)) = bubble.anchor else {
            return;
        };
        let placement = place(anchor, work, height);
        if bubble.placement == Some(placement) {
            return;
        }
        bubble.placement = Some(placement);
        placement
    };
    if let Some(window) = bubble_window(app) {
        apply(&window, placement);
    }
    tail(app, placement);
}

/// Called by the update loop, without the engine lock, with where the open
/// bubble's pet is now. The pet is pinned while the bubble is open, but the
/// window it stands on may move, and the bubble goes with it.
pub fn follow(app: &tauri::AppHandle, anchor: Option<(Rect, Rect)>) {
    let Some((anchor, work)) = anchor else {
        return;
    };
    let placement = {
        let mut bubble = bubble_state();
        if bubble.pet.is_none() {
            return;
        }
        let moved = bubble.anchor.is_none_or(|(old, old_work)| {
            (old.x - anchor.x).abs() > 0.5 || (old.y - anchor.y).abs() > 0.5 || old_work != work
        });
        if !moved {
            return;
        }
        bubble.anchor = Some((anchor, work));
        let placement = place(anchor, work, bubble.height);
        bubble.placement = Some(placement);
        placement
    };
    if let Some(window) = bubble_window(app) {
        apply(&window, placement);
    }
    tail(app, placement);
}

/// Close the bubble if `session` is still the current opening: the pet is
/// free to wander again unless its own task holds it. Closing never touches
/// the task.
fn close(app: &tauri::AppHandle, session: Option<u64>) -> Result<(), String> {
    let pet = {
        let mut bubble = bubble_state();
        if session.is_some_and(|s| s != bubble.session) {
            return Ok(());
        }
        bubble.pet.take()
    };
    if let Some(pet) = pet {
        let shared = app.state::<Shared>();
        let mut state = shared.lock().map_err(|_| "Pet state unavailable")?;
        state.engine.stop_listening(pet);
    }
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Some(window) = bubble_window(&handle) {
            let _ = window.hide();
        }
    })
    .map_err(|e| e.to_string())
}

/// Give the bubble keyboard focus. On macOS it is a non-activating panel that
/// can still become key, so typing into it neither activates Ledgeling nor brings
/// the dashboard forward.
fn focus(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = window.ns_window() {
        unsafe {
            perch_bubble_focus(pointer);
        }
        return;
    }
    let _ = window.set_focus();
}

/// Build the bubble window the first time it is needed. UI thread only.
fn ensure_bubble(app: &tauri::AppHandle) -> Option<WebviewWindow> {
    if let Some(window) = bubble_window(app) {
        return Some(window);
    }
    let built = WebviewWindowBuilder::new(app, BUBBLE, WebviewUrl::App("index.html?bubble".into()))
        .title("Ledgeling")
        .inner_size(BUBBLE_WIDTH + BUBBLE_MARGIN * 2., DEFAULT_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .focused(false)
        .accept_first_mouse(true)
        .visible_on_all_workspaces(true)
        .build();
    let window = match built {
        Ok(window) => window,
        Err(error) => {
            eprintln!("Ledgeling could not create the pet bubble: {error}");
            return None;
        }
    };
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = window.ns_window() {
        unsafe {
            perch_bubble_panel(pointer);
        }
    }
    #[cfg(target_os = "windows")]
    if let Ok(handle) = window.hwnd() {
        super::windows::quiet_overlay_frame(handle.0 as isize);
        super::windows::tool_window(handle.0 as isize);
    }
    // As with the overlay, GTK would otherwise draw the app menu inside it.
    #[cfg(target_os = "linux")]
    let _ = window.remove_menu();
    let handle = app.clone();
    window.on_window_event(move |event| {
        // Clicking anywhere else dismisses the bubble, as a popover would —
        // except into the folder dialog the bubble itself opened. The page
        // fades out and then reports the close.
        if let tauri::WindowEvent::Focused(false) = event {
            let session = {
                let bubble = bubble_state();
                (bubble.pet.is_some() && !bubble.picking).then_some(bubble.session)
            };
            if let Some(session) = session {
                let _ = handle.emit_to(BUBBLE, "bubble:dismiss", session);
            }
        }
    });
    Some(window)
}

/// The native folder dialog, attached to the bubble where the OS can.
async fn pick_folder(window: &WebviewWindow) -> Option<String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Choose a working folder")
        .set_parent(window)
        .pick_folder()
        .await?;
    Some(picked.path().to_string_lossy().into_owned())
}

/// Give a pet a working folder for this session.
fn bind(app: &tauri::AppHandle, pet: u64, raw: &str) -> Result<(), String> {
    let path = std::path::Path::new(raw)
        .canonicalize()
        .map_err(|_| "The chosen folder is unavailable")?;
    let path = path.to_string_lossy().into_owned();
    crate::tasks::files::Scope::new(std::slice::from_ref(&path), &[])?;
    FOLDERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(pet, path.clone());
    let _ = app.emit_to(
        "main",
        "pet:folder",
        serde_json::json!({"petId":pet,"folder":path}),
    );
    // Carries the folder too, so the bubble's folder row stops saying
    // "choose one" the moment a drop binds it.
    send(
        app,
        serde_json::json!({
            "petId": pet,
            "folder": path,
            "message": format!("Ready in {path}. Right-click me when you need a hand."),
        }),
    );
    Ok(())
}

fn handle_event(app: &tauri::AppHandle, v: serde_json::Value) -> Result<(), String> {
    let service = app.state::<TaskService>();
    let pet = v["petId"].as_u64().unwrap_or(0);
    match v["type"].as_str().unwrap_or("") {
        "bind" => bind(app, pet, v["folder"].as_str().ok_or("Missing drop folder")?)?,
        "start" => {
            let kind = effective_connection(&service.config(), Some(pet)).kind;
            let request = compose_request(
                kind,
                pet,
                v["mode"].as_str().unwrap_or("actions"),
                v["preset"].as_str().unwrap_or(""),
                v["prompt"].as_str().unwrap_or(""),
                folder(pet),
                v["allowCloud"].as_bool().unwrap_or(false),
                v["allowWeb"].as_bool().unwrap_or(false),
                v["parentId"].as_i64(),
                || crate::tasks::files::default_root().map(|p| p.to_string_lossy().into_owned()),
            )?;
            service.start_for(request, crate::app::task_pet(app, Some(pet)))?;
        }
        // The bubble asks lazily: model listing can spawn a Codex process or
        // hit the network, so it happens here on the event thread, never
        // inside show_pet_bubble.
        "models" => {
            let connection = effective_connection(&service.config(), Some(pet));
            if matches!(connection.kind, ProviderKind::None) {
                return Err("Set up an AI provider in Ledgeling first.".into());
            }
            let models = if matches!(connection.kind, ProviderKind::Codex) {
                crate::tasks::runtimes::capabilities(&connection).map(|c| c.models)
            } else {
                crate::tasks::providers::list_models(&connection)
            }?;
            send(app, serde_json::json!({"petId": pet, "models": models}));
        }
        "model" => {
            let model = v["model"].as_str().ok_or("Missing model")?.to_string();
            let mut config = service.config();
            let mut connection = effective_connection(&config, Some(pet));
            if matches!(connection.kind, ProviderKind::None) {
                return Err("Set up an AI provider in Ledgeling first.".into());
            }
            // Overrides must clear the same endpoint bar as the default.
            if matches!(
                connection.kind,
                ProviderKind::Ollama
                    | ProviderKind::Openai
                    | ProviderKind::Anthropic
                    | ProviderKind::Compatible
            ) {
                crate::tasks::providers::base(&connection)?;
            }
            connection.model = model.clone();
            config.per_pet.insert(pet, connection);
            service.set_config(&config)?;
            let _ = app.emit_to("main", "connections:changed", ());
            send(
                app,
                serde_json::json!({"petId": pet, "model": model, "message": "Model saved for this companion. The next task uses it."}),
            );
        }
        "approve" => service.answer(
            v["id"].as_i64().ok_or("Missing task")?,
            v["approvalId"].as_u64().ok_or("Missing approval")?,
            v["accept"].as_bool().unwrap_or(false),
        )?,
        "cancel" => service.cancel(v["id"].as_i64().ok_or("Missing task")?)?,
        "undo" => service.undo(v["id"].as_i64().ok_or("Missing task")?)?,
        "open" => crate::tasks::commands::open_web_url(v["url"].as_str().ok_or("Missing link")?)?,
        "settings" => {
            let handle = app.clone();
            app.run_on_main_thread(move || {
                if let Some(main) = handle.get_webview_window("main") {
                    let _ = main.unminimize();
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = handle.emit_to("main", "providers:open", ());
                }
            })
            .map_err(|e| e.to_string())?;
        }
        // The bubble closed, by the close button, Escape, or by losing focus.
        "closed" => close(app, v["session"].as_u64())?,
        // The page laid itself out and says how tall it is.
        "size" => {
            let height = v["height"].as_f64().ok_or("Missing height")?;
            if height.is_finite() {
                resize(app, height.clamp(120., 900.));
            }
        }
        // The page (re)loaded after the opening was sent; tell it again.
        "ready" => {
            let shown = bubble_state().shown.clone();
            if let Some(shown) = shown {
                let _ = app.emit_to(BUBBLE, "bubble:show", &shown);
                if let Some(pet) = shown["petId"].as_u64() {
                    send_live_task(app, pet)?;
                }
            }
        }
        "dragEnded" => {
            let shared = app.state::<Shared>();
            let mut state = shared.lock().map_err(|_| "Pet state unavailable")?;
            for p in &mut state.engine.pets {
                if p.id == pet && p.is_held() {
                    p.release();
                }
            }
            drop(state);
            let _ = app.emit("pet:drag-ended", pet);
        }
        "error" => message(
            app,
            v["message"]
                .as_str()
                .unwrap_or("The pet interaction could not finish."),
        ),
        _ => return Err("Unknown pet interaction".into()),
    }
    Ok(())
}
/// Built-in action ids map to (title, instruction): the title is the offline
/// fallback prompt — the rename preset uses the prompt as a literal filename
/// prefix, so it must never receive the instruction text — while the
/// instruction is composed into model/agent prompts when a connection runs it.
pub fn action_texts(preset: &str) -> Option<(&'static str, &'static str)> {
    Some(match preset {
        "sort_type" => (
            "Sort by type",
            "Sort the immediate files in the working folder by file type, moving each into a new subfolder for its type. Keep existing subfolders unchanged.",
        ),
        "sort_date" => (
            "Sort by month",
            "Sort the immediate files in the working folder by modified month, moving each into a subfolder named like 2026-09. Keep existing subfolders unchanged.",
        ),
        "rename" => (
            "Rename with prefix",
            "Rename the files in the working folder by adding the prefix the user gave before each file name.",
        ),
        "find" => (
            "Find files",
            "Find files in the working folder matching what the user described and report where each is.",
        ),
        "duplicates" => (
            "Find duplicates",
            "Find byte-identical duplicate files in the working folder and report the groups.",
        ),
        _ => return None,
    })
}
/// Ask is questions and web search only; Actions is built-in presets without a
/// connection, or the connected model's tool loop with one. With any
/// connection the preset becomes an instruction for the model, which may also
/// create folders, so a chosen folder is optional and falls back to Ledgeling's
/// own Documents folder.
#[allow(clippy::too_many_arguments)]
pub fn compose_request(
    kind: ProviderKind,
    pet: u64,
    mode: &str,
    preset: &str,
    text: &str,
    folder: Option<String>,
    allow_cloud: bool,
    allow_web: bool,
    parent: Option<i64>,
    fallback_root: impl FnOnce() -> Result<String, String>,
) -> Result<TaskRequest, String> {
    let agent_runtime = matches!(kind, ProviderKind::Codex | ProviderKind::Claude);
    let connected = !matches!(kind, ProviderKind::None);
    if mode == "ask" {
        if !connected {
            return Err("Set up an AI provider in Ledgeling to ask questions.".into());
        }
        if text.trim().is_empty() {
            return Err("Type a question first.".into());
        }
        // Which loop an ask runs is a property of the connection, not a
        // choice: an agent runtime has no plain-completion call, and a model
        // connection has no agent loop. The bubble asks neither.
        return Ok(TaskRequest {
            prompt: text.into(),
            pet_id: Some(pet),
            roots: vec![],
            attachments: vec![],
            preset: String::new(),
            mode: if agent_runtime {
                "advanced"
            } else {
                "standard"
            }
            .into(),
            parent_id: parent,
            // Only the question itself is sent; asking is the consent.
            allow_cloud: true,
            allow_web,
        });
    }
    if connected {
        let instruction = action_texts(preset).map(|(_, i)| i);
        let prompt = match (instruction, text.trim()) {
            (Some(i), "") => i.into(),
            (Some(i), t) => format!("{i}\n\n{t}"),
            (None, "") => return Err("Describe what to do, or pick a built-in action.".into()),
            (None, t) => t.into(),
        };
        // Cloud consent is given once, when the provider is added; this only
        // records what an older bubble still sends.
        let cloud = agent_runtime || allow_cloud;
        return Ok(TaskRequest {
            prompt,
            pet_id: Some(pet),
            roots: vec![match folder {
                Some(f) => f,
                None => fallback_root()?,
            }],
            attachments: vec![],
            preset: String::new(),
            mode: if agent_runtime {
                "advanced"
            } else {
                "standard"
            }
            .into(),
            parent_id: parent,
            allow_cloud: cloud,
            allow_web,
        });
    }
    // Without a connection the built-in actions run deterministically and
    // offline, and need a folder the user chose.
    let (title, _) = action_texts(preset)
        .ok_or("Pick a built-in action, or set up an AI provider in Ledgeling.")?;
    let folder = folder.ok_or("Choose a folder for built-in actions.")?;
    Ok(TaskRequest {
        prompt: if text.trim().is_empty() {
            title.into()
        } else {
            text.into()
        },
        pet_id: Some(pet),
        roots: vec![folder],
        attachments: vec![],
        preset: preset.into(),
        mode: "standard".into(),
        parent_id: parent,
        allow_cloud: false,
        allow_web: false,
    })
}
fn send(app: &tauri::AppHandle, value: serde_json::Value) {
    let _ = app.emit_to(BUBBLE, "bubble:update", value);
}
pub fn message(app: &tauri::AppHandle, text: &str) {
    send(app, serde_json::json!({"message":text}));
}
pub fn update(app: &tauri::AppHandle, task: &Task) {
    if let Ok(v) = serde_json::to_value(task) {
        send(app, v);
    }
}
/// Only an in-flight task belongs in a freshly opened bubble; a task that
/// finished in a prior session is history, not this conversation.
fn send_live_task(app: &tauri::AppHandle, pet: u64) -> Result<(), String> {
    if let Some(task) = app.state::<TaskService>().list()?.into_iter().find(|t| {
        t.request.pet_id == Some(pet)
            && matches!(
                t.status.as_str(),
                "queued" | "running" | "waiting" | "cancelling" | "undoing"
            )
    }) {
        update(app, &task);
    }
    Ok(())
}
pub fn overlay(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = window.ns_window() {
        unsafe {
            perch_overlay_panel(pointer);
        }
    }
    #[cfg(target_os = "windows")]
    if let Ok(handle) = window.hwnd() {
        super::windows::quiet_overlay_frame(handle.0 as isize);
    }
    // GTK gives every window the app's menu bar, which would sit across the
    // top of the pet screen as a stray "Ledgeling" menu.
    #[cfg(target_os = "linux")]
    let _ = window.remove_menu();
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let _ = window;
}

/// Showing a window lets the toolkit rewrite its extended styles; put the
/// click-through state back. Only Windows needs this.
pub fn restore_overlay(window: &tauri::WebviewWindow, click_through: bool) {
    #[cfg(target_os = "windows")]
    let _ = self::click_through(window, click_through);
    #[cfg(not(target_os = "windows"))]
    let _ = (window, click_through);
}

/// Let clicks pass through the overlay, or catch them while over a pet.
pub fn click_through(window: &tauri::WebviewWindow, click_through: bool) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = window.hwnd() {
        super::windows::set_click_through(handle.0 as isize, click_through);
        return Ok(());
    }
    window.set_ignore_cursor_events(click_through)
}
#[tauri::command]
pub async fn pet_folder(pet_id: u64) -> Option<String> {
    folder(pet_id)
}
#[tauri::command]
pub async fn show_pet_bubble(app: tauri::AppHandle, pet_id: u64) -> Result<(), String> {
    let (name, anchor) = {
        let shared = app.state::<Shared>();
        let mut state = shared.lock().map_err(|_| "Pet state unavailable")?;
        let name = state
            .engine
            .pets
            .iter()
            .find(|p| p.id == pet_id)
            .map(|p| p.pack.manifest.name.clone())
            .ok_or("This pet is no longer on the desktop")?;
        // Whatever it was doing, it now stays under the bubble pointing at it.
        // There is one bubble, so a pet it was open on before is released.
        state.engine.listen(pet_id);
        (name, anchor(&state.engine, pet_id))
    };
    // The bubble states what a mode still needs before it is picked, so it
    // has to know which runtime is connected. Per-pet overrides count: the
    // bubble must describe the connection this pet would run on.
    let config = app.state::<TaskService>().config();
    let connection = effective_connection(&config, Some(pet_id));
    let runtime = match connection.kind {
        ProviderKind::None => "none",
        ProviderKind::Ollama => "ollama",
        ProviderKind::Codex => "codex",
        ProviderKind::Claude => "claude",
        _ => "model",
    };
    let (payload, placement) = {
        let mut bubble = bubble_state();
        bubble.session += 1;
        bubble.pet = Some(pet_id);
        bubble.anchor = anchor;
        bubble.placement = anchor.map(|(anchor, work)| place(anchor, work, bubble.height));
        let placement = bubble.placement;
        let payload = serde_json::json!({
            "session": bubble.session,
            "petId": pet_id,
            "name": name,
            "runtime": runtime,
            "model": connection.model,
            "folder": folder(pet_id).unwrap_or_default(),
            "tailX": placement.map(|p| p.tail_x),
            "tailUp": placement.is_some_and(|p| p.tail_up),
        });
        bubble.shown = Some(payload.clone());
        (payload, placement)
    };
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let Some(window) = ensure_bubble(&handle) else {
            return;
        };
        if let Some(placement) = placement {
            apply(&window, placement);
        }
        let _ = window.show();
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = window.hwnd() {
            // Showing lets the toolkit rewrite the extended styles.
            super::windows::tool_window(hwnd.0 as isize);
        }
        focus(&window);
    })
    .map_err(|e| e.to_string())?;
    // A bubble built just now has no page listening yet; it asks with
    // "ready" once it has, and gets the same opening again.
    let _ = app.emit_to(BUBBLE, "bubble:show", &payload);
    send_live_task(&app, pet_id)
}
/// Everything the bubble page reports. Only the bubble may send these.
#[tauri::command]
pub async fn pet_bubble(
    app: tauri::AppHandle,
    window: WebviewWindow,
    event: serde_json::Value,
) -> Result<(), String> {
    if window.label() != BUBBLE {
        return Err("This control belongs to the pet bubble.".into());
    }
    if event["type"] == "choose" {
        let pet = event["petId"].as_u64().ok_or("Missing pet")?;
        bubble_state().picking = true;
        let picked = pick_folder(&window).await;
        bubble_state().picking = false;
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = bubble_window(&handle) {
                focus(&window);
            }
        });
        let Some(picked) = picked else {
            return Ok(());
        };
        return tauri::async_runtime::spawn_blocking(move || {
            if let Err(error) = bind(&app, pet, &picked) {
                message(&app, &error);
            }
        })
        .await
        .map_err(|e| e.to_string());
    }
    // Task storage, model listing and the engine lock all block; none of it
    // runs on the command's async executor.
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = handle_event(&app, event) {
            message(&app, &error);
        }
    })
    .await
    .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_root() -> Result<String, String> {
        Ok("/tmp/perch-test-root".into())
    }

    fn req(
        kind: ProviderKind,
        mode: &str,
        preset: &str,
        text: &str,
    ) -> Result<TaskRequest, String> {
        compose_request(
            kind, 1, mode, preset, text, None, false, false, None, fixed_root,
        )
    }

    #[test]
    fn ask_uses_the_connection_loop_and_no_scope() {
        let r = req(ProviderKind::Ollama, "ask", "", "why is the sky blue?").unwrap();
        assert_eq!(r.mode, "standard");
        assert_eq!(r.preset, "");
        assert!(r.roots.is_empty());
        assert!(r.allow_cloud);
        assert_eq!(r.prompt, "why is the sky blue?");
        let r = req(ProviderKind::Claude, "ask", "", "summarize this").unwrap();
        assert_eq!(r.mode, "advanced");
        assert!(r.roots.is_empty());
    }

    #[test]
    fn ask_without_connection_or_text_is_rejected() {
        assert!(req(ProviderKind::None, "ask", "", "hello?").is_err());
        assert!(req(ProviderKind::Ollama, "ask", "", "  ").is_err());
    }

    #[test]
    fn builtin_actions_run_offline_only_with_a_chosen_folder() {
        let r = compose_request(
            ProviderKind::None,
            1,
            "actions",
            "sort_date",
            "",
            Some("/tmp/chosen".into()),
            true,
            false,
            None,
            fixed_root,
        )
        .unwrap();
        assert_eq!(r.mode, "standard");
        assert_eq!(r.preset, "sort_date");
        assert_eq!(r.roots, vec!["/tmp/chosen"]);
        assert!(!r.allow_cloud);
        // No text: the prompt is the preset title, never the instruction, so
        // the offline rename/find presets read user text only.
        assert_eq!(r.prompt, "Sort by month");
        let r = compose_request(
            ProviderKind::None,
            1,
            "actions",
            "rename",
            "trip-",
            Some("/tmp/chosen".into()),
            false,
            false,
            None,
            fixed_root,
        )
        .unwrap();
        assert_eq!(r.prompt, "trip-");
        assert!(req(ProviderKind::None, "actions", "sort_type", "").is_err());
        assert!(req(ProviderKind::None, "actions", "", "do things").is_err());
    }

    #[test]
    fn any_connection_takes_over_actions_in_the_bound_folder() {
        let r = compose_request(
            ProviderKind::Ollama,
            1,
            "actions",
            "sort_type",
            "also make a photos folder",
            Some("/tmp/chosen".into()),
            false,
            true,
            None,
            fixed_root,
        )
        .unwrap();
        assert_eq!(r.mode, "standard");
        assert_eq!(r.preset, "");
        assert_eq!(r.roots, vec!["/tmp/chosen"]);
        assert!(r.prompt.contains("Sort the immediate files"));
        assert!(r.prompt.ends_with("also make a photos folder"));
        assert!(r.allow_web);
    }

    #[test]
    fn folderless_actions_fall_back_to_the_perch_folder() {
        let r = req(ProviderKind::Ollama, "actions", "find", "").unwrap();
        assert_eq!(r.roots, vec!["/tmp/perch-test-root"]);
        let r = req(ProviderKind::Codex, "actions", "duplicates", "").unwrap();
        assert_eq!(r.mode, "advanced");
        assert_eq!(r.roots, vec!["/tmp/perch-test-root"]);
        // The agent start approval is the consent, so cloud use is implied.
        assert!(r.allow_cloud);
    }

    #[test]
    fn remote_model_actions_keep_the_explicit_cloud_choice() {
        let r = req(ProviderKind::Openai, "actions", "sort_type", "").unwrap();
        assert_eq!(r.mode, "standard");
        assert!(!r.allow_cloud);
        let r = compose_request(
            ProviderKind::Openai,
            1,
            "actions",
            "sort_type",
            "",
            None,
            true,
            false,
            None,
            fixed_root,
        )
        .unwrap();
        assert!(r.allow_cloud);
    }

    #[test]
    fn connected_custom_actions_need_a_description() {
        assert!(req(ProviderKind::Ollama, "actions", "", "  ").is_err());
        let r = req(ProviderKind::Ollama, "actions", "", "organise by client").unwrap();
        assert_eq!(r.prompt, "organise by client");
        assert_eq!(r.preset, "");
    }

    const WORK: Rect = Rect {
        x: 0.,
        y: 25.,
        width: 1440.,
        height: 850.,
    };

    #[test]
    fn the_bubble_sits_above_its_pet_with_the_tail_on_it() {
        let pet = Rect::new(600., 500., 40., 40.);
        let p = place(pet, WORK, 300.);
        assert!(!p.tail_up);
        // The tail tip is the bottom margin's inner edge, on the pet's head.
        assert_eq!(p.y + p.height - BUBBLE_MARGIN, pet.y);
        assert_eq!(p.x + BUBBLE_MARGIN + p.tail_x, 620.);
        assert_eq!(p.width, BUBBLE_WIDTH + BUBBLE_MARGIN * 2.);
    }

    #[test]
    fn no_room_above_puts_the_bubble_below_with_the_tail_up() {
        let pet = Rect::new(600., 60., 40., 40.);
        let p = place(pet, WORK, 300.);
        assert!(p.tail_up);
        assert_eq!(p.y + BUBBLE_MARGIN, pet.bottom());
    }

    #[test]
    fn a_pet_at_the_screen_edge_keeps_the_bubble_on_screen_and_aimed_at_it() {
        for x in [0., 1420.] {
            let pet = Rect::new(x, 500., 20., 20.);
            let p = place(pet, WORK, 300.);
            let left = p.x + BUBBLE_MARGIN;
            let right = left + BUBBLE_WIDTH;
            assert!(left >= WORK.x && right <= WORK.right(), "{p:?}");
            // Still pointing at the pet, not at a fixed spot on the bubble.
            assert_eq!(left + p.tail_x, x + 10.);
        }
    }

    #[test]
    fn a_tall_bubble_is_clamped_to_the_work_area() {
        let pet = Rect::new(600., 400., 40., 40.);
        let p = place(pet, WORK, 2000.);
        assert!(p.y + BUBBLE_MARGIN >= WORK.y);
    }

    #[test]
    fn the_anchor_is_the_pets_sprite_in_logical_pixels() {
        use crate::world::{MonitorInfo, Point};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs");
        let pack = crate::packs::loader::load(&root.join("budgie-classic"), true).unwrap();
        let mut settings = crate::settings::Settings::default();
        settings.pets.clear();
        let mut engine = Engine::new(
            settings,
            HashMap::from([(pack.manifest.id.clone(), std::sync::Arc::new(pack))]),
        );
        engine.set_monitors(vec![MonitorInfo {
            id: 0,
            bounds: Rect::new(0., 0., 2880., 1800.),
            work: Rect::new(0., 50., 2880., 1750.),
            scale: 2.,
            is_primary: true,
        }]);
        let pack = engine.packs.keys().next().cloned().unwrap();
        let id = engine.spawn(&pack).unwrap();
        let pet = engine.pets.iter_mut().find(|p| p.id == id).unwrap();
        pet.position = Point::new(1000., 1000.);
        let sprite = pet.sprite_rect(2.);
        let (anchor, work) = anchor(&engine, id).unwrap();
        assert_eq!(anchor.x, sprite.x / 2.);
        assert_eq!(anchor.width, sprite.width / 2.);
        assert_eq!(work, Rect::new(0., 25., 1440., 875.));
        assert!(super::anchor(&engine, id + 1).is_none());
    }
}
#[tauri::command]
pub async fn pet_folder_drag(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    pet_id: u64,
) -> Result<(), String> {
    if !window.label().starts_with("pet-") {
        return Err("Drag a desktop pet to start a folder handoff.".into());
    }
    #[cfg(target_os = "macos")]
    {
        let (image, frames) = {
            let shared = app.state::<Shared>();
            let state = shared.lock().map_err(|_| "Pet state unavailable")?;
            let pet = state
                .engine
                .pets
                .iter()
                .find(|p| p.id == pet_id)
                .ok_or("Pet not found")?;
            let atlas = pet.pack.atlas();
            let clip = atlas.values().next().ok_or("Pet has no sprite")?;
            (clip.image.clone(), clip.frames as i32)
        };
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let result = (|| {
                let pointer = window.ns_window().map_err(|e| e.to_string())?;
                let image = std::ffi::CString::new(image).map_err(|e| e.to_string())?;
                let started =
                    unsafe { perch_begin_folder_drag(pointer, pet_id, image.as_ptr(), frames) };
                if started == 0 {
                    return Err(
                        "Keep the mouse pressed while dragging into a Finder folder.".into(),
                    );
                }
                let _ = window.set_ignore_cursor_events(true);
                Ok(())
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "Native drag did not start".to_string())?
    }
    #[cfg(target_os = "windows")]
    {
        {
            let shared = app.state::<Shared>();
            let state = shared.lock().map_err(|_| "Pet state unavailable")?;
            if !state.engine.pets.iter().any(|p| p.id == pet_id) {
                return Err("Pet not found".into());
            }
        }
        let overlay = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(super::windows_drop::begin(overlay, pet_id));
        })
        .map_err(|e| e.to_string())?;
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "Native drag did not start".to_string())?
    }
    // No native folder drop here: the pet just follows the cursor.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (app, pet_id);
        Err("Folder drops are not available on this platform.".into())
    }
}
