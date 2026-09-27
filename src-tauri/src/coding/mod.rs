//! Coding mode: an opt-in developer workspace in its own window.
//!
//! Nothing here runs unless the user turns on Developer tools. The service is
//! built on first use and dropped (killing every process it started) when the
//! setting goes off, so ordinary users pay nothing for it.
pub mod commands;
pub mod fs;
pub mod git;
pub mod ports;
pub mod pty;
pub mod run;
pub mod store;
pub mod tools;
pub mod watch;
pub mod window;

use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

pub use store::Store;

/// Everything coding mode holds while it is on. Lives behind its own lock,
/// never the engine's, so terminal output can stream without touching pets.
pub struct CodingService {
    pub store: Store,
    pub ptys: HashMap<u32, pty::Session>,
    pub watchers: HashMap<PathBuf, watch::Watcher>,
    next_pty: u32,
}

impl CodingService {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            store: Store::load(&config_dir.join("coding")),
            ptys: HashMap::new(),
            watchers: HashMap::new(),
            next_pty: 1,
        }
    }

    pub fn next_pty_id(&mut self) -> u32 {
        let id = self.next_pty;
        self.next_pty += 1;
        id
    }

    /// Stops every terminal and run process and every watcher.
    pub fn shutdown(&mut self) {
        for (_, mut session) in self.ptys.drain() {
            session.kill();
        }
        self.watchers.clear();
    }
}

impl Drop for CodingService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The slot the Tauri state holds. `None` until coding mode is first opened.
pub type SharedCoding = Arc<Mutex<Option<CodingService>>>;

/// Resolve `relative` inside `root`, refusing anything that would leave it:
/// `..`, absolute paths, and symlinks pointing elsewhere.
pub fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "The project folder is no longer available.".to_string())?;
    let relative = Path::new(relative);
    let mut joined = root.clone();
    for component in relative.components() {
        match component {
            Component::Normal(part) => joined.push(part),
            Component::CurDir => {}
            _ => return Err("That path is outside the project.".into()),
        }
    }
    // The file itself may not exist yet (create, rename target); its nearest
    // existing ancestor must still resolve inside the root.
    let mut probe = joined.clone();
    loop {
        if let Ok(real) = probe.canonicalize() {
            if !real.starts_with(&root) {
                return Err("That path is outside the project.".into());
            }
            break;
        }
        if !probe.pop() {
            return Err("That path is outside the project.".into());
        }
    }
    Ok(joined)
}

/// A path relative to `root`, with forward slashes, for the frontend.
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = path.strip_prefix(&root).ok()?;
    Some(
        path.components()
            .filter_map(|c| match c {
                Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_cannot_escape_the_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        assert!(inside(dir.path(), "src/main.rs").is_ok());
        assert!(inside(dir.path(), "new/deep/file.txt").is_ok());
        assert!(inside(dir.path(), "../outside").is_err());
        assert!(inside(dir.path(), "src/../../outside").is_err());
        assert!(inside(dir.path(), "/etc/passwd").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_out_of_the_project_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), dir.path().join("link")).unwrap();
        assert!(inside(dir.path(), "link/secret").is_err());
    }
}
