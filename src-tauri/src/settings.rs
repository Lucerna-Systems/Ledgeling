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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PetSetting {
    pub pack_id: String,
    /// Last known position, restored on launch as a starting point. Absent
    /// until the pet has actually stood somewhere.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// The folder this pet works in: where its terminal opens and what its
    /// actions run on. Chosen from its bubble, kept across launches.
    pub folder: Option<String>,
    /// The name the user gave this pet; the pack's name when absent.
    pub name: Option<String>,
    /// ISO dates (`YYYY-MM-DD`): when they met and when it was last on screen.
    pub met: Option<String>,
    pub last_seen: Option<String>,
    /// Days they have spent together.
    pub days: u32,
    /// How fond of the user it is, 0 to 1.
    pub affection: f64,
}
impl Default for PetSetting {
    fn default() -> Self {
        Self {
            pack_id: String::new(),
            x: None,
            y: None,
            folder: None,
            name: None,
            met: None,
            last_seen: None,
            days: 1,
            affection: 0.2,
        }
    }
}

/// The longest name a pet can be given.
pub const MAX_PET_NAME: usize = 24;

/// A trimmed, length-limited name, or `None` when nothing is left of it.
pub fn clean_name(name: &str) -> Option<String> {
    let trimmed: String = name.trim().chars().take(MAX_PET_NAME).collect();
    let trimmed = trimmed.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn clean_date(date: &Option<String>) -> Option<String> {
    date.as_deref()
        .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
        .map(str::to_string)
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
    /// Set once the first-run welcome has dropped a pet onto the desktop, so
    /// it never repeats. Installs that already have pets count as welcomed.
    pub welcomed: bool,
    /// The "meet your pet" tip card on the Pets page has been dismissed.
    pub tips_dismissed: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            // Empty on purpose: the first-run welcome picks the first pet.
            pets: Vec::new(),
            paused: false,
            hidden: false,
            speed_scale: 1.,
            debug: false,
            developer_mode: false,
            sounds: false,
            launch_at_login: false,
            awareness: AwarenessConfig::default(),
            category_overrides: BTreeMap::new(),
            uninstalled_builtins: Vec::new(),
            pet_screen: None,
            theme: ThemeMode::default(),
            accent: DEFAULT_ACCENT.into(),
            welcomed: false,
            tips_dismissed: false,
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
        for pet in &mut self.pets {
            pet.name = pet.name.as_deref().and_then(clean_name);
            pet.met = clean_date(&pet.met);
            pet.last_seen = clean_date(&pet.last_seen);
            pet.days = pet.days.max(1);
            pet.affection = if pet.affection.is_finite() {
                pet.affection.clamp(0., 1.)
            } else {
                0.2
            };
        }
        if !self.pets.is_empty() {
            self.welcomed = true;
        }
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
        let dir = std::env::temp_dir().join(format!("ledgeling-settings-{}", std::process::id()));
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
        let dir =
            std::env::temp_dir().join(format!("ledgeling-settings-rt-{}", std::process::id()));
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
    fn a_pets_bond_round_trips_and_nonsense_is_fixed() {
        let dir =
            std::env::temp_dir().join(format!("ledgeling-settings-bond-{}", std::process::id()));
        let mut settings = Settings::default();
        settings.pets = vec![PetSetting {
            pack_id: "a".into(),
            name: Some("Biscuit".into()),
            met: Some("2026-10-01".into()),
            last_seen: Some("2026-10-24".into()),
            days: 7,
            affection: 0.6,
            ..Default::default()
        }];
        save(&dir, &settings).unwrap();
        let (loaded, note) = load(&dir);
        assert!(note.is_none());
        assert_eq!(loaded.pets[0].name.as_deref(), Some("Biscuit"));
        assert_eq!(loaded.pets[0].days, 7);
        assert_eq!(loaded.pets[0].affection, 0.6);
        let _ = std::fs::remove_dir_all(&dir);

        let mut messy = Settings::default();
        messy.pets = vec![PetSetting {
            pack_id: "a".into(),
            name: Some("   ".into()),
            met: Some("last tuesday".into()),
            last_seen: Some("2026-13-40".into()),
            days: 0,
            affection: 9.,
            ..Default::default()
        }];
        messy.pets.push(PetSetting {
            pack_id: "b".into(),
            name: Some(format!("  {}  ", "x".repeat(60))),
            affection: f64::NAN,
            ..Default::default()
        });
        messy.sanitize();
        assert_eq!(messy.pets[0].name, None);
        assert_eq!(messy.pets[0].met, None);
        assert_eq!(messy.pets[0].last_seen, None);
        assert_eq!(messy.pets[0].days, 1);
        assert_eq!(messy.pets[0].affection, 1.);
        assert_eq!(messy.pets[1].name.as_ref().map(|n| n.len()), Some(24));
        assert_eq!(messy.pets[1].affection, 0.2);
    }
    #[test]
    fn an_old_pet_entry_loads_with_default_bond() {
        let pet: PetSetting = serde_json::from_str(r#"{"packId":"a","x":1.0}"#).unwrap();
        assert_eq!((pet.days, pet.affection, pet.name), (1, 0.2, None));
    }
    #[test]
    fn theme_and_accent_round_trip() {
        let dir =
            std::env::temp_dir().join(format!("ledgeling-settings-theme-{}", std::process::id()));
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
        let dir =
            std::env::temp_dir().join(format!("ledgeling-settings-mig-{}", std::process::id()));
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
    fn an_old_file_with_pets_counts_as_welcomed() {
        let dir =
            std::env::temp_dir().join(format!("ledgeling-settings-old-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join(SETTINGS_FILE),
            r#"{"version":1,"pets":[{"packId":"com.perch.budgie-classic"}]}"#,
        )
        .unwrap();
        let (loaded, note) = load(&dir);
        assert!(note.is_none());
        assert!(loaded.welcomed);
        assert!(!loaded.tips_dismissed);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn a_fresh_install_is_not_welcomed_and_has_no_pets() {
        let settings = Settings::default();
        assert!(!settings.welcomed);
        assert!(settings.pets.is_empty());
    }
}
