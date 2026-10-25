//! Git through the `git` executable the user already has. No libgit2: it keeps
//! the binary small and behaves exactly like the user's own terminal.
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
};

pub fn command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        // A credential prompt would hang an invisible process forever.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

/// `path` in a form git accepts: canonical Windows paths are verbatim
/// (`\\?\C:\...`), and git cannot create directories under those.
pub fn plain(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

pub fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = command(dir).args(args).output().map_err(|_| {
        "Git is not installed or not on your PATH. See Add-ons in the Worktrees tab.".to_string()
    })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if message.is_empty() {
            format!("git {} failed.", args.first().unwrap_or(&""))
        } else {
            message
        })
    }
}

/// The main checkout that owns `dir`, whether `dir` is it or one of its worktrees.
pub fn main_root(dir: &Path) -> Result<PathBuf, String> {
    let common = run(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map_err(|_| "That folder is not a git repository.".to_string())?;
    let common = PathBuf::from(common.trim());
    let root = if common.file_name().is_some_and(|n| n == ".git") {
        common.parent().map(Path::to_path_buf).unwrap_or(common)
    } else {
        // A bare repository: its worktrees are the only checkouts.
        common
    };
    root.canonicalize().map_err(|e| e.to_string())
}

#[derive(Clone, Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    pub branch: Option<String>,
    pub head: String,
    pub main: bool,
    pub locked: bool,
    pub prunable: bool,
}

pub fn parse_worktrees(text: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    for block in text.split("\n\n") {
        let mut tree = Worktree::default();
        let mut bare = false;
        for line in block.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "worktree" => tree.path = value.to_string(),
                "HEAD" => tree.head = value.chars().take(12).collect(),
                "branch" => tree.branch = Some(value.trim_start_matches("refs/heads/").to_string()),
                "bare" => bare = true,
                "locked" => tree.locked = true,
                "prunable" => tree.prunable = true,
                _ => {}
            }
        }
        if !tree.path.is_empty() && !bare {
            tree.main = out.is_empty();
            out.push(tree);
        }
    }
    out
}

pub fn worktrees(root: &Path) -> Result<Vec<Worktree>, String> {
    Ok(parse_worktrees(&run(
        root,
        &["worktree", "list", "--porcelain"],
    )?))
}

/// A new worktree on a new branch, next to the main checkout in
/// `<repo>.worktrees/<branch>` so it is easy to find and never nested inside.
pub fn add_worktree(root: &Path, branch: &str, base: &str) -> Result<PathBuf, String> {
    let valid = !branch.is_empty()
        && !branch.starts_with('-')
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'))
        && !branch.contains("..");
    if !valid {
        return Err("Use letters, numbers, dashes, dots or slashes for the branch name.".into());
    }
    if base.starts_with('-') {
        return Err("That base branch name is not valid.".into());
    }
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let parent = root.parent().ok_or("The project has no parent folder.")?;
    let target = plain(
        &parent
            .join(format!("{name}.worktrees"))
            .join(branch.replace('/', "-")),
    );
    if target.exists() {
        return Err(format!("{} already exists.", target.display()));
    }
    let target_text = target.to_string_lossy().into_owned();
    let mut args = vec!["worktree", "add", "-b", branch, &target_text];
    if !base.is_empty() {
        args.push(base);
    }
    run(root, &args)?;
    Ok(target)
}

pub fn remove_worktree(root: &Path, path: &str, force: bool) -> Result<(), String> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    let path = plain(Path::new(path)).to_string_lossy().into_owned();
    args.push(&path);
    run(root, &args).map(|_| ())
}

pub fn branches(root: &Path) -> Result<Vec<String>, String> {
    Ok(run(
        root,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )?
    .lines()
    .map(str::to_string)
    .collect())
}

#[derive(Clone, Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub original: Option<String>,
    /// Porcelain status letters: index (staged) and worktree (unstaged).
    pub staged: char,
    pub unstaged: char,
    pub untracked: bool,
    pub conflicted: bool,
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Clone, Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<FileChange>,
}

pub fn parse_status(text: &str) -> Status {
    let mut status = Status::default();
    let mut entries = text.split('\0').peekable();
    while let Some(entry) = entries.next() {
        if let Some(header) = entry.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.head" if value != "(detached)" => status.branch = Some(value.into()),
                "branch.upstream" => status.upstream = Some(value.into()),
                "branch.ab" => {
                    for part in value.split(' ') {
                        if let Some(n) = part.strip_prefix('+') {
                            status.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            status.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let mut change = FileChange::default();
        let fields: Vec<&str>;
        match entry.chars().next() {
            Some('1') => {
                fields = entry.splitn(9, ' ').collect();
                change.path = fields.get(8).copied().unwrap_or_default().into();
            }
            Some('2') => {
                fields = entry.splitn(10, ' ').collect();
                change.path = fields.get(9).copied().unwrap_or_default().into();
                change.original = entries.next().map(str::to_string);
            }
            Some('u') => {
                fields = entry.splitn(11, ' ').collect();
                change.path = fields.get(10).copied().unwrap_or_default().into();
                change.conflicted = true;
            }
            Some('?') => {
                change.path = entry[2..].into();
                change.untracked = true;
                change.unstaged = '?';
                change.staged = '.';
                status.files.push(change);
                continue;
            }
            _ => continue,
        }
        let xy: Vec<char> = fields.get(1).copied().unwrap_or("..").chars().collect();
        change.staged = xy.first().copied().unwrap_or('.');
        change.unstaged = xy.get(1).copied().unwrap_or('.');
        if !change.path.is_empty() {
            status.files.push(change);
        }
    }
    status
}

/// `git diff --numstat -z` against HEAD: path → (added, removed). Binary
/// files report `-` and are left without counts.
pub fn parse_numstat(text: &str) -> HashMap<String, (u32, u32)> {
    let mut out = HashMap::new();
    let mut entries = text.split('\0');
    while let Some(entry) = entries.next() {
        let mut parts = entry.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let path = if path.is_empty() {
            // A rename: the old and new names follow as their own entries.
            let _old = entries.next();
            entries.next().unwrap_or_default().to_string()
        } else {
            path.to_string()
        };
        if let (Ok(a), Ok(r)) = (added.parse(), removed.parse()) {
            out.insert(path, (a, r));
        }
    }
    out
}

pub fn status(dir: &Path) -> Result<Status, String> {
    let mut status = parse_status(&run(
        dir,
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
        ],
    )?);
    // A repository with no commits yet has no HEAD to count against.
    let counts = run(dir, &["diff", "--numstat", "-z", "HEAD"])
        .map(|text| parse_numstat(&text))
        .unwrap_or_default();
    for file in &mut status.files {
        if let Some((a, r)) = counts.get(&file.path) {
            file.added = Some(*a);
            file.removed = Some(*r);
        } else if file.untracked {
            file.added = std::fs::read(dir.join(&file.path))
                .ok()
                .filter(|bytes| bytes.len() < 2_000_000 && !bytes.contains(&0))
                .map(|bytes| bytecount_lines(&bytes));
            file.removed = Some(0);
        }
    }
    Ok(status)
}

fn bytecount_lines(bytes: &[u8]) -> u32 {
    let lines = bytes.iter().filter(|b| **b == b'\n').count();
    (lines + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"))) as u32
}

/// A file as committed at HEAD (or staged, for `index`), for the diff's left side.
/// A file that did not exist there is empty rather than an error.
pub fn original(dir: &Path, path: &str, index: bool) -> String {
    let spec = if index {
        format!(":{path}")
    } else {
        format!("HEAD:{path}")
    };
    run(dir, &["show", &spec]).unwrap_or_default()
}

pub fn stage(dir: &Path, paths: &[String]) -> Result<(), String> {
    let mut args = vec!["add", "--"];
    args.extend(paths.iter().map(String::as_str));
    run(dir, &args).map(|_| ())
}

pub fn unstage(dir: &Path, paths: &[String]) -> Result<(), String> {
    let mut args = vec!["restore", "--staged", "--"];
    args.extend(paths.iter().map(String::as_str));
    run(dir, &args).map(|_| ())
}

/// Throw away unstaged edits. Untracked files are deleted, since that is
/// what "discard" means for a file git has never seen.
pub fn discard(dir: &Path, paths: &[String], untracked: &[String]) -> Result<(), String> {
    if !paths.is_empty() {
        let mut args = vec!["restore", "--worktree", "--"];
        args.extend(paths.iter().map(String::as_str));
        run(dir, &args)?;
    }
    for path in untracked {
        let full = super::inside(dir, path)?;
        if full.is_dir() {
            std::fs::remove_dir_all(full).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(full).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn commit(dir: &Path, message: &str, all: bool) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("Write a short message saying what changed.".into());
    }
    if all {
        run(dir, &["add", "--all"])?;
    }
    run(dir, &["commit", "-m", message])?;
    Ok(run(dir, &["rev-parse", "--short", "HEAD"])?
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        run(root, &["init", "-q", "-b", "main"]).unwrap();
        run(root, &["config", "user.email", "t@example.invalid"]).unwrap();
        run(root, &["config", "user.name", "Test"]).unwrap();
        std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        run(root, &["add", "."]).unwrap();
        run(root, &["commit", "-q", "-m", "init"]).unwrap();
        dir
    }

    #[test]
    fn porcelain_status_is_parsed() {
        let text = "# branch.oid abc\0# branch.head feat\0# branch.upstream origin/feat\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 aaa bbb src/a.rs\0\
2 R. N... 100644 100644 100644 aaa bbb R100 new name.rs\0old.rs\0\
u UU N... 100644 100644 100644 100644 a b c conflict.rs\0\
? notes.md\0";
        let status = parse_status(text);
        assert_eq!(status.branch.as_deref(), Some("feat"));
        assert_eq!((status.ahead, status.behind), (2, 1));
        assert_eq!(status.files.len(), 4);
        assert_eq!(status.files[0].path, "src/a.rs");
        assert_eq!(status.files[0].unstaged, 'M');
        assert_eq!(status.files[1].path, "new name.rs");
        assert_eq!(status.files[1].original.as_deref(), Some("old.rs"));
        assert!(status.files[2].conflicted);
        assert!(status.files[3].untracked);
    }

    #[test]
    fn numstat_handles_renames_and_binaries() {
        let text = "3\t1\tsrc/a.rs\0-\t-\timg.png\x000\t0\t\0old.rs\0new.rs\0";
        let counts = parse_numstat(text);
        assert_eq!(counts.get("src/a.rs"), Some(&(3, 1)));
        assert!(!counts.contains_key("img.png"));
        assert_eq!(counts.get("new.rs"), Some(&(0, 0)));
    }

    #[test]
    fn worktree_porcelain_is_parsed() {
        let text = "worktree /r\nHEAD 0123456789abcdef\nbranch refs/heads/main\n\nworktree /r.worktrees/x\nHEAD fedcba\nbranch refs/heads/x\nlocked\n\n";
        let trees = parse_worktrees(text);
        assert_eq!(trees.len(), 2);
        assert!(trees[0].main);
        assert_eq!(trees[1].branch.as_deref(), Some("x"));
        assert!(trees[1].locked);
    }

    #[test]
    fn a_real_repository_round_trips() {
        let dir = repo();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "one\nTWO\nthree\n").unwrap();
        std::fs::write(root.join("b.txt"), "new\n").unwrap();
        let status = status(&root).unwrap();
        assert_eq!(status.branch.as_deref(), Some("main"));
        let a = status.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!((a.added, a.removed), (Some(2), Some(1)));
        let b = status.files.iter().find(|f| f.path == "b.txt").unwrap();
        assert!(b.untracked);
        assert_eq!(b.added, Some(1));
        assert_eq!(original(&root, "a.txt", false), "one\ntwo\n");
        assert_eq!(original(&root, "b.txt", false), "");

        stage(&root, &["a.txt".into()]).unwrap();
        assert_eq!(super::status(&root).unwrap().files[0].staged, 'M');
        unstage(&root, &["a.txt".into()]).unwrap();
        discard(&root, &["a.txt".into()], &["b.txt".into()]).unwrap();
        assert!(super::status(&root).unwrap().files.is_empty());
        assert!(!root.join("b.txt").exists());
    }

    #[test]
    fn worktrees_are_created_beside_the_project_and_removed() {
        let dir = repo();
        let root = dir.path().canonicalize().unwrap();
        let tree = add_worktree(&root, "feature/x", "main").unwrap();
        assert!(tree.ends_with("feature-x"));
        assert!(tree.join("a.txt").exists());
        assert_eq!(main_root(&tree).unwrap(), root);
        let listed = worktrees(&root).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[1].branch.as_deref(), Some("feature/x"));
        remove_worktree(&root, &tree.to_string_lossy(), false).unwrap();
        assert_eq!(worktrees(&root).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(tree.parent().unwrap());
    }

    #[test]
    fn verbatim_windows_paths_are_made_plain_for_git() {
        assert_eq!(plain(Path::new(r"\\?\C:\a\b")), PathBuf::from(r"C:\a\b"));
        assert_eq!(
            plain(Path::new(r"\\?\UNC\server\share\x")),
            PathBuf::from(r"\\server\share\x")
        );
        assert_eq!(plain(Path::new("/tmp/a")), PathBuf::from("/tmp/a"));
    }

    #[test]
    fn unsafe_branch_names_are_refused() {
        let dir = repo();
        for name in ["-x", "a..b", "a b", ""] {
            assert!(add_worktree(dir.path(), name, "main").is_err(), "{name}");
        }
    }

    #[test]
    fn commit_needs_a_message() {
        let dir = repo();
        std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
        assert!(commit(dir.path(), "  ", true).is_err());
        assert!(!commit(dir.path(), "Change a", true).unwrap().is_empty());
    }
}
