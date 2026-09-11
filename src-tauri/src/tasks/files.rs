use super::{types::Operation, Cancel};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MAX_TEXT: usize = 120_000;
#[derive(Clone)]
pub struct Scope {
    pub roots: Vec<PathBuf>,
    pub attachments: Vec<PathBuf>,
}
impl Scope {
    pub fn new(roots: &[String], attachments: &[String]) -> Result<Self, String> {
        let resolve = |s: &String| -> Result<PathBuf, String> {
            let p = Path::new(s);
            no_links(p)?;
            p.canonicalize().map_err(|_| format!("Cannot access {s}"))
        };
        let roots: Vec<_> = roots.iter().map(resolve).collect::<Result<_, _>>()?;
        if roots.iter().any(|p| !p.is_dir() || p.parent().is_none()) {
            return Err("Select a specific folder, not the entire disk.".into());
        }
        let attachments: Vec<_> = attachments.iter().map(resolve).collect::<Result<_, _>>()?;
        if attachments.iter().any(|p| !p.is_file()) {
            return Err("Attachments must be files.".into());
        }
        Ok(Self { roots, attachments })
    }
    pub fn path(&self, input: &str, write: bool) -> Result<PathBuf, String> {
        let raw = Path::new(input);
        let path = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            self.roots
                .first()
                .ok_or("Choose a folder first.")?
                .join(raw)
        };
        no_links(&path)?;
        let allowed = self.roots.iter().any(|r| path.starts_with(r) && path != *r)
            || (!write && self.attachments.contains(&path));
        if !allowed {
            return Err(format!(
                "Outside the selected task scope: {}",
                path.display()
            ));
        }
        Ok(path)
    }
    pub fn list(&self, cancel: &Cancel) -> Result<Vec<PathBuf>, String> {
        fn walk(
            p: &Path,
            out: &mut Vec<PathBuf>,
            cancel: &Cancel,
            depth: usize,
        ) -> Result<(), String> {
            cancel.check()?;
            if depth > 20 {
                return Err("Folder nesting exceeds 20 levels. Select a smaller folder.".into());
            }
            for entry in fs::read_dir(p).map_err(|e| e.to_string())? {
                cancel.check()?;
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let ty = entry.file_type().map_err(|e| e.to_string())?;
                if ty.is_symlink() {
                    continue;
                }
                if ty.is_dir() {
                    walk(&entry.path(), out, cancel, depth + 1)?;
                }
                if ty.is_file() {
                    out.push(entry.path());
                }
                if out.len() > 2000 {
                    return Err(
                        "This task is limited to 2,000 files. Choose a smaller folder.".into(),
                    );
                }
            }
            Ok(())
        }
        let mut out = self.attachments.clone();
        for root in &self.roots {
            no_links(root)?;
            walk(root, &mut out, cancel, 0)?;
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}
/// The folder tasks fall back to when no folder was chosen. Created on demand
/// so a fresh install works immediately; the agent may add subfolders inside.
pub fn default_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or("The home folder is unavailable.")?;
    let root = home.join("Documents").join("Perch");
    fs::create_dir_all(&root).map_err(|e| format!("Could not create {}: {e}", root.display()))?;
    root.canonicalize().map_err(|e| e.to_string())
}

pub fn no_links(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("An absolute path is required.".into());
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, Component::ParentDir | Component::CurDir) {
            return Err("Relative path traversal is not allowed.".into());
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "Symbolic links are not followed: {}",
                    current.display()
                ))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
pub fn fingerprint(path: &Path, cancel: &Cancel) -> Result<String, String> {
    no_links(path)?;
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let metadata = f.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 1_000_000_000 {
        return Err("Only regular files up to 1 GB are supported.".into());
    }
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        cancel.check()?;
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn read_text(path: &Path, cancel: &Cancel) -> Result<String, String> {
    no_links(path)?;
    cancel.check()?;
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 10_000_000 {
        return Err("Document exceeds the 10 MB reading limit.".into());
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let text = if ext == "pdf" {
        pdf_extract::extract_text(path).map_err(|_| {
            "Unable to read this PDF. Encrypted and scanned PDFs are not supported.".to_string()
        })?
    } else {
        if !matches!(
            ext.as_str(),
            "txt"
                | "md"
                | "csv"
                | "json"
                | "rs"
                | "ts"
                | "js"
                | "vue"
                | "py"
                | "toml"
                | "yaml"
                | "yml"
                | "html"
                | "css"
                | "log"
                | "xml"
                | ""
        ) {
            return Err("Unsupported format. Use text, Markdown, CSV or a text-based PDF; OCR is not included.".into());
        }
        fs::read_to_string(path).map_err(|_| "This file is not UTF-8 text.".to_string())?
    };
    cancel.check()?;
    if text.trim().is_empty() {
        return Err("No text found. Scanned documents need OCR, which is not included.".into());
    }
    if text.len() > MAX_TEXT {
        return Err("Document text exceeds 120 KB. Select a smaller document or excerpt.".into());
    }
    Ok(text)
}
pub fn prepare(
    scope: &Scope,
    kind: &str,
    source: Option<&str>,
    destination: &str,
    content: Option<&str>,
    cancel: &Cancel,
) -> Result<Operation, String> {
    if !matches!(kind, "move" | "write") {
        return Err("Only move and new-file write operations are supported.".into());
    }
    let dest = scope.path(destination, true)?;
    if dest.exists() {
        return Err(format!(
            "Destination already exists: {}. Nothing will be overwritten.",
            dest.display()
        ));
    }
    let (source, fp) = if kind == "move" {
        let src = scope.path(source.ok_or("Move requires a source")?, true)?;
        let fp = fingerprint(&src, cancel)?;
        (Some(src.to_string_lossy().into_owned()), Some(fp))
    } else {
        if content.unwrap_or("").len() > MAX_TEXT {
            return Err("Generated file exceeds 120 KB.".into());
        }
        (None, None)
    };
    Ok(Operation {
        kind: kind.into(),
        source,
        destination: dest.to_string_lossy().into_owned(),
        content: content.map(str::to_owned),
        fingerprint: fp,
        result_fingerprint: None,
        state: "planned".into(),
    })
}
pub fn validate_plan(scope: &Scope, ops: &[Operation], cancel: &Cancel) -> Result<(), String> {
    let mut destinations = std::collections::HashSet::new();
    let mut sources = std::collections::HashSet::new();
    for op in ops {
        cancel.check()?;
        let destination = scope.path(&op.destination, true)?;
        if destination.exists() || !destinations.insert(destination) {
            return Err(format!("Conflicting destination: {}", op.destination));
        }
        if let Some(src) = &op.source {
            let src_path = scope.path(src, true)?;
            if !sources.insert(src.clone())
                || Some(fingerprint(&src_path, cancel)?) != op.fingerprint
            {
                return Err(format!("Source changed or used twice: {src}"));
            }
        }
    }
    Ok(())
}
/// The caller persists `applying` BEFORE entering this function. No overwrite
/// rename is used: hard-link creation and create_new fail if a target appeared.
pub fn apply(scope: &Scope, op: &mut Operation, cancel: &Cancel) -> Result<(), String> {
    validate_plan(scope, std::slice::from_ref(op), cancel)?;
    let dest = scope.path(&op.destination, true)?;
    fs::create_dir_all(dest.parent().ok_or("Missing parent")?).map_err(|e| e.to_string())?;
    no_links(&dest)?;
    cancel.check()?;
    if let Some(src) = &op.source {
        fs::hard_link(src, &dest)
            .map_err(|e| format!("Move could not start (same-volume files required): {e}"))?;
        // Once the destination exists, complete this atomic unit even if cancelled.
        if let Err(e) = fs::remove_file(src) {
            return Err(format!(
                "Destination created but source could not be removed: {e}. Review the journal."
            ));
        }
        op.result_fingerprint = op.fingerprint.clone();
    } else {
        let content = op.content.as_deref().unwrap_or("");
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)
            .map_err(|e| e.to_string())?;
        f.write_all(content.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        op.result_fingerprint = Some(format!("{:x}", Sha256::digest(content.as_bytes())));
    }
    op.state = "applied".into();
    Ok(())
}
pub fn undo(scope: &Scope, op: &mut Operation, cancel: &Cancel) -> Result<(), String> {
    if !matches!(op.state.as_str(), "applied" | "applying" | "undoing") {
        return Ok(());
    }
    let dest = scope.path(&op.destination, true)?;
    let expected = op
        .result_fingerprint
        .clone()
        .or(op.fingerprint.clone())
        .or_else(|| {
            op.content
                .as_ref()
                .map(|c| format!("{:x}", Sha256::digest(c.as_bytes())))
        });
    if let Some(src) = &op.source {
        let src = scope.path(src, true)?;
        if !dest.exists() && src.exists() && Some(fingerprint(&src, cancel)?) == expected {
            op.state = "undone".into();
            return Ok(());
        }
    } else if !dest.exists() {
        op.state = "undone".into();
        return Ok(());
    }
    if Some(fingerprint(&dest, cancel)?) != expected {
        return Err(format!(
            "Undo conflict: {} changed since the task.",
            op.destination
        ));
    }
    if let Some(src) = &op.source {
        let src = scope.path(src, true)?;
        if src.exists() {
            // A crash can leave both hard links. Only remove the task's link
            // when they are provably the SAME file, never merely equal bytes.
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let a = fs::metadata(&src).map_err(|e| e.to_string())?;
                let b = fs::metadata(&dest).map_err(|e| e.to_string())?;
                if a.ino() != b.ino() || a.dev() != b.dev() {
                    return Err("Undo conflict: original path is occupied.".into());
                }
            }
            #[cfg(not(unix))]
            return Err("Undo conflict: original path is occupied. Review both files.".into());
        } else {
            fs::hard_link(&dest, &src).map_err(|e| e.to_string())?;
        }
    }
    fs::remove_file(&dest).map_err(|e| e.to_string())?;
    op.state = "undone".into();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Scope, Cancel) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let scope = Scope::new(&[root.to_string_lossy().into()], &[]).unwrap();
        (dir, scope, Cancel::default())
    }
    #[test]
    fn move_roundtrip_and_changed_destination_conflict() {
        let (_dir, scope, c) = setup();
        let root = &scope.roots[0];
        fs::write(root.join("a.txt"), "one").unwrap();
        let mut op = prepare(&scope, "move", Some("a.txt"), "sorted/a.txt", None, &c).unwrap();
        apply(&scope, &mut op, &c).unwrap();
        fs::write(root.join("sorted/a.txt"), "two").unwrap();
        assert!(undo(&scope, &mut op, &c).is_err());
        fs::write(root.join("sorted/a.txt"), "one").unwrap();
        undo(&scope, &mut op, &c).unwrap();
        assert!(root.join("a.txt").exists());
    }
    #[test]
    fn refuses_traversal_collisions_and_changed_sources() {
        let (_d, s, c) = setup();
        fs::write(s.roots[0].join("a"), "one").unwrap();
        assert!(s.path("../escape", true).is_err());
        assert!(prepare(&s, "write", None, "a", Some("x"), &c).is_err());
        let op = prepare(&s, "move", Some("a"), "b", None, &c).unwrap();
        assert!(validate_plan(&s, &[op.clone(), op.clone()], &c).is_err());
        fs::write(s.roots[0].join("a"), "changed").unwrap();
        assert!(validate_plan(&s, &[op], &c).is_err());
    }
    #[test]
    fn cancelled_action_does_not_write() {
        let (_d, s, c) = setup();
        let mut op = prepare(&s, "write", None, "new.txt", Some("hello"), &c).unwrap();
        c.cancel();
        assert!(apply(&s, &mut op, &c).is_err());
        assert!(!s.roots[0].join("new.txt").exists());
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected() {
        let (_d, s, _c) = setup();
        std::os::unix::fs::symlink("/tmp", s.roots[0].join("link")).unwrap();
        assert!(s.path("link/escape", true).is_err());
    }
    #[test]
    fn interrupted_write_can_be_undone() {
        let (_d, s, c) = setup();
        let mut op = prepare(&s, "write", None, "new.txt", Some("hello"), &c).unwrap();
        op.state = "applying".into();
        fs::write(s.roots[0].join("new.txt"), "hello").unwrap();
        undo(&s, &mut op, &c).unwrap();
        assert_eq!(op.state, "undone");
    }
}
