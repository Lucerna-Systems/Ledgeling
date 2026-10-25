//! What the user has allowed. Everything starts `Off`; only a scope the user
//! chose to remember is written to disk, and nothing else ever is.
use super::scope::Scope;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const CONSENT_FILE: &str = "net-consent.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Off,
    /// Until Ledgeling quits.
    Session,
    /// Until the user switches it off, across launches.
    Remembered,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    remembered: Vec<String>,
}

#[derive(Debug)]
pub struct Consent {
    modes: BTreeMap<Scope, Mode>,
    path: Option<PathBuf>,
}

impl Consent {
    /// In memory only: nothing is read or written. Every scope is off.
    pub fn ephemeral() -> Consent {
        Consent {
            modes: BTreeMap::new(),
            path: None,
        }
    }

    /// Reads the remembered scopes from `directory`. A missing or damaged file
    /// means everything is off, never that something is on.
    pub fn load(directory: &Path) -> Consent {
        let path = directory.join(CONSENT_FILE);
        let saved: Saved = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let modes = saved
            .remembered
            .iter()
            .filter_map(|name| Scope::parse(name))
            .map(|scope| (scope, Mode::Remembered))
            .collect();
        Consent {
            modes,
            path: Some(path),
        }
    }

    pub fn mode(&self, scope: Scope) -> Mode {
        self.modes.get(&scope).copied().unwrap_or_default()
    }

    pub fn allows(&self, scope: Scope) -> bool {
        self.mode(scope) != Mode::Off
    }

    pub fn set(&mut self, scope: Scope, mode: Mode) -> Result<(), String> {
        if mode == Mode::Session && scope.always_remembered() {
            return Err(format!(
                "{} can only be switched on until you switch it off, not for one session.",
                scope.label()
            ));
        }
        if mode == Mode::Off {
            self.modes.remove(&scope);
        } else {
            self.modes.insert(scope, mode);
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let saved = Saved {
            remembered: Scope::ALL
                .into_iter()
                .filter(|s| self.mode(*s) == Mode::Remembered)
                .map(|s| s.as_str().to_string())
                .collect(),
        };
        let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("Could not save your choice: {e}"))
    }
}
