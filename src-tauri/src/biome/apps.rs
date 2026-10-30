//! Which window belongs to which biome. A biome is keyed by an app's
//! executable name, so a window's owning process has to be named, and that
//! name is exactly what the privacy rules say must not be kept. So it is not:
//! each pid is named once, compared with the biomes the user made, and only
//! the answer ("biome 3", or "none") is remembered.
use super::BiomeSetting;
use crate::{platform::Platform, world::WindowInfo};
use std::collections::HashMap;

/// The app of a window the user clicked in Decorate mode, to make or open its
/// biome. Asked only because the user chose this window.
pub fn app_of(window: &WindowInfo, platform: &dyn Platform) -> Option<String> {
    window.pid.and_then(|pid| platform.process_name(pid))
}

/// New pids named per scan at most; the rest wait for the next scan.
const LOOKUPS_PER_SCAN: usize = 8;

#[derive(Default)]
pub struct AppResolver {
    /// The biome each process belongs to, if any. Never a name.
    cache: HashMap<u32, Option<u64>>,
    /// The biomes' apps as the user saved them, to notice when they change.
    apps: Vec<(String, u64)>,
}

impl AppResolver {
    /// Forget every answer when the biomes change: a "none" may now match.
    pub fn set_biomes(&mut self, biomes: &[BiomeSetting]) {
        let apps: Vec<(String, u64)> = biomes.iter().map(|b| (b.app.clone(), b.id)).collect();
        if apps != self.apps {
            self.apps = apps;
            self.cache.clear();
        }
    }

    /// The biome each window belongs to, by window id.
    pub fn resolve(
        &mut self,
        windows: &[WindowInfo],
        platform: &dyn Platform,
    ) -> HashMap<u64, u64> {
        if self.apps.is_empty() {
            self.cache.clear();
            return HashMap::new();
        }
        let live: Vec<u32> = windows.iter().filter_map(|w| w.pid).collect();
        self.cache.retain(|pid, _| live.contains(pid));
        let mut budget = LOOKUPS_PER_SCAN;
        let mut owners = HashMap::new();
        for window in windows {
            let Some(pid) = window.pid else {
                continue;
            };
            if !self.cache.contains_key(&pid) {
                if budget == 0 {
                    continue;
                }
                budget -= 1;
                // The name lives only for this comparison.
                let biome = platform.process_name(pid).and_then(|name| {
                    self.apps
                        .iter()
                        .find(|(app, _)| *app == name)
                        .map(|(_, id)| *id)
                });
                self.cache.insert(pid, biome);
            }
            if let Some(Some(biome)) = self.cache.get(&pid) {
                owners.insert(window.id, *biome);
            }
        }
        owners
    }

    #[cfg(test)]
    fn cached(&self) -> &HashMap<u32, Option<u64>> {
        &self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{platform::mock::MockPlatform, world::Rect};

    fn biome(id: u64, app: &str) -> BiomeSetting {
        BiomeSetting {
            id,
            app: app.into(),
            props: Vec::new(),
        }
    }

    fn platform() -> MockPlatform {
        let platform = MockPlatform::default();
        let mut names = platform.names.lock().unwrap();
        names.insert(7, "safari".into());
        names.insert(9, "slack".into());
        drop(names);
        platform
    }

    fn window(id: u64, pid: u32) -> WindowInfo {
        WindowInfo::new(id, Rect::new(0., 0., 400., 300.)).with_pid(pid)
    }

    #[test]
    fn only_the_answer_is_kept_never_the_name() {
        let platform = platform();
        let mut resolver = AppResolver::default();
        resolver.set_biomes(&[biome(3, "safari")]);
        let owners = resolver.resolve(&[window(1, 7), window(2, 9)], &platform);
        assert_eq!(owners, HashMap::from([(1, 3)]));
        assert_eq!(
            resolver.cached(),
            &HashMap::from([(7, Some(3)), (9, None)]),
            "the cache holds biome ids, not process names"
        );
    }

    #[test]
    fn each_process_is_named_once_until_the_biomes_change() {
        let platform = platform();
        let mut resolver = AppResolver::default();
        resolver.set_biomes(&[biome(3, "safari")]);
        let windows = [window(1, 7), window(2, 9), window(3, 7)];
        resolver.resolve(&windows, &platform);
        resolver.resolve(&windows, &platform);
        assert_eq!(*platform.name_lookups.lock().unwrap(), 2);
        resolver.set_biomes(&[biome(3, "safari"), biome(4, "slack")]);
        let owners = resolver.resolve(&windows, &platform);
        assert_eq!(owners.get(&2), Some(&4), "a new biome matches at once");
    }

    #[test]
    fn with_no_biomes_nothing_is_named_at_all() {
        let platform = platform();
        let mut resolver = AppResolver::default();
        resolver.resolve(&[window(1, 7)], &platform);
        assert_eq!(*platform.name_lookups.lock().unwrap(), 0);
    }
}
