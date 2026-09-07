//! Preferences and session state, persisted as JSON. A corrupt file is moved
//! aside and replaced with defaults rather than being allowed to stop launch.
use crate::awareness::{AppCategory, AwarenessConfig, CategoryMap};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub const SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PetSetting {
    pub pack_id: String,
    /// Last known position, restored on launch as a starting point. Absent
    /// until the pet has actually stood somewhere.
    pub x: Option<f64>,
    pub y: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub pets: Vec<PetSetting>,
    pub paused: bool,
    pub hidden: bool,
    /// Multiplies every pack's speeds, so "unhurried" is one slider.
    pub speed_scale: f64,
    pub debug: bool,
    pub sounds: bool,
    pub launch_at_login: bool,
    pub awareness: AwarenessConfig,
    /// User additions to the process-name → category table.
    pub category_overrides: BTreeMap<String, AppCategory>,
    pub uninstalled_builtins: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            pets: vec![PetSetting {
                pack_id: "com.perch.budgie-classic".into(),
                ..Default::default()
            }],
            paused: false,
            hidden: false,
            speed_scale: 1.,
            debug: false,
            sounds: false,
            launch_at_login: false,
            awareness: AwarenessConfig::default(),
            category_overrides: BTreeMap::new(),
            uninstalled_builtins: Vec::new(),
        }
    }
}

impl Settings {
    pub fn category_map(&self) -> CategoryMap {
        let mut map = CategoryMap::default();
        for (pattern, category) in &self.category_overrides {
            map.patterns.insert(pattern.to_lowercase(), *category);
        }
        map
    }
    pub fn sanitize(&mut self) {
        if !self.speed_scale.is_finite() || !(0.25..=3.).contains(&self.speed_scale) {
            self.speed_scale = 1.;
        }
        self.pets.retain(|p| !p.pack_id.is_empty());
        self.pets.truncate(8);
    }
}

/// Load settings, returning a human-readable note when the file had to be
/// recovered so the companion window can say what happened.
pub fn load(directory: &Path) -> (Settings, Option<String>) {
    let path = directory.join(SETTINGS_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (Settings::default(), None);
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(mut settings) => {
            settings.sanitize();
            (settings, None)
        }
        Err(error) => {
            let backup = directory.join(format!("{SETTINGS_FILE}.broken"));
            let _ = std::fs::rename(&path, &backup);
            (
                Settings::default(),
                Some(format!(
                    "Your settings file could not be read ({error}), so Perch started with \
defaults. The old file is kept at {}.",
                    backup.display()
                )),
            )
        }
    }
}

pub fn save(directory: &Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("Could not create the settings folder: {e}"))?;
    let text = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Could not encode the settings: {e}"))?;
    let temporary = directory.join(format!("{SETTINGS_FILE}.tmp"));
    std::fs::write(&temporary, text).map_err(|e| format!("Could not write settings: {e}"))?;
    // Replace atomically so an interrupted write cannot corrupt the real file.
    std::fs::rename(&temporary, directory.join(SETTINGS_FILE))
        .map_err(|e| format!("Could not save settings: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_corrupt_file_recovers_to_defaults_and_is_kept() {
        let dir = std::env::temp_dir().join(format!("perch-settings-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(SETTINGS_FILE), "{ not json").unwrap();
        let (settings, note) = load(&dir);
        assert_eq!(settings.speed_scale, 1.);
        assert!(note.is_some_and(|n| n.contains("defaults")));
        assert!(dir.join("settings.json.broken").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn round_trips_and_clamps_nonsense() {
        let dir = std::env::temp_dir().join(format!("perch-settings-rt-{}", std::process::id()));
        let mut settings = Settings {
            speed_scale: 99.,
            ..Default::default()
        };
        settings.sanitize();
        save(&dir, &settings).unwrap();
        let (loaded, note) = load(&dir);
        assert!(note.is_none());
        assert_eq!(loaded.speed_scale, 1.);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
