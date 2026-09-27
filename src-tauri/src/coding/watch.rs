//! Live file changes for one worktree, so edits made by an agent show up in
//! the tree, editor and diff while they happen.
use notify::{RecursiveMode, Watcher as _};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
use tauri::ipc::Channel;

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    /// Worktree-relative paths of files that changed.
    pub paths: Vec<String>,
    /// Git's own state moved: a commit, a stage, a checkout.
    pub git: bool,
}

/// Dropping it stops the watch and its debounce thread.
pub struct Watcher {
    _inner: notify::RecommendedWatcher,
}

/// Bursts (an agent writing twenty files, a build) collapse into one message.
const DEBOUNCE: Duration = Duration::from_millis(180);

pub fn start(root: &Path, channel: Channel<Changes>) -> Result<Watcher, String> {
    let root: PathBuf = root.canonicalize().map_err(|e| e.to_string())?;
    let (sender, receiver) = mpsc::channel::<PathBuf>();
    let mut inner = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if let Ok(event) = result {
            for path in event.paths {
                let _ = sender.send(path);
            }
        }
    })
    .map_err(|e| e.to_string())?;
    inner
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    let ignored = super::fs::ignore_matcher(&root);
    std::thread::spawn(move || {
        let mut pending = Changes::default();
        let mut seen = BTreeSet::new();
        let mut since: Option<Instant> = None;
        loop {
            let timeout = since.map_or(Duration::from_secs(3600), |start| {
                DEBOUNCE.saturating_sub(start.elapsed())
            });
            match receiver.recv_timeout(timeout) {
                Ok(path) => {
                    let Some(relative) = super::relative(&root, &path) else {
                        continue;
                    };
                    if let Some(git_path) = relative.strip_prefix(".git/") {
                        // Object writes are noise; these name a state change.
                        if matches!(git_path, "index" | "HEAD") || git_path.starts_with("refs/") {
                            pending.git = true;
                            since.get_or_insert_with(Instant::now);
                        }
                        continue;
                    }
                    if relative == ".git" || relative.is_empty() {
                        continue;
                    }
                    let is_ignored = ignored.as_ref().is_some_and(|m| {
                        path.starts_with(m.path())
                            && m.matched_path_or_any_parents(&path, path.is_dir())
                                .is_ignore()
                    });
                    if !is_ignored && seen.insert(relative) {
                        since.get_or_insert_with(Instant::now);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if since.take().is_some() {
                        pending.paths = std::mem::take(&mut seen).into_iter().collect();
                        if channel.send(std::mem::take(&mut pending)).is_err() {
                            break;
                        }
                    }
                }
                // The watcher was dropped: the worktree was closed.
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    Ok(Watcher { _inner: inner })
}
