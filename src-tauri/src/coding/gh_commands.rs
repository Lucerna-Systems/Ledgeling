//! Tauri commands for the coding window's GitHub view. The window names a
//! worktree, never a repository: the repository is whatever that worktree's
//! remote points at, as `gh` sees it.
use super::{
    commands::{code_only, live_pets, with, worktree},
    git,
    github::{self, Action, BoardAction, Filter, Kind, Repo},
    SharedCoding,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::{State, WebviewWindow};

/// `gh` waits on the network, so it runs off the async runtime's threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| "The GitHub request stopped unexpectedly.".to_string())?
}

/// The worktree's repository, from the cache or found now.
async fn repo(state: &State<'_, SharedCoding>, tree: &Path) -> Result<Repo, String> {
    let key = tree.to_path_buf();
    if let Some(found) = with(state, |s| Ok(s.github.get(&key).cloned()))? {
        return Ok(found);
    }
    let dir = key.clone();
    let status = blocking(move || Ok(github::status(&dir))).await?;
    let found = status.repo.ok_or_else(|| {
        status
            .problem
            .unwrap_or_else(|| "This worktree has no GitHub remote.".into())
    })?;
    with(state, |s| {
        s.github.insert(key, found.clone());
        Ok(found)
    })
}

#[tauri::command]
pub async fn code_gh_status(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<github::GhStatus, String> {
    code_only(&window)?;
    let tree = self::worktree(&state, &worktree)?;
    let dir = tree.clone();
    let status = blocking(move || Ok(github::status(&dir))).await?;
    with(&state, |s| {
        match &status.repo {
            Some(repo) => s.github.insert(tree, repo.clone()),
            None => s.github.remove(&tree),
        };
        Ok(())
    })?;
    Ok(status)
}

#[tauri::command]
pub async fn code_gh_list(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    kind: Kind,
    filter: Filter,
    search: String,
) -> Result<Value, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::list(&repo.name_with_owner, kind, filter, &search)).await
}

#[tauri::command]
pub async fn code_gh_view(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    kind: Kind,
    number: u64,
) -> Result<Value, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::view(&repo.name_with_owner, kind, number)).await
}

#[tauri::command]
pub async fn code_gh_diff(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    number: u64,
) -> Result<String, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::diff(&repo.name_with_owner, number)).await
}

/// The pull request for the branch the worktree has checked out, if any.
#[tauri::command]
pub async fn code_gh_branch_pr(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<Option<Value>, String> {
    code_only(&window)?;
    let tree = self::worktree(&state, &worktree)?;
    let repo = repo(&state, &tree).await?;
    let Some(branch) = git::status(&tree)?.branch else {
        return Ok(None);
    };
    if branch == repo.default_branch {
        return Ok(None);
    }
    blocking(move || github::branch_pr(&repo.name_with_owner, &branch)).await
}

#[tauri::command]
pub async fn code_gh_meta(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<Value, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::meta(&repo.name_with_owner)).await
}

#[tauri::command]
pub async fn code_gh_act(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    action: Action,
) -> Result<String, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::act(&repo.name_with_owner, &action)).await
}

/// Where a remote-tracking branch for `branch` would live: its upstream's
/// remote when it has one, else `origin`.
fn remote_of(status: &git::Status) -> String {
    status
        .upstream
        .as_deref()
        .and_then(|u| u.split_once('/'))
        .map(|(remote, _)| remote.to_string())
        .unwrap_or_else(|| "origin".into())
}

#[tauri::command]
pub async fn code_gh_pr_draft(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<github::PrDraft, String> {
    code_only(&window)?;
    let tree = self::worktree(&state, &worktree)?;
    let repo = repo(&state, &tree).await?;
    let status = git::status(&tree)?;
    let branch = status
        .branch
        .clone()
        .ok_or("This worktree is not on a branch. Create or switch to one first.")?;
    if branch == repo.default_branch {
        return Err(format!(
            "You are on {branch}, the default branch. Make a worktree on a new branch for this change."
        ));
    }
    let remote = remote_of(&status);
    let base = repo.default_branch.clone();
    let log = git::run(
        &tree,
        &[
            "log",
            "--format=%s%x1f%b%x1e",
            &format!("{remote}/{base}..HEAD"),
        ],
    )
    .unwrap_or_default();
    let (title, body) = github::draft_from_log(&branch, &log);
    let pushed =
        status.upstream.as_deref() == Some(&format!("{remote}/{branch}")) && status.ahead == 0;
    Ok(github::PrDraft {
        commits: log.split('\x1e').filter(|r| !r.trim().is_empty()).count(),
        branch,
        base,
        title,
        body,
        pushed,
        remote,
    })
}

/// Pushes with the login PATH, so a `gh` credential helper can be found.
fn push(tree: &Path, remote: &str, branch: &str) -> Result<(), String> {
    if remote.starts_with('-') || branch.starts_with('-') {
        return Err("Those branch names are not valid.".into());
    }
    let mut command = git::command(tree);
    if let Some(path) = super::pty::login_path() {
        command.env("PATH", path);
    }
    let output = command
        .args(["push", "--set-upstream", remote, branch])
        .output()
        .map_err(|_| "Git could not be started.".to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(
        if message.contains("could not read Username") || message.contains("Authentication failed")
        {
            "Git could not sign in to push. Run `gh auth setup-git` in a terminal, then try again."
                .into()
        } else {
            message
        },
    )
}

/// Pushes the worktree's branch if needed, then opens a pull request for it.
#[tauri::command]
pub async fn code_gh_create_pr(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    base: String,
    title: String,
    body: String,
    draft: bool,
) -> Result<String, String> {
    code_only(&window)?;
    let tree = self::worktree(&state, &worktree)?;
    let repo = repo(&state, &tree).await?;
    blocking(move || {
        let status = git::status(&tree)?;
        let branch = status
            .branch
            .clone()
            .ok_or("This worktree is not on a branch.")?;
        let remote = remote_of(&status);
        if status.upstream.as_deref() != Some(&format!("{remote}/{branch}")) || status.ahead > 0 {
            push(&tree, &remote, &branch)?;
        }
        github::create_pr(&repo.name_with_owner, &branch, &base, &title, &body, draft)
    })
    .await
}

/// Checks a pull request out into its own worktree beside the project, and
/// gives it a pet, so reviewing never disturbs work in progress.
#[tauri::command]
pub async fn code_gh_checkout(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    number: u64,
) -> Result<String, String> {
    code_only(&window)?;
    let tree = self::worktree(&state, &worktree)?;
    let repo = repo(&state, &tree).await?;
    let root = git::main_root(&tree)?;
    let name_with_owner = repo.name_with_owner.clone();
    let created = blocking(move || {
        let pr = github::view(&name_with_owner, Kind::Pr, number)?;
        let head = pr["headRefName"].as_str().unwrap_or_default().to_string();
        let branch = if pr["isCrossRepository"].as_bool() == Some(true) {
            let owner = pr["headRepositoryOwner"]["login"]
                .as_str()
                .unwrap_or("fork");
            format!("{owner}-{head}")
        } else {
            head
        };
        if let Some(existing) = git::worktrees(&root)?
            .into_iter()
            .find(|t| t.branch.as_deref() == Some(branch.as_str()))
        {
            return Ok((PathBuf::from(existing.path), false));
        }
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
        let target = root
            .parent()
            .ok_or("The project has no parent folder.")?
            .join(format!("{name}.worktrees"))
            .join(format!("pr-{number}"));
        if target.exists() {
            return Err(format!("{} already exists.", target.display()));
        }
        let target_text = target.to_string_lossy().into_owned();
        git::run(&root, &["worktree", "add", "--detach", &target_text])?;
        if let Err(e) = github::checkout(&target, &name_with_owner, number, &branch) {
            let _ = git::remove_worktree(&root, &target_text, true);
            return Err(e);
        }
        Ok((target, true))
    })
    .await?;
    let (path, fresh) = created;
    let text = path
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();
    let live = live_pets(&app);
    with(&state, |s| {
        if fresh {
            if let Some((id, pack)) = s.store.free_pet(&live) {
                s.store.bind(&text, id, &pack, &live);
            }
        }
        s.store.data.selected = Some(text.clone());
        s.store.save()
    })?;
    Ok(text)
}

#[tauri::command]
pub async fn code_gh_projects(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
) -> Result<Vec<github::ProjectSummary>, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::projects(&repo)).await
}

#[tauri::command]
pub async fn code_gh_board(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    project: String,
) -> Result<github::Board, String> {
    code_only(&window)?;
    // Boards need no repository, but the worktree check still applies.
    self::worktree(&state, &worktree)?;
    blocking(move || github::board(&project)).await
}

#[tauri::command]
pub async fn code_gh_board_act(
    window: WebviewWindow,
    state: State<'_, SharedCoding>,
    worktree: String,
    project: String,
    action: BoardAction,
) -> Result<Option<String>, String> {
    code_only(&window)?;
    let repo = repo(&state, &self::worktree(&state, &worktree)?).await?;
    blocking(move || github::board_act(&project, &repo, &action)).await
}
