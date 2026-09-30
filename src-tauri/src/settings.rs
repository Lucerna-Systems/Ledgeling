//! Preferences and session state, persisted as JSON. A corrupt file is moved
//! aside and replaced with defaults rather than being allowed to stop launch.
use crate::awareness::{AppCategory, AwarenessConfig, CategoryMap};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub const SETTINGS_FILE: &str = "settings.json";
pub const DEFAULT_ACCENT: &str = "#5b5bd6";
/// The pre-redesign default, which every existing install has saved. It no
/// longer matches a preset, so it is treated as "never chosen" and replaced.
const RETIRED_DEFAULT_ACCENT: &str = "#31725b";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PetSetting {
    pub pack_id: String,
    /// Last known position, restored on launch as a starting point. Absent
    /// until the pet has actually stood somewhere.
    pub x: Option<f64>,
    pub y: Option<f64>,
}
/// How the companion UI is coloured. `System` follows the OS appearance,
/// resolved per-window on the frontend and via `set_theme(None)` natively.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
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
    /// The hardware name of the one screen pets are allowed on. `None` means
    /// the main display. A name that is not currently connected falls back to
    /// the main display until it returns, so unplugging and replugging the
    /// "workstation" screen round-trips.
    pub pet_screen: Option<String>,
    pub theme: ThemeMode,
    /// Accent colour as `#rrggbb`. Rust only validates the shape; the frontend
    /// derives hover/soft variants so hand-tuned presets stay in one place.
    pub accent: String,
    /// Developer tools: coding mode and its tray entry. Off for everyone who
    /// did not ask, so none of it loads.
    pub developer_mode: bool,
    /// The first-run tour has been finished or skipped. A file saved before the
    /// tour existed reads as done, so only a genuinely new install sees it.
    #[serde(default = "tour_seen")]
    pub tour_done: bool,
}
fn tour_seen() -> bool {
    true
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
            developer_mode: false,
            tour_done: false,
            sounds: false,
            launch_at_login: false,
            awareness: AwarenessConfig::default(),
            category_overrides: BTreeMap::new(),
            uninstalled_builtins: Vec::new(),
            pet_screen: None,
            theme: ThemeMode::default(),
            accent: DEFAULT_ACCENT.into(),
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
        // A setting saved by a build with coding mode must not offer it in one without.
        self.developer_mode &= cfg!(feature = "coding");
        self.pets.retain(|p| !p.pack_id.is_empty());
        self.pets.truncate(8);
        self.accent = self.accent.to_lowercase();
        if !is_hex_color(&self.accent) || self.accent == RETIRED_DEFAULT_ACCENT {
            self.accent = DEFAULT_ACCENT.into();
        }
    }
}

/// `#` followed by exactly six ASCII hex digits.
fn is_hex_color(value: &str) -> bool {
    let hex = match value.strip_prefix('#') {
        Some(hex) => hex,
        None => return false,
    };
    hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit())
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
                    "Your settings file could not be read ({error}), so Ledgeling started with \
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
    #[test]
    fn theme_and_accent_round_trip() {
        let dir = std::env::temp_dir().join(format!("perch-settings-theme-{}", std::process::id()));
        let settings = Settings {
            theme: ThemeMode::Dark,
            accent: "#3A7BD5".into(),
            ..Default::default()
        };
        save(&dir, &settings).unwrap();
        let text = std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
        assert!(text.contains(r#""theme": "dark""#));
        let (loaded, _) = load(&dir);
        assert_eq!(loaded.theme, ThemeMode::Dark);
        assert_eq!(loaded.accent, "#3a7bd5");
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn bad_accent_falls_back_to_default() {
        let mut settings = Settings {
            theme: ThemeMode::Light,
            ..Default::default()
        };
        for bad in ["blue", "#1234", "#12345g", "31725b", "#31725B"] {
            settings.accent = bad.into();
            settings.sanitize();
            assert_eq!(settings.accent, DEFAULT_ACCENT, "should reject {bad}");
        }
        assert_eq!(settings.theme, ThemeMode::Light);
    }
    #[test]
    fn missing_theme_and_accent_read_as_defaults() {
        // A settings.json written before the feature existed has neither key.
        let dir = std::env::temp_dir().join(format!("perch-settings-mig-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join(SETTINGS_FILE),
            r#"{"version":1,"pets":[{"packId":"com.perch.budgie-classic"}],"paused":false,
               "hidden":false,"speedScale":1,"debug":false,"sounds":false,"launchAtLogin":false,
               "awareness":{},"categoryOverrides":{},"uninstalledBuiltins":[],"petScreen":null}"#,
        )
        .unwrap();
        let (settings, note) = load(&dir);
        assert!(note.is_none());
        assert_eq!(settings.theme, ThemeMode::System);
        assert_eq!(settings.accent, DEFAULT_ACCENT);
        assert!(
            !settings.developer_mode,
            "coding mode stays off for existing users"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn only_a_new_install_gets_the_tour() {
        assert!(!Settings::default().tour_done);
        let dir = std::env::temp_dir().join(format!("perch-settings-tour-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(SETTINGS_FILE), r#"{"version":1,"pets":[]}"#).unwrap();
        assert!(
            load(&dir).0.tour_done,
            "an existing install does not get it"
        );
        let mut settings = Settings::default();
        settings.tour_done = true;
        save(&dir, &settings).unwrap();
        assert!(load(&dir).0.tour_done);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
