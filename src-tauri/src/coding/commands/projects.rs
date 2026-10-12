use super::{code_only, live_pets, resolve_worktree, with};
use crate::coding::{
    git, pty,
    store::{Project, StoreData},
    SharedCoding,
};
use serde::Serialize;
use std::path::Path;
use tauri::{State, WebviewWindow};

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
    let root = resolve_worktree(&state, &project)?;
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
    let root = resolve_worktree(&state, &project)?;
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
    let root = resolve_worktree(&state, &project)?;
    let tree = resolve_worktree(&state, &path)?;
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
    let tree = resolve_worktree(&state, &worktree)?
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
