use super::{
    apple,
    providers::{self, ModelProvider},
    runtimes, setup,
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
/// The coding window's Chat tab runs ordinary tasks in the selected worktree.
fn main_or_code(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(feature = "coding")]
    if window.label() == crate::coding::window::LABEL {
        return Ok(());
    }
    main_only(window)
}
#[tauri::command]
pub async fn task_list(
    window: WebviewWindow,
    state: State<'_, TaskService>,
) -> Result<Vec<Task>, String> {
    main_or_code(&window)?;
    state.list()
}
#[tauri::command]
pub async fn task_start(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    request: TaskRequest,
) -> Result<Task, String> {
    main_or_code(&window)?;
    let pet = crate::app::task_pet(window.app_handle(), request.pet_id);
    state.start_for(request, pet)
}
#[tauri::command]
pub async fn task_rename(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
    title: String,
) -> Result<Task, String> {
    main_or_code(&window)?;
    state.rename(id, &title)
}
#[tauri::command]
pub async fn task_delete(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
) -> Result<(), String> {
    main_or_code(&window)?;
    state.delete(id)
}
#[tauri::command]
pub async fn task_cancel(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
) -> Result<(), String> {
    main_or_code(&window)?;
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
    main_or_code(&window)?;
    state.answer(id, approval_id, accept)
}
#[tauri::command]
pub async fn task_undo(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    id: i64,
) -> Result<(), String> {
    main_or_code(&window)?;
    state.undo(id)
}
#[tauri::command]
pub async fn task_connections(
    window: WebviewWindow,
    state: State<'_, TaskService>,
) -> Result<Connections, String> {
    main_or_code(&window)?;
    Ok(state.config())
}
#[tauri::command]
pub async fn task_save_connections(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    config: Connections,
) -> Result<Connections, String> {
    main_only(&window)?;
    let config = setup::accept_choices(&state.config(), config)?;
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
    state.set_config(&config)?;
    Ok(config)
}
/// Adds a provider, or saves changes to one already added. `accept` is the
/// user ticking the terms shown by `task_provider_terms`; the version and
/// time are stamped here, never taken from the frontend.
#[tauri::command]
pub async fn task_add_provider(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    kind: ProviderKind,
    setup: ProviderSetup,
    accept: bool,
) -> Result<Connections, String> {
    main_only(&window)?;
    let mut config = state.config();
    setup::add(
        &mut config,
        kind,
        setup,
        accept,
        chrono::Utc::now().timestamp(),
    )?;
    state.set_config(&config)?;
    Ok(config)
}
#[tauri::command]
pub async fn task_remove_provider(
    window: WebviewWindow,
    state: State<'_, TaskService>,
    kind: ProviderKind,
) -> Result<Connections, String> {
    main_only(&window)?;
    let mut config = state.config();
    setup::remove(&mut config, &kind);
    state.set_config(&config)?;
    Ok(config)
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
    // Testing runs a real completion, so the on-device model is gated exactly
    // like a task. Other probes send only a fixed synthetic prompt.
    if config.kind == ProviderKind::Apple && !setup::ready(&state.config(), &config.kind) {
        return Err(setup::not_ready(&config.kind));
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
/// Every provider's terms and their version, so the providers page never has
/// to hardcode the text or the version the backend checks.
#[tauri::command]
pub async fn task_provider_terms(window: WebviewWindow) -> Result<Value, String> {
    main_only(&window)?;
    let kinds = [
        ProviderKind::Apple,
        ProviderKind::Codex,
        ProviderKind::Claude,
        ProviderKind::Ollama,
        ProviderKind::Openai,
        ProviderKind::Anthropic,
        ProviderKind::Compatible,
    ];
    let mut terms = serde_json::Map::new();
    for kind in kinds {
        if let Some((version, text)) = setup::terms(&kind) {
            let key = serde_json::to_value(&kind).map_err(|e| e.to_string())?;
            terms.insert(
                key.as_str().unwrap_or_default().into(),
                serde_json::json!({ "version": version, "text": text }),
            );
        }
    }
    Ok(Value::Object(terms))
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
    main_or_code(&window)?;
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
