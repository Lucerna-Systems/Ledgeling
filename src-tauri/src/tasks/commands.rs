use super::{
    apple,
    providers::{self, ModelProvider},
    runtimes,
    types::*,
    Cancel, TaskService,
};
use serde_json::Value;
use tauri::{Emitter, Manager, State, WebviewWindow};
fn main_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Task controls belong to the companion window.".into())
    }
}
#[tauri::command]
pub async fn task_list(
    window: WebviewWindow,
    state: State<'_, TaskService>,
) -> Result<Vec<Task>, String> {
    main_only(&window)?;
    state.list()
}
#[tauri::command]
pub async fn task_start(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    request: TaskRequest,
) -> Result<Task, String> {
    main_only(&window)?;
    state.start(request)
}
#[tauri::command]
pub async fn task_cancel(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
) -> Result<(), String> {
    main_only(&window)?;
    state.cancel(id)
}
#[tauri::command]
pub async fn task_approve(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
    approval_id: u64,
    accept: bool,
) -> Result<(), String> {
    main_only(&window)?;
    state.answer(id, approval_id, accept)
}
#[tauri::command]
pub async fn task_undo(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
) -> Result<(), String> {
    main_only(&window)?;
    state.undo(id)
}
#[tauri::command]
pub async fn task_connections(
    window: WebviewWindow,
    state: State<'_, TaskService>,
) -> Result<Connections, String> {
    main_only(&window)?;
    Ok(state.config())
}
#[tauri::command]
pub async fn task_save_connections(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    mut config: Connections,
) -> Result<(), String> {
    main_only(&window)?;
    // Acceptance is recorded here rather than trusted from the frontend: the
    // version is clamped to what this build can show, and the timestamp is
    // stamped on the transition so it records a real moment of consent.
    let stored = state.config();
    config.apple_terms.accepted_version = config
        .apple_terms
        .accepted_version
        .min(apple::TERMS_VERSION);
    config.apple_terms.accepted_at =
        if config.apple_terms.accepted_version > stored.apple_terms.accepted_version {
            chrono::Utc::now().timestamp()
        } else {
            stored.apple_terms.accepted_at
        };
    // `fm` accepts only one model, so a model id left over from a previously
    // chosen provider would otherwise be stored and shown as if it were in use.
    for connection in std::iter::once(&mut config.active).chain(config.per_pet.values_mut()) {
        if connection.kind == ProviderKind::Apple {
            connection.model = apple::MODEL.into();
        }
    }
    let uses_apple = std::iter::once(&config.active)
        .chain(config.per_pet.values())
        .any(|c| c.kind == ProviderKind::Apple);
    if uses_apple && !apple::terms_accepted(&config) {
        return Err(
            "Accept the Apple Intelligence terms before choosing the on-device model.".into(),
        );
    }
    // Per-pet overrides must clear the same endpoint bar as the default,
    // or they would become a bypass for the HTTPS/localhost rules.
    for connection in std::iter::once(&config.active).chain(config.per_pet.values()) {
        if matches!(
            connection.kind,
            ProviderKind::Ollama
                | ProviderKind::Openai
                | ProviderKind::Anthropic
                | ProviderKind::Compatible
        ) {
            providers::base(connection)?;
        }
    }
    state.set_config(&config)
}
#[tauri::command]
pub async fn task_secret(window: WebviewWindow, name: String, value: String) -> Result<(), String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || providers::set_secret(&name, &value))
        .await
        .map_err(|_| "Credential operation failed".to_string())?
}
#[tauri::command]
pub async fn task_models(window: WebviewWindow, config: Connection) -> Result<Vec<String>, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || match config.kind {
        ProviderKind::None => Err("Choose a provider first.".into()),
        ProviderKind::Codex => runtimes::capabilities(&config).map(|c| c.models),
        _ => providers::list_models(&config),
    })
    .await
    .map_err(|_| "Model listing failed".to_string())?
}
#[tauri::command]
pub async fn task_detect_providers(
    window: WebviewWindow,
    state: State<'_, TaskService>,
) -> Result<ProviderDetection, String> {
    main_only(&window)?;
    let config = state.config();
    tauri::async_runtime::spawn_blocking(move || providers::detect(&config))
        .await
        .map_err(|_| "Provider detection failed".to_string())
}
#[tauri::command]
pub async fn task_test_connection(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    config: Connection,
) -> Result<Capabilities, String> {
    main_only(&window)?;
    // Testing runs a real completion, so it is gated exactly like a task.
    if config.kind == ProviderKind::Apple && !apple::terms_accepted(&state.config()) {
        return Err("Accept the Apple Intelligence terms before testing this connection.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        if matches!(config.kind, ProviderKind::Codex | ProviderKind::Claude) {
            runtimes::capabilities(&config)
        } else {
            providers::HttpModel(config).capabilities(&Cancel::default())
        }
    })
    .await
    .map_err(|_| "Connection test failed".to_string())?
}
#[tauri::command]
pub async fn task_account(
    window: WebviewWindow,
    config: Connection,
    action: String,
) -> Result<Value, String> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || runtimes::account(&config, &action))
        .await
        .map_err(|_| "Account operation failed".to_string())?
}
/// Perch's own terms for the on-device provider, plus whether they have been
/// accepted, so the panel never has to hardcode the text or the version.
#[tauri::command]
pub async fn task_apple_terms(
    window: WebviewWindow,
    state: State<'_, TaskService>,
) -> Result<Value, String> {
    main_only(&window)?;
    let config = state.config();
    Ok(serde_json::json!({
        "version": apple::TERMS_VERSION,
        "text": apple::TERMS,
        "accepted": apple::terms_accepted(&config),
        "acceptedAt": config.apple_terms.accepted_at,
    }))
}
#[tauri::command]
pub async fn task_apple_open_settings(window: WebviewWindow) -> Result<(), String> {
    main_only(&window)?;
    apple::open_system_settings()
}
#[tauri::command]
pub async fn task_pick(window: WebviewWindow, folders: bool) -> Result<Vec<String>, String> {
    main_only(&window)?;
    let dialog = rfd::AsyncFileDialog::new().set_title(if folders {
        "Choose task folders"
    } else {
        "Attach documents"
    });
    let selected = if folders {
        dialog.pick_folders().await
    } else {
        dialog.pick_files().await
    };
    selected
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            p.path()
                .canonicalize()
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| e.to_string())
        })
        .collect()
}
#[tauri::command]
pub async fn task_open_url(window: WebviewWindow, url: String) -> Result<(), String> {
    main_only(&window)?;
    open_web_url(&url)
}
/// Open a result's link in the browser. Web links only: the dashboard and the
/// pet bubble both route through this one check.
pub fn open_web_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid URL".to_string())?;
    if !matches!(parsed.scheme(), "https" | "http")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Only web links can be opened.".into());
    }
    open::that(url).map_err(|_| "Could not open the browser.".into())
}
#[tauri::command]
pub async fn open_assistant(app: tauri::AppHandle, pet_id: Option<u64>) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or("Companion window missing")?;
    main.show().map_err(|e| e.to_string())?;
    main.set_focus().map_err(|e| e.to_string())?;
    app.emit_to("main", "assistant:open", pet_id)
        .map_err(|e| e.to_string())
}
