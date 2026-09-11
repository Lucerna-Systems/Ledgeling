use super::{
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
    config: Connections,
) -> Result<(), String> {
    main_only(&window)?;
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
pub async fn task_models(
    window: WebviewWindow,
    config: Connection,
) -> Result<Vec<String>, String> {
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
    config: Connection,
) -> Result<Capabilities, String> {
    main_only(&window)?;
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
    let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid URL".to_string())?;
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
