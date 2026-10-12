use super::{code_only, resolve_worktree, with};
use crate::coding::{fs, git, watch, SharedCoding};
use std::path::Path;
use tauri::{ipc::Channel, State, WebviewWindow};

// ---------------------------------------------------------------- changes

#[tauri::command]
pub async fn code_status(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<git::Status, String> {
    code_only(&window)?;
    git::status(&resolve_worktree(&state, &worktree)?)
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
    let root = resolve_worktree(&state, &worktree)?;
    crate::coding::inside(&root, &path)?;
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
    let root = resolve_worktree(&state, &worktree)?;
    for path in paths.iter().chain(&untracked) {
        crate::coding::inside(&root, path)?;
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
    fs::list(&resolve_worktree(&state, &worktree)?, &dir)
}

#[tauri::command]
pub async fn code_read(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    path: String,
) -> Result<fs::FileText, String> {
    code_only(&window)?;
    fs::read(&resolve_worktree(&state, &worktree)?, &path)
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
    fs::write(
        &resolve_worktree(&state, &worktree)?,
        &path,
        &text,
        expected,
    )
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
    let root = resolve_worktree(&state, &worktree)?;
    match op.as_str() {
        "create-file" => fs::create(&root, &path, false),
        "create-folder" => fs::create(&root, &path, true),
        "rename" => fs::rename(&root, &path, &to),
        "delete" => fs::delete(&root, &path),
        "reveal" => {
            let target = crate::coding::inside(&root, &path)?;
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
    let root = resolve_worktree(&state, &worktree)?;
    let watcher = watch::start(&root, channel)?;
    // One worktree is watched at a time: the one on screen.
    with(&state, |s| {
        s.watchers.clear();
        s.watchers.insert(root, watcher);
        Ok(())
    })
}
