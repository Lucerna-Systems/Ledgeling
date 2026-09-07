//! Process name → category. The executable name is read, mapped, and dropped.
//! Window titles are never requested from the OS at any point.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppCategory {
    Code,
    Browser,
    Comms,
    Media,
    Creative,
    Game,
    #[default]
    Other,
}
impl AppCategory {
    pub const ALL: [AppCategory; 7] = [
        AppCategory::Code,
        AppCategory::Browser,
        AppCategory::Comms,
        AppCategory::Media,
        AppCategory::Creative,
        AppCategory::Game,
        AppCategory::Other,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            AppCategory::Code => "code",
            AppCategory::Browser => "browser",
            AppCategory::Comms => "comms",
            AppCategory::Media => "media",
            AppCategory::Creative => "creative",
            AppCategory::Game => "game",
            AppCategory::Other => "other",
        }
    }
    pub fn parse(name: &str) -> Option<AppCategory> {
        AppCategory::ALL.into_iter().find(|c| c.as_str() == name)
    }
}

/// Editable mapping of process-name patterns to categories. A pattern matches
/// if the process name equals it, or starts with it when it ends in `*`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryMap {
    pub patterns: BTreeMap<String, AppCategory>,
}
impl Default for CategoryMap {
    fn default() -> Self {
        use AppCategory::*;
        let defaults: &[(&str, AppCategory)] = &[
            ("devenv", Code),
            ("code", Code),
            ("code - insiders", Code),
            ("cursor", Code),
            ("idea*", Code),
            ("pycharm*", Code),
            ("webstorm*", Code),
            ("rider", Code),
            ("vim", Code),
            ("nvim", Code),
            ("sublime_text", Code),
            ("sublime text", Code),
            ("xcode", Code),
            ("zed", Code),
            ("terminal", Code),
            ("iterm2", Code),
            ("alacritty", Code),
            ("kitty", Code),
            ("ghostty", Code),
            ("wezterm*", Code),
            ("chrome", Browser),
            ("google chrome", Browser),
            ("firefox", Browser),
            ("msedge", Browser),
            ("microsoft edge", Browser),
            ("safari", Browser),
            ("arc", Browser),
            ("brave*", Browser),
            ("vivaldi", Browser),
            ("opera", Browser),
            ("slack", Comms),
            ("teams", Comms),
            ("microsoft teams", Comms),
            ("discord", Comms),
            ("zoom*", Comms),
            ("outlook", Comms),
            ("thunderbird", Comms),
            ("mail", Comms),
            ("messages", Comms),
            ("signal", Comms),
            ("whatsapp", Comms),
            ("telegram", Comms),
            ("photoshop", Creative),
            ("illustrator", Creative),
            ("blender", Creative),
            ("figma", Creative),
            ("aseprite", Creative),
            ("ableton*", Creative),
            ("davinci resolve", Creative),
            ("logic pro", Creative),
            ("affinity*", Creative),
            ("sketch", Creative),
            ("inkscape", Creative),
            ("gimp*", Creative),
            ("vlc", Media),
            ("spotify", Media),
            ("mpv", Media),
            ("music", Media),
            ("quicktime player", Media),
            ("iina", Media),
            ("plex", Media),
            ("tv", Media),
            ("steam", Game),
            ("steamwebhelper", Game),
            ("epicgameslauncher", Game),
            ("minecraft*", Game),
            ("factorio", Game),
        ];
        Self {
            patterns: defaults
                .iter()
                .map(|(k, v)| ((*k).to_string(), *v))
                .collect(),
        }
    }
}
impl CategoryMap {
    pub fn categorize(&self, process: &str) -> AppCategory {
        let process = process.to_lowercase();
        let process = process.trim_end_matches(".exe");
        let mut best: Option<(usize, AppCategory)> = None;
        for (pattern, category) in &self.patterns {
            let hit = match pattern.strip_suffix('*') {
                Some(prefix) => process.starts_with(prefix),
                None => process == pattern,
            };
            // Longest matching pattern wins, so "code - insiders" beats "code".
            if hit && best.is_none_or(|(len, _)| pattern.len() > len) {
                best = Some((pattern.len(), *category));
            }
        }
        best.map(|(_, c)| c).unwrap_or(AppCategory::Other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_process_names_and_falls_back_to_other() {
        let map = CategoryMap::default();
        assert_eq!(map.categorize("Code.exe"), AppCategory::Code);
        assert_eq!(map.categorize("idea64"), AppCategory::Code);
        assert_eq!(map.categorize("Slack"), AppCategory::Comms);
        assert_eq!(map.categorize("something-unheard-of"), AppCategory::Other);
    }
    #[test]
    fn longest_pattern_wins() {
        let map = CategoryMap::default();
        assert_eq!(map.categorize("code - insiders"), AppCategory::Code);
    }
}
