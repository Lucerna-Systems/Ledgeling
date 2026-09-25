//! The folder handoff: a pet dragged onto a folder leaves a uniquely named,
//! hidden file there, and that file's parent directory is the folder. The path
//! comes from the drop itself, never from inspecting the window it landed in.
//! The drag is native per platform; what is decided here is shared and tested.
use std::path::{Path, PathBuf};

pub const PREFIX: &str = ".perch-handoff-";
/// The handoff file's entire contents. Removal checks for exactly this, so a
/// file that is not the one Perch offered is never deleted.
pub const MARKER: &[u8] = b"Perch folder handoff\n";

pub fn file_name(unique: &str) -> String {
    format!("{PREFIX}{unique}")
}

/// The folder the handoff file `name` was written into, from the paths a
/// shell change notification names. A notification names either the new file
/// itself or only the folder whose contents changed; either way the file has
/// to actually be there.
pub fn landed_in(
    name: &str,
    paths: &[PathBuf],
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if !name.starts_with(PREFIX) {
        return None;
    }
    paths
        .iter()
        .find(|p| p.file_name().is_some_and(|n| n == name) && is_file(p))
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .or_else(|| paths.iter().find(|p| is_file(&p.join(name))).cloned())
}

/// Is `path` still exactly the handoff file Perch offered?
pub fn is_ours(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(PREFIX) && n.len() > PREFIX.len())
        && std::fs::symlink_metadata(path)
            .is_ok_and(|m| m.is_file() && m.len() == MARKER.len() as u64)
        && std::fs::read(path).is_ok_and(|bytes| bytes == MARKER)
}

/// Remove the handoff file once the drop has finished with it. The file
/// manager may still be holding it open, so a few attempts are made a second
/// apart; anything that is not exactly the handoff file is left alone.
pub fn remove_later(path: PathBuf) {
    std::thread::spawn(move || {
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if !is_ours(&path) || std::fs::remove_file(&path).is_ok() {
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = ".perch-handoff-3F2504E0-4F89-11D3-9A0C-0305E82C3301";

    #[test]
    fn a_notification_naming_the_file_gives_its_folder() {
        let file = PathBuf::from("/home/me/Clients").join(NAME);
        let found = landed_in(NAME, std::slice::from_ref(&file), |p| p == file);
        assert_eq!(found, Some(PathBuf::from("/home/me/Clients")));
    }

    #[test]
    fn a_notification_naming_only_the_folder_checks_the_file_is_there() {
        let folder = PathBuf::from("/home/me/Clients");
        let file = folder.join(NAME);
        assert_eq!(
            landed_in(NAME, std::slice::from_ref(&folder), |p| p == file),
            Some(folder.clone())
        );
        assert_eq!(landed_in(NAME, &[folder], |_| false), None);
    }

    #[test]
    fn unrelated_changes_bind_nothing() {
        let other = PathBuf::from("/home/me/Clients/.perch-handoff-somebody-else");
        assert_eq!(
            landed_in(NAME, std::slice::from_ref(&other), |p| p == other),
            None
        );
        let listed = PathBuf::from("/home/me/Clients").join(NAME);
        // A path with the right name that is not a file is not the handoff.
        assert_eq!(landed_in(NAME, &[listed], |_| false), None);
        assert_eq!(
            landed_in("report.pdf", &[PathBuf::from("/x/report.pdf")], |_| true),
            None
        );
    }

    #[test]
    fn only_the_exact_handoff_file_counts_as_ours() {
        let dir = tempfile::tempdir().unwrap();
        let ours = dir.path().join(NAME);
        std::fs::write(&ours, MARKER).unwrap();
        assert!(is_ours(&ours));

        let edited = dir.path().join(file_name("edited"));
        std::fs::write(&edited, b"Perch folder handoff\nplus notes\n").unwrap();
        assert!(!is_ours(&edited));

        let renamed = dir.path().join("notes.txt");
        std::fs::write(&renamed, MARKER).unwrap();
        assert!(!is_ours(&renamed));

        let folder = dir.path().join(file_name("folder"));
        std::fs::create_dir(&folder).unwrap();
        assert!(!is_ours(&folder));
        assert!(!is_ours(&dir.path().join(file_name("missing"))));
    }

    #[test]
    fn removal_leaves_anything_else_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let ours = dir.path().join(NAME);
        std::fs::write(&ours, MARKER).unwrap();
        let theirs = dir.path().join(file_name("theirs"));
        std::fs::write(&theirs, b"a user's file").unwrap();
        remove_later(ours.clone());
        remove_later(theirs.clone());
        std::thread::sleep(std::time::Duration::from_millis(1600));
        assert!(!ours.exists());
        assert!(theirs.exists());
    }
}
