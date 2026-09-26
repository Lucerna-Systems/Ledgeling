//! One-time carry-over from the app's pre-rename identifier, so an upgraded
//! install keeps its settings, chats, pets and saved keys.
use std::path::Path;

/// The bundle identifier before the app was renamed to Ledgeling.
pub const LEGACY_IDENTIFIER: &str = "com.perch.desktop";

/// Moves the legacy app folder (a sibling of `current`) into place. Never
/// overwrites: a `current` folder with anything in it wins and the legacy one
/// is left for the user. Returns whether a move happened.
pub fn move_app_dir(current: &Path) -> Result<bool, String> {
    let Some(parent) = current.parent() else {
        return Ok(false);
    };
    let legacy = parent.join(LEGACY_IDENTIFIER);
    if !legacy.is_dir() {
        return Ok(false);
    }
    if current.exists() {
        let empty = std::fs::read_dir(current)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !empty {
            return Ok(false);
        }
        std::fs::remove_dir(current)
            .map_err(|e| format!("Could not replace {}: {e}", current.display()))?;
    }
    std::fs::rename(&legacy, current).map_err(|e| {
        format!(
            "Your earlier settings are still in {} and could not be moved ({e}).",
            legacy.display()
        )
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_with_settings(root: &Path) {
        let legacy = root.join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(legacy.join("assistant")).unwrap();
        std::fs::write(legacy.join("settings.json"), "{}").unwrap();
    }

    #[test]
    fn moves_the_legacy_folder_into_an_absent_or_empty_one() {
        for pre_created in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let current = root.path().join("technology.lucerna-systems.ledgeling");
            legacy_with_settings(root.path());
            if pre_created {
                std::fs::create_dir(&current).unwrap();
            }
            assert!(move_app_dir(&current).unwrap());
            assert!(current.join("settings.json").exists());
            assert!(current.join("assistant").is_dir());
            assert!(!root.path().join(LEGACY_IDENTIFIER).exists());
            assert!(
                !move_app_dir(&current).unwrap(),
                "a second launch finds nothing to move"
            );
        }
    }

    #[test]
    fn never_overwrites_data_already_in_the_new_folder() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("technology.lucerna-systems.ledgeling");
        legacy_with_settings(root.path());
        std::fs::create_dir(&current).unwrap();
        std::fs::write(current.join("settings.json"), "{\"new\":true}").unwrap();
        assert!(!move_app_dir(&current).unwrap());
        assert_eq!(
            std::fs::read_to_string(current.join("settings.json")).unwrap(),
            "{\"new\":true}"
        );
        assert!(root
            .path()
            .join(LEGACY_IDENTIFIER)
            .join("settings.json")
            .exists());
    }

    #[test]
    fn does_nothing_without_a_legacy_folder() {
        let root = tempfile::tempdir().unwrap();
        assert!(!move_app_dir(&root.path().join("technology.lucerna-systems.ledgeling")).unwrap());
    }
}
