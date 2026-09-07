//! Session rhythm, derived entirely from idle time and held only in memory.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleState {
    #[default]
    Active,
    Pausing,
    Away,
    LongAway,
}
impl IdleState {
    pub fn as_str(self) -> &'static str {
        match self {
            IdleState::Active => "active",
            IdleState::Pausing => "pausing",
            IdleState::Away => "away",
            IdleState::LongAway => "long_away",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thresholds {
    pub pausing_seconds: f64,
    pub away_seconds: f64,
    pub long_away_seconds: f64,
    /// An away period at least this long counts as a real break.
    pub break_seconds: f64,
    /// A pausing period at least this long is noted but does not reset.
    pub micro_break_seconds: f64,
}
impl Default for Thresholds {
    fn default() -> Self {
        Self {
            pausing_seconds: 30.,
            away_seconds: 180.,
            long_away_seconds: 1200.,
            break_seconds: 300.,
            micro_break_seconds: 90.,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub idle_seconds: f64,
    pub idle_state: &'static str,
    /// Minutes since the last real break.
    pub stretch_minutes: f64,
    /// Minutes since launch or since the last long absence.
    pub session_minutes: f64,
    pub micro_breaks: u32,
    pub breaks: u32,
}

/// Tracks the current unbroken stretch. Nothing here reaches the disk: the
/// counters are aggregates and are gone when the app closes.
#[derive(Debug)]
pub struct Session {
    pub thresholds: Thresholds,
    pub state: IdleState,
    pub idle_seconds: f64,
    stretch_seconds: f64,
    session_seconds: f64,
    /// Longest idle time seen since input last resumed: the length of the gap
    /// that is about to end, since the platform only reports the current value.
    peak_idle: f64,
    pub micro_breaks: u32,
    pub breaks: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionEvent {
    IdleStateChanged(IdleState),
    BreakTaken,
    Returned,
}

impl Default for Session {
    fn default() -> Self {
        Self::new(Thresholds::default())
    }
}

impl Session {
    pub fn new(thresholds: Thresholds) -> Self {
        Self {
            thresholds,
            state: IdleState::Active,
            idle_seconds: 0.,
            stretch_seconds: 0.,
            session_seconds: 0.,
            peak_idle: 0.,
            micro_breaks: 0,
            breaks: 0,
        }
    }

    /// `idle_seconds` comes straight from the platform; `dt` is wall time.
    pub fn update(&mut self, dt: f64, idle_seconds: f64) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        // A tick counter that wrapped, or a clock that went backwards, must not
        // register as an enormous absence.
        let idle = if idle_seconds.is_finite() && idle_seconds >= 0. {
            idle_seconds
        } else {
            0.
        };
        let returned = idle < self.idle_seconds - 1.;
        let gap = self.peak_idle.max(self.idle_seconds);
        self.idle_seconds = idle;
        self.peak_idle = if returned {
            0.
        } else {
            self.peak_idle.max(idle)
        };
        self.session_seconds += dt;
        if idle < self.thresholds.pausing_seconds {
            self.stretch_seconds += dt;
        }
        let previous = self.state;
        self.state = if idle >= self.thresholds.long_away_seconds {
            IdleState::LongAway
        } else if idle >= self.thresholds.away_seconds {
            IdleState::Away
        } else if idle >= self.thresholds.pausing_seconds {
            IdleState::Pausing
        } else {
            IdleState::Active
        };
        if self.state != previous {
            events.push(SessionEvent::IdleStateChanged(self.state));
        }
        if returned {
            if gap >= self.thresholds.break_seconds {
                self.stretch_seconds = 0.;
                self.breaks += 1;
                events.push(SessionEvent::BreakTaken);
            } else if gap >= self.thresholds.micro_break_seconds {
                self.micro_breaks += 1;
            }
            if gap >= self.thresholds.away_seconds {
                events.push(SessionEvent::Returned);
            }
            if gap >= self.thresholds.long_away_seconds {
                self.session_seconds = 0.;
            }
        }
        events
    }

    pub fn stretch_minutes(&self) -> f64 {
        self.stretch_seconds / 60.
    }
    pub fn session_minutes(&self) -> f64 {
        self.session_seconds / 60.
    }
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            idle_seconds: self.idle_seconds,
            idle_state: self.state.as_str(),
            stretch_minutes: self.stretch_minutes(),
            session_minutes: self.session_minutes(),
            micro_breaks: self.micro_breaks,
            breaks: self.breaks,
        }
    }
    /// Manual reset, used when the user disables and re-enables awareness.
    pub fn reset(&mut self) {
        let thresholds = self.thresholds;
        *self = Session::new(thresholds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(session: &mut Session, seconds: f64, idle: impl Fn(f64) -> f64) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        let mut t = 0.;
        while t < seconds {
            t += 1.;
            events.extend(session.update(1., idle(t)));
        }
        events
    }
    #[test]
    fn derives_four_idle_states_at_the_documented_thresholds() {
        let mut session = Session::default();
        session.update(1., 5.);
        assert_eq!(session.state, IdleState::Active);
        session.update(1., 45.);
        assert_eq!(session.state, IdleState::Pausing);
        session.update(1., 400.);
        assert_eq!(session.state, IdleState::Away);
        session.update(1., 2000.);
        assert_eq!(session.state, IdleState::LongAway);
    }
    #[test]
    fn a_five_minute_absence_resets_the_stretch_and_a_short_one_does_not() {
        let mut session = Session::default();
        run(&mut session, 600., |_| 0.);
        assert!(session.stretch_minutes() > 9.);
        run(&mut session, 400., |t| t);
        let events = session.update(1., 0.);
        assert!(events.contains(&SessionEvent::BreakTaken));
        assert!(session.stretch_minutes() < 0.1);

        run(&mut session, 600., |_| 0.);
        let before = session.stretch_minutes();
        run(&mut session, 100., |t| t);
        session.update(1., 0.);
        assert!(
            session.stretch_minutes() > before,
            "a micro-break must not reset the stretch"
        );
        assert_eq!(session.micro_breaks, 1);
    }
    #[test]
    fn a_tick_counter_that_wraps_is_not_a_two_day_absence() {
        let mut session = Session::default();
        session.update(1., 100.);
        session.update(1., f64::NAN);
        assert_eq!(session.idle_seconds, 0.);
        session.update(1., -5.);
        assert_eq!(session.idle_seconds, 0.);
        assert_eq!(session.state, IdleState::Active);
    }
}
