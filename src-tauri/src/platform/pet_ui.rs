//! Explicit pet interaction UI. Native panels do not activate the dashboard.
use crate::{
    app::Shared,
    tasks::{
        types::{effective_connection, ProviderKind, Task, TaskRequest},
        TaskService,
    },
};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use tauri::{Emitter, Manager};
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
    fn perch_bubble_show(json: *const std::ffi::c_char);
    fn perch_bubble_update(json: *const std::ffi::c_char);
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
#[cfg(target_os = "macos")]
extern "C" fn event(raw: *const std::ffi::c_char) {
    if raw.is_null() {
        return;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }.to_string_lossy();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
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
fn handle_event(app: &tauri::AppHandle, v: serde_json::Value) -> Result<(), String> {
    let service = app.state::<TaskService>();
    let pet = v["petId"].as_u64().unwrap_or(0);
    match v["type"].as_str().unwrap_or("") {
        "bind" => {
            let raw = v["folder"].as_str().ok_or("Missing drop folder")?;
            let path = std::path::Path::new(raw)
                .canonicalize()
                .map_err(|_| "The dropped folder is unavailable")?;
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
                    "folder": path,
                    "message": format!("Ready in {path}. Right-click me when you need a hand."),
                }),
            );
        }
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
            service.start(request)?;
        }
        // The bubble asks lazily: model listing can spawn a Codex process or
        // hit the network, so it happens here on the event thread, never
        // inside show_pet_bubble.
        "models" => {
            let connection = effective_connection(&service.config(), Some(pet));
            if matches!(connection.kind, ProviderKind::None) {
                return Err("Connect a model in Connections first.".into());
            }
            let models = if matches!(connection.kind, ProviderKind::Codex) {
                crate::tasks::runtimes::capabilities(&connection).map(|c| c.models)
            } else {
                crate::tasks::providers::list_models(&connection)
            }?;
            send(app, serde_json::json!({"petId": pet, "models": models}));
        }
        "model" => {
            let model = v["model"]
                .as_str()
                .ok_or("Missing model")?
                .to_string();
            let mut config = service.config();
            let mut connection = effective_connection(&config, Some(pet));
            if matches!(connection.kind, ProviderKind::None) {
                return Err("Connect a model in Connections first.".into());
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
            connection.model = model;
            config.per_pet.insert(pet, connection);
            service.set_config(&config)?;
            let _ = app.emit_to("main", "connections:changed", ());
            send(
                app,
                serde_json::json!({"petId": pet, "message": "Model saved for this companion. The next task uses it."}),
            );
        }
        "approve" => service.answer(
            v["id"].as_i64().ok_or("Missing task")?,
            v["approvalId"].as_u64().ok_or("Missing approval")?,
            v["accept"].as_bool().unwrap_or(false),
        )?,
        "cancel" => service.cancel(v["id"].as_i64().ok_or("Missing task")?)?,
        "undo" => service.undo(v["id"].as_i64().ok_or("Missing task")?)?,
        "settings" => {
            let handle = app.clone();
            app.run_on_main_thread(move || {
                if let Some(main) = handle.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = handle.emit_to("main", "assistant:open", Some(pet));
                }
            })
            .map_err(|e| e.to_string())?;
        }
        "dragEnded" => {
            let shared = app.state::<Shared>();
            let mut state = shared.lock().map_err(|_| "Pet state unavailable")?;
            for p in &mut state.engine.pets {
                if p.id == pet && p.is_held() {
                    p.release();
                }
            }
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
/// create folders, so a chosen folder is optional and falls back to Perch's
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
            return Err("Connect a model in Connections to ask questions.".into());
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
        // The agent start approval is the consent for cloud use; a model runs
        // only with the bubble's explicit cloud checkbox.
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
    let (title, _) =
        action_texts(preset).ok_or("Pick a built-in action, or connect a model in Connections.")?;
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
    #[cfg(target_os = "macos")]
    {
        let text = value.to_string();
        let _ = app.run_on_main_thread(move || {
            if let Ok(s) = std::ffi::CString::new(text) {
                unsafe {
                    perch_bubble_update(s.as_ptr());
                }
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, value);
    }
}
pub fn message(app: &tauri::AppHandle, text: &str) {
    send(app, serde_json::json!({"message":text}));
}
pub fn update(app: &tauri::AppHandle, task: &Task) {
    if let Ok(v) = serde_json::to_value(task) {
        send(app, v);
    }
}
pub fn overlay(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = window.ns_window() {
        unsafe {
            perch_overlay_panel(pointer);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}
#[tauri::command]
pub async fn pet_folder(pet_id: u64) -> Option<String> {
    folder(pet_id)
}
#[tauri::command]
pub async fn show_pet_bubble(app: tauri::AppHandle, pet_id: u64) -> Result<(), String> {
    let name = {
        let shared = app.state::<Shared>();
        let state = shared.lock().map_err(|_| "Pet state unavailable")?;
        state
            .engine
            .pets
            .iter()
            .find(|p| p.id == pet_id)
            .map(|p| p.pack.manifest.name.clone())
            .ok_or("This pet is no longer on the desktop")?
    };
    #[cfg(target_os = "macos")]
    {
        // The bubble states what a mode still needs before it is picked, so it
        // has to know which runtime is connected and whether it is a cloud
        // API, whose Actions need explicit consent. Per-pet overrides count:
        // the bubble must describe the connection this pet would run on.
        let config = app.state::<TaskService>().config();
        let connection = effective_connection(&config, Some(pet_id));
        let runtime = match connection.kind {
            ProviderKind::None => "none",
            ProviderKind::Ollama => "ollama",
            ProviderKind::Codex => "codex",
            ProviderKind::Claude => "claude",
            _ => "model",
        };
        let remote = matches!(
            connection.kind,
            ProviderKind::Openai | ProviderKind::Anthropic | ProviderKind::Compatible
        );
        let json=serde_json::json!({"petId":pet_id,"name":name,"runtime":runtime,"remote":remote,"model":connection.model,"folder":folder(pet_id).unwrap_or_default()}).to_string();
        app.run_on_main_thread(move || {
            if let Ok(s) = std::ffi::CString::new(json) {
                unsafe {
                    perch_bubble_show(s.as_ptr());
                }
            }
        })
        .map_err(|e| e.to_string())?;
        // Only an in-flight task belongs in a freshly opened bubble; a task that
        // finished in a prior session is history, not this conversation.
        if let Some(task) = app.state::<TaskService>().list()?.into_iter().find(|t| {
            t.request.pet_id == Some(pet_id)
                && matches!(
                    t.status.as_str(),
                    "queued" | "running" | "waiting" | "cancelling" | "undoing"
                )
        }) {
            update(&app, &task);
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, name);
        Err("Pet speech bubbles are currently available on macOS only.".into())
    }
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, pet_id);
        Err("Native folder drops are currently available on macOS only.".into())
    }
}
