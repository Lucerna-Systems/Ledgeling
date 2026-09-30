//! Reading and writing project files, always resolved inside the worktree.
use super::inside;
use serde::Serialize;
use std::{path::Path, time::UNIX_EPOCH};

/// Files larger than this open read-only as a notice instead of in the editor.
const MAX_EDITABLE: u64 = 5 * 1024 * 1024;

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    pub ignored: bool,
}

/// One directory level, folders first. Git-ignored entries are listed but
/// marked, so `node_modules` stays reachable without flooding the tree.
pub fn list(root: &Path, relative: &str) -> Result<Vec<Entry>, String> {
    let dir = inside(root, relative)?;
    let ignored = ignore_matcher(root);
    let mut out = Vec::new();
    for item in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let Ok(item) = item else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        if name == ".git" || name == ".DS_Store" {
            continue;
        }
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let path = if relative.is_empty() {
            name.clone()
        } else {
            format!("{}/{name}", relative.trim_end_matches('/'))
        };
        out.push(Entry {
            ignored: ignored.as_ref().is_some_and(|m| {
                item.path().starts_with(m.path())
                    && m.matched_path_or_any_parents(item.path(), is_dir)
                        .is_ignore()
            }),
            name,
            path,
            dir: is_dir,
        });
    }
    out.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// Built on the canonical root: the matcher panics on paths outside it, and
/// temporary folders on macOS are reached through a `/private` symlink.
pub fn ignore_matcher(root: &Path) -> Option<ignore::gitignore::Gitignore> {
    let root = root.canonicalize().ok()?;
    let mut builder = ignore::gitignore::GitignoreBuilder::new(&root);
    builder.add(root.join(".gitignore"));
    builder.build().ok()
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub text: String,
    /// Milliseconds since the epoch, handed back on save to detect clobbering.
    pub modified: u64,
    pub binary: bool,
    pub too_large: bool,
}

fn modified_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn read(root: &Path, relative: &str) -> Result<FileText, String> {
    let path = inside(root, relative)?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let modified = modified_ms(&path);
    if size > MAX_EDITABLE {
        return Ok(FileText {
            text: String::new(),
            modified,
            binary: false,
            too_large: true,
        });
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let binary = bytes.iter().take(8000).any(|b| *b == 0);
    Ok(FileText {
        text: if binary {
            String::new()
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        },
        modified,
        binary,
        too_large: false,
    })
}

/// Save, unless the file changed on disk since it was read — an agent may
/// have written it in the meantime. `expected` of 0 skips the check.
pub fn write(root: &Path, relative: &str, text: &str, expected: u64) -> Result<u64, String> {
    let path = inside(root, relative)?;
    if expected != 0 && path.exists() && modified_ms(&path) != expected {
        return Err("changed-on-disk".into());
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(modified_ms(&path))
}

pub fn create(root: &Path, relative: &str, dir: bool) -> Result<(), String> {
    let path = inside(root, relative)?;
    if path.exists() {
        return Err("Something with that name already exists.".into());
    }
    if dir {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, "").map_err(|e| e.to_string())
    }
}

pub fn rename(root: &Path, from: &str, to: &str) -> Result<(), String> {
    let source = inside(root, from)?;
    let target = inside(root, to)?;
    if target.exists() {
        return Err("Something with that name already exists.".into());
    }
    std::fs::rename(source, target).map_err(|e| e.to_string())
}

pub fn delete(root: &Path, relative: &str) -> Result<(), String> {
    if relative.trim_matches('/').is_empty() {
        return Err("The project folder itself cannot be deleted here.".into());
    }
    let path = inside(root, relative)?;
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_puts_folders_first_and_marks_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join(".gitignore"), "dist/\n").unwrap();
        std::fs::create_dir(root.join("dist")).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("b.txt"), "").unwrap();
        let entries = list(root, "").unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["dist", "src", ".gitignore", "b.txt"]);
        assert!(entries[0].ignored);
        assert!(!entries[1].ignored);
    }

    #[test]
    fn a_save_does_not_clobber_a_newer_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one").unwrap();
        let read_once = read(dir.path(), "a.txt").unwrap();
        let saved = write(dir.path(), "a.txt", "two", read_once.modified).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "two"
        );
        assert_eq!(
            write(dir.path(), "a.txt", "three", saved.saturating_sub(5000)),
            Err("changed-on-disk".into())
        );
    }

    #[test]
    fn binary_files_are_not_opened_as_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.bin"), [0u8, 1, 2]).unwrap();
        assert!(read(dir.path(), "x.bin").unwrap().binary);
    }

    #[test]
    fn the_root_cannot_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(delete(dir.path(), "").is_err());
        assert!(delete(dir.path(), "/").is_err());
    }
}
