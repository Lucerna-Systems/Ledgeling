//! The companion window ("main"), created on demand.
//!
//! A created-but-hidden webview costs tens of MiB, so the window does not
//! exist until something asks for it (tray, a pet's bubble, a drop).
//!
//! Close policy: closing hides the window, as it always did. Only after it has
//! stayed hidden for [`GRACE`] is it destroyed, and never while a task is
//! queued, running or waiting on the user, or while the page holds input that
//! is not persisted (`volatile` in the UI state). State that lives only in the
//! webview (page, unsent assistant text, the Studio design) is written to
//! `ui_state.json` by the frontend as it changes and handed back by
//! `companion_ready` when the window is recreated.
use crate::app::{task_attention, Shared};
use serde::Serialize;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "main";
/// How long the companion stays hidden before it may be destroyed.
pub const GRACE: Duration = Duration::from_secs(10 * 60);
const UI_FILE: &str = "ui_state.json";
const UI_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct Inner {
    /// The webview has registered its listeners and read its state.
    ready: bool,
    /// Events sent while the window was absent or still loading.
    pending: Vec<(String, Value)>,
    hidden_since: Option<Instant>,
    ui: Option<String>,
}

pub struct CompanionState {
    inner: Mutex<Inner>,
    dir: PathBuf,
}

impl CompanionState {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            inner: Mutex::new(Inner {
                ui: load_ui(config_dir),
                ..Default::default()
            }),
            dir: config_dir.to_path_buf(),
        }
    }
}

/// Persist the frontend's UI state blob. Must be a JSON object of sane size.
pub fn save_ui(dir: &Path, json: &str) -> Result<(), String> {
    if json.len() > UI_MAX_BYTES {
        return Err("The saved window state is too large.".into());
    }
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Object(_)) => {}
        _ => return Err("The saved window state must be a JSON object.".into()),
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let temp = dir.join(format!("{UI_FILE}.tmp"));
    std::fs::write(&temp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, dir.join(UI_FILE)).map_err(|e| e.to_string())
}

pub fn load_ui(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(UI_FILE)).ok()?;
    matches!(serde_json::from_str::<Value>(&text), Ok(Value::Object(_))).then_some(text)
}

/// Does the saved state say the page holds input that is not persisted?
pub fn is_volatile(ui: Option<&str>) -> bool {
    ui.and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| value["volatile"].as_bool())
        .unwrap_or(false)
}

/// The grace-period decision. `hidden_for` is `None` while the window is
/// visible or absent.
pub fn should_destroy(
    hidden_for: Option<Duration>,
    grace: Duration,
    busy: bool,
    volatile: bool,
) -> bool {
    hidden_for.is_some_and(|hidden| hidden >= grace) && !busy && !volatile
}

fn state(app: &tauri::AppHandle) -> Option<State<'_, CompanionState>> {
    app.try_state::<CompanionState>()
}

fn lock(state: &CompanionState) -> std::sync::MutexGuard<'_, Inner> {
    state.inner.lock().unwrap_or_else(|e| e.into_inner())
}

/// Build the window if it does not exist. UI thread only. Properties come from
/// the `main` entry in tauri.conf.json (which has `create: false`).
pub fn ensure(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        return Ok(window);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == LABEL)
        .cloned()
        .ok_or("The companion window is not configured")?;
    // The engine lock is held only for this read, never across window calls.
    let theme = app
        .try_state::<Shared>()
        .and_then(|shared| shared.lock().ok().map(|a| a.engine.settings.theme))
        .unwrap_or_default();
    let window = WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .theme(crate::app::theme_to_option(theme))
        .build()
        .map_err(|e| e.to_string())?;
    if let Some(state) = state(app) {
        let mut inner = lock(&state);
        inner.ready = false;
        inner.hidden_since = None;
    }
    let handle = app.clone();
    let closing = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // Closing the companion leaves the pet on the desktop.
            api.prevent_close();
            let _ = closing.hide();
            if let Some(state) = state(&handle) {
                lock(&state).hidden_since = Some(Instant::now());
            }
        }
        tauri::WindowEvent::Destroyed => {
            if let Some(state) = state(&handle) {
                let mut inner = lock(&state);
                inner.ready = false;
                inner.hidden_since = None;
            }
        }
        // Dragging a .petpack or a pack folder onto the window installs it.
        tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
            crate::app::handle_drop(&handle, paths);
        }
        _ => {}
    });
    Ok(window)
}

/// Create (if needed), show and focus the companion. UI thread only.
pub fn reveal(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    let window = ensure(app)?;
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    if let Some(state) = state(app) {
        lock(&state).hidden_since = None;
    }
    Ok(window)
}

/// `reveal` from any thread without waiting (safe from the menu handler).
pub fn show(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Err(error) = reveal(&handle) {
            eprintln!("Ledgeling could not open the companion: {error}");
        }
    });
}

/// Deliver an event to the companion once it can hear it: now if it is ready,
/// otherwise when it announces itself through `companion_ready`.
pub fn send(app: &tauri::AppHandle, event: &str, payload: impl Serialize) {
    let value = serde_json::to_value(payload).unwrap_or(Value::Null);
    let ready = match state(app) {
        Some(state) => {
            let mut inner = lock(&state);
            if !inner.ready {
                inner.pending.push((event.to_string(), value.clone()));
            }
            inner.ready
        }
        None => false,
    };
    if ready {
        let _ = app.emit_to(LABEL, event, value);
    }
}

/// Emit only if the companion exists; it re-reads everything when it opens.
pub fn emit_if_open(app: &tauri::AppHandle, event: &str, payload: impl Serialize + Clone) {
    if app.get_webview_window(LABEL).is_some() {
        let _ = app.emit_to(LABEL, event, payload);
    }
}

#[derive(Serialize)]
pub struct Pending {
    event: String,
    payload: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ready {
    ui: Option<Value>,
    pending: Vec<Pending>,
}

/// The webview's listeners are up: hand over the saved UI state and anything
/// sent while it was loading, and start emitting directly.
#[tauri::command]
pub fn companion_ready(
    window: WebviewWindow,
    state: State<'_, CompanionState>,
) -> Result<Ready, String> {
    if window.label() != LABEL {
        return Err("This control belongs to the companion window.".into());
    }
    let mut inner = lock(&state);
    inner.ready = true;
    let pending = std::mem::take(&mut inner.pending)
        .into_iter()
        .map(|(event, payload)| Pending { event, payload })
        .collect();
    let ui = inner
        .ui
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok());
    Ok(Ready { ui, pending })
}

#[tauri::command]
pub fn companion_save_ui(
    window: WebviewWindow,
    state: State<'_, CompanionState>,
    json: String,
) -> Result<(), String> {
    if window.label() != LABEL {
        return Err("This control belongs to the companion window.".into());
    }
    save_ui(&state.dir, &json)?;
    lock(&state).ui = Some(json);
    Ok(())
}

/// Called periodically from the update loop: destroy the companion once it has
/// been hidden past the grace period and nothing would be lost.
pub fn maintain(app: &tauri::AppHandle) {
    let Some(state) = state(app) else { return };
    let (hidden_for, ui) = {
        let inner = lock(&state);
        (inner.hidden_since.map(|t| t.elapsed()), inner.ui.clone())
    };
    if hidden_for.is_none_or(|d| d < GRACE) {
        return;
    }
    let Some(window) = app.get_webview_window(LABEL) else {
        lock(&state).hidden_since = None;
        return;
    };
    if window.is_visible().unwrap_or(true) {
        lock(&state).hidden_since = None;
        return;
    }
    let busy = app
        .try_state::<crate::tasks::TaskService>()
        .map(|tasks| match tasks.list() {
            Ok(list) => list.iter().any(|t| task_attention(&t.status).is_some()),
            Err(_) => true,
        })
        .unwrap_or(false);
    if should_destroy(hidden_for, GRACE, busy, is_volatile(ui.as_deref())) {
        let _ = crate::app::on_main(app, |handle| {
            if let Some(window) = handle.get_webview_window(LABEL) {
                let _ = window.destroy();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destroys_only_after_grace_when_idle() {
        let g = Duration::from_secs(600);
        assert!(!should_destroy(None, g, false, false));
        assert!(!should_destroy(
            Some(Duration::from_secs(599)),
            g,
            false,
            false
        ));
        assert!(should_destroy(Some(g), g, false, false));
        assert!(!should_destroy(Some(g * 2), g, true, false));
        assert!(!should_destroy(Some(g * 2), g, false, true));
    }

    #[test]
    fn ui_state_round_trips_and_rejects_junk() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_ui(dir.path()), None);
        let json = r#"{"page":"studio","prompt":"hi","volatile":false}"#;
        save_ui(dir.path(), json).unwrap();
        assert_eq!(load_ui(dir.path()).as_deref(), Some(json));
        assert!(save_ui(dir.path(), "[1]").is_err());
        assert!(save_ui(dir.path(), "nope").is_err());
        assert_eq!(load_ui(dir.path()).as_deref(), Some(json));
        std::fs::write(dir.path().join(UI_FILE), "garbage").unwrap();
        assert_eq!(load_ui(dir.path()), None);
    }

    #[test]
    fn volatile_flag_is_read_from_ui_state() {
        assert!(is_volatile(Some(r#"{"volatile":true}"#)));
        assert!(!is_volatile(Some(r#"{"volatile":false}"#)));
        assert!(!is_volatile(Some("{}")));
        assert!(!is_volatile(None));
    }
}
