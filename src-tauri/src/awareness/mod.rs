//! The awareness layer: coarse signals about the session, and the restrained
//! nudge ladder built on top of them. Everything here is optional — turn it off
//! and the pet still lives on your windows, it just stops noticing your day.
pub mod categories;
pub mod nudge;
pub mod session;

pub use categories::{AppCategory, CategoryMap};
pub use nudge::{NudgeConfig, NudgeLadder};
pub use session::{IdleState, Session, SessionEvent, SessionSummary, Thresholds};

use crate::{
    packs::{manifest::Event, Signals},
    platform::Platform,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AwarenessConfig {
    /// The one master switch: off means no idle polling, no foreground query,
    /// no session tracking and no nudges.
    pub enabled: bool,
    pub thresholds: Thresholds,
    pub nudge: NudgeConfig,
    /// Seconds a foreground category must persist before it counts.
    pub category_debounce: f64,
}
impl Default for AwarenessConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds: Thresholds::default(),
            nudge: NudgeConfig::default(),
            category_debounce: 5.,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwarenessSummary {
    pub enabled: bool,
    pub session: SessionSummary,
    pub category: String,
    pub nudge_tier: u8,
    pub energy: f64,
    pub mood: f64,
}

pub struct Awareness {
    pub config: AwarenessConfig,
    pub map: CategoryMap,
    pub session: Session,
    pub nudge: NudgeLadder,
    pub category: AppCategory,
    pending: Option<(AppCategory, f64)>,
    hour: u32,
    poll_accumulator: f64,
    category_accumulator: f64,
    started: bool,
}

impl Default for Awareness {
    fn default() -> Self {
        Self::new(AwarenessConfig::default(), CategoryMap::default())
    }
}

impl Awareness {
    pub fn new(config: AwarenessConfig, map: CategoryMap) -> Self {
        Self {
            session: Session::new(config.thresholds),
            nudge: NudgeLadder::new(config.nudge),
            config,
            map,
            category: AppCategory::Other,
            pending: None,
            hour: local_hour(),
            poll_accumulator: 0.,
            category_accumulator: 0.,
            started: false,
        }
    }

    pub fn apply(&mut self, config: AwarenessConfig, map: CategoryMap) {
        let was_enabled = self.config.enabled;
        self.session.thresholds = config.thresholds;
        self.nudge.config = config.nudge;
        self.config = config;
        self.map = map;
        if !was_enabled && self.config.enabled {
            self.session.reset();
        }
        if !self.config.enabled {
            self.category = AppCategory::Other;
            self.session.reset();
            self.nudge.reset();
        }
    }

    /// Poll the OS at the documented rates and turn what comes back into events.
    /// Idle time is read at 1 Hz and the foreground process at 2 Hz; the
    /// executable name is mapped to a category and dropped immediately.
    pub fn tick(&mut self, dt: f64, platform: &dyn Platform) -> Vec<Event> {
        let mut events = Vec::new();
        if !self.started {
            self.started = true;
            events.push(Event::Startup);
        }
        if !self.config.enabled {
            return events;
        }
        self.poll_accumulator += dt;
        self.category_accumulator += dt;
        if self.category_accumulator >= 0.5 {
            let elapsed = std::mem::take(&mut self.category_accumulator);
            self.poll_category(elapsed, platform, &mut events);
        }
        if self.poll_accumulator >= 1. {
            let elapsed = std::mem::take(&mut self.poll_accumulator);
            let idle = platform.idle_seconds().unwrap_or(0.);
            for event in self.session.update(elapsed, idle) {
                match event {
                    SessionEvent::BreakTaken => events.push(Event::BreakTaken),
                    SessionEvent::Returned => events.push(Event::UserReturned),
                    SessionEvent::IdleStateChanged(state) => {
                        if state == IdleState::Away || state == IdleState::LongAway {
                            events.push(Event::UserIdleLong);
                        }
                    }
                }
            }
            let hour = local_hour();
            if hour != self.hour {
                self.hour = hour;
                events.push(Event::HourChanged);
            }
            if self
                .nudge
                .tick(elapsed, self.session.stretch_minutes(), self.category)
            {
                events.push(Event::LongSession);
            }
        }
        events
    }

    fn poll_category(&mut self, dt: f64, platform: &dyn Platform, events: &mut Vec<Event>) {
        let observed = match platform.foreground_process() {
            // The process name is used here and then dropped; it is never
            // stored, logged, or sent anywhere.
            Ok(Some(process)) => self.map.categorize(&process.name),
            _ => return,
        };
        if observed == self.category {
            self.pending = None;
            return;
        }
        let waited = match self.pending {
            Some((pending, waited)) if pending == observed => waited + dt,
            _ => dt,
        };
        // Debounced, so alt-tabbing through three windows changes nothing.
        if waited >= self.config.category_debounce {
            self.category = observed;
            self.pending = None;
            events.push(Event::AppCategoryChanged);
        } else {
            self.pending = Some((observed, waited));
        }
    }

    pub fn signals(&self) -> Signals {
        Signals {
            idle_seconds: if self.config.enabled {
                self.session.idle_seconds
            } else {
                0.
            },
            session_minutes: if self.config.enabled {
                self.session.stretch_minutes()
            } else {
                0.
            },
            app_category: self.category,
            hour: self.hour as f64,
            ..Signals::default()
        }
    }

    pub fn summary(&self, energy: f64, mood: f64) -> AwarenessSummary {
        AwarenessSummary {
            enabled: self.config.enabled,
            session: self.session.summary(),
            category: self.category.as_str().to_string(),
            nudge_tier: self.nudge.tier(),
            energy,
            mood,
        }
    }
}

/// Local hour without pulling in a date library: the offset is derived from the
/// difference between the system's local and UTC representation of "now".
pub fn local_hour() -> u32 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    ((seconds + local_offset_seconds()).rem_euclid(86_400) / 3600) as u32
}

#[cfg(target_family = "unix")]
fn local_offset_seconds() -> i64 {
    extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime(t: *const i64) -> *const Tm;
    }
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: i64,
        zone: *const i8,
    }
    unsafe {
        let now = time(std::ptr::null_mut());
        let tm = localtime(&now);
        if tm.is_null() {
            0
        } else {
            (*tm).gmtoff
        }
    }
}

#[cfg(not(target_family = "unix"))]
fn local_offset_seconds() -> i64 {
    // Windows: GetTimeZoneInformation reports the bias in minutes west of UTC.
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: [u16; 8],
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: [u16; 8],
        daylight_bias: i32,
    }
    extern "system" {
        fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
    }
    unsafe {
        let mut info: TimeZoneInformation = std::mem::zeroed();
        let result = GetTimeZoneInformation(&mut info);
        let extra = match result {
            2 => info.daylight_bias,
            1 => info.standard_bias,
            _ => 0,
        };
        -((info.bias + extra) as i64) * 60
    }
}
