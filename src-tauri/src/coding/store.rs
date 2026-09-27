//! Coding mode's own small state file: the projects the user opened, which pet
//! works in which worktree, and where the window was.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Project {
    /// The main checkout's root, canonical.
    pub path: String,
    pub name: String,
}

/// One pet ↔ one worktree. Pet ids are handed out per launch, so the pack is
/// what survives a restart; the id is kept to tell two pets of one pack apart
/// within a session.
#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Binding {
    pub worktree: String,
    pub pack_id: String,
    pub pet_id: Option<u64>,
}

/// Physical pixels, like every other coordinate in Rust.
#[derive(Clone, Copy, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct StoreData {
    pub projects: Vec<Project>,
    pub bindings: Vec<Binding>,
    pub window: Option<WindowRect>,
    pub pinned: bool,
    /// The worktree the window last showed.
    pub selected: Option<String>,
}

pub struct Store {
    dir: PathBuf,
    pub data: StoreData,
}

impl Store {
    pub fn load(dir: &Path) -> Self {
        let data = std::fs::read_to_string(dir.join("workspaces.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            dir: dir.to_path_buf(),
            data,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
        let temporary = self.dir.join("workspaces.json.tmp");
        std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, self.dir.join("workspaces.json")).map_err(|e| e.to_string())
    }

    pub fn add_project(&mut self, path: &Path) -> Project {
        let path_text = path.to_string_lossy().into_owned();
        if let Some(existing) = self.data.projects.iter().find(|p| p.path == path_text) {
            return existing.clone();
        }
        let project = Project {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path_text.clone()),
            path: path_text,
        };
        self.data.projects.push(project.clone());
        project
    }

    pub fn remove_project(&mut self, path: &str) {
        self.data.projects.retain(|p| p.path != path);
    }

    /// Give `worktree` to the pet, taking the pet away from any other worktree
    /// and the worktree away from any other pet.
    pub fn bind(&mut self, worktree: &str, pet_id: u64, pack_id: &str, live: &[(u64, String)]) {
        let current = self.resolve(live);
        self.data.bindings.retain(|b| {
            b.worktree != worktree && current.get(&b.worktree).copied() != Some(pet_id)
        });
        self.data.bindings.push(Binding {
            worktree: worktree.to_string(),
            pack_id: pack_id.to_string(),
            pet_id: Some(pet_id),
        });
    }

    pub fn unbind(&mut self, worktree: &str) {
        self.data.bindings.retain(|b| b.worktree != worktree);
    }

    /// Which live pet holds each worktree. An exact id match wins; otherwise
    /// the first unclaimed pet of the same pack, so bindings survive a restart.
    pub fn resolve(&self, live: &[(u64, String)]) -> HashMap<String, u64> {
        let mut claimed = HashSet::new();
        let mut out = HashMap::new();
        for binding in &self.data.bindings {
            if let Some(id) = binding.pet_id {
                if live.iter().any(|(i, p)| *i == id && *p == binding.pack_id) {
                    claimed.insert(id);
                    out.insert(binding.worktree.clone(), id);
                }
            }
        }
        for binding in &self.data.bindings {
            if out.contains_key(&binding.worktree) {
                continue;
            }
            if let Some((id, _)) = live
                .iter()
                .find(|(i, p)| *p == binding.pack_id && !claimed.contains(i))
            {
                claimed.insert(*id);
                out.insert(binding.worktree.clone(), *id);
            }
        }
        out
    }

    /// A pet with no worktree yet, for a new one.
    pub fn free_pet(&self, live: &[(u64, String)]) -> Option<(u64, String)> {
        let taken: HashSet<u64> = self.resolve(live).into_values().collect();
        live.iter().find(|(id, _)| !taken.contains(id)).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live() -> Vec<(u64, String)> {
        vec![
            (1, "budgie".into()),
            (2, "gecko".into()),
            (3, "budgie".into()),
        ]
    }

    #[test]
    fn a_pet_holds_one_worktree_and_a_worktree_one_pet() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path());
        store.bind("/a", 1, "budgie", &live());
        store.bind("/b", 1, "budgie", &live());
        let map = store.resolve(&live());
        assert_eq!(map.get("/b"), Some(&1));
        assert!(
            !map.contains_key("/a"),
            "moving a pet frees its old worktree"
        );
        store.bind("/b", 2, "gecko", &live());
        let map = store.resolve(&live());
        assert_eq!(map.get("/b"), Some(&2));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn bindings_survive_new_pet_ids_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path());
        store.bind("/a", 1, "budgie", &live());
        store.bind("/b", 3, "budgie", &live());
        store.save().unwrap();
        let reloaded = Store::load(dir.path());
        let after = vec![(10, "budgie".to_string()), (11, "budgie".to_string())];
        let map = reloaded.resolve(&after);
        assert_eq!(map.len(), 2);
        assert_ne!(map["/a"], map["/b"]);
    }

    #[test]
    fn a_free_pet_is_one_without_a_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path());
        store.bind("/a", 1, "budgie", &live());
        assert_eq!(store.free_pet(&live()).map(|p| p.0), Some(2));
    }
}
