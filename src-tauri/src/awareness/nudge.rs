//! The nudge ladder. Three tiers, long cooldowns, a hard cap on escalation,
//! and no popup, toast, text or focus steal anywhere in it — the pet's body
//! language is the entire communication channel.
use super::AppCategory;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NudgeConfig {
    pub enabled: bool,
    /// Unbroken-stretch minutes at which each tier begins. Deliberately long.
    pub tier1_minutes: f64,
    pub tier2_minutes: f64,
    pub tier3_minutes: f64,
    pub cooldown_seconds: f64,
}
impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tier1_minutes: 45.,
            tier2_minutes: 75.,
            tier3_minutes: 110.,
            cooldown_seconds: 900.,
        }
    }
}

pub struct NudgeLadder {
    pub config: NudgeConfig,
    tier: u8,
    cooldown: f64,
    fired_tier: u8,
}

impl NudgeLadder {
    pub fn new(config: NudgeConfig) -> Self {
        Self {
            config,
            tier: 0,
            cooldown: 0.,
            fired_tier: 0,
        }
    }

    pub fn tier(&self) -> u8 {
        self.tier
    }

    pub fn reset(&mut self) {
        self.tier = 0;
        self.fired_tier = 0;
        self.cooldown = 0.;
    }

    /// Returns true when a nudge should fire this tick.
    pub fn tick(&mut self, dt: f64, stretch_minutes: f64, category: AppCategory) -> bool {
        self.cooldown = (self.cooldown - dt).max(0.);
        // Someone watching a film or playing a game is not failing to take a
        // break, so the ladder is suppressed entirely in those categories.
        let suppressed = matches!(category, AppCategory::Media | AppCategory::Game);
        if !self.config.enabled || suppressed {
            self.tier = 0;
            return false;
        }
        let tier = if stretch_minutes >= self.config.tier3_minutes {
            3
        } else if stretch_minutes >= self.config.tier2_minutes {
            2
        } else if stretch_minutes >= self.config.tier1_minutes {
            1
        } else {
            0
        };
        if tier < self.tier {
            // The stretch was reset by a break: start the ladder again.
            self.fired_tier = 0;
        }
        self.tier = tier;
        if tier == 0 {
            return false;
        }
        // Escalation caps at tier 3, and a tier only re-fires after the cooldown.
        if tier > self.fired_tier || (tier == 3 && self.cooldown <= 0.) {
            self.fired_tier = tier;
            self.cooldown = self.config.cooldown_seconds;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escalates_once_per_tier_and_caps_at_three() {
        let mut ladder = NudgeLadder::new(NudgeConfig::default());
        assert!(!ladder.tick(1., 44., AppCategory::Code));
        assert!(ladder.tick(1., 46., AppCategory::Code));
        assert!(
            !ladder.tick(1., 50., AppCategory::Code),
            "no re-fire inside a tier"
        );
        assert!(ladder.tick(1., 80., AppCategory::Code));
        assert!(ladder.tick(1., 115., AppCategory::Code));
        assert_eq!(ladder.tier(), 3);
        assert!(!ladder.tick(1., 300., AppCategory::Code), "cooldown holds");
        assert!(!ladder.tick(800., 300., AppCategory::Code));
        assert!(
            ladder.tick(120., 300., AppCategory::Code),
            "tier 3 repeats after cooldown"
        );
        assert_eq!(ladder.tier(), 3, "escalation caps");
    }
    #[test]
    fn suppressed_in_games_and_films_and_when_disabled() {
        let mut ladder = NudgeLadder::new(NudgeConfig::default());
        assert!(!ladder.tick(1., 200., AppCategory::Game));
        assert!(!ladder.tick(1., 200., AppCategory::Media));
        assert_eq!(ladder.tier(), 0);
        ladder.config.enabled = false;
        assert!(!ladder.tick(1., 200., AppCategory::Code));
    }
    #[test]
    fn a_break_restarts_the_ladder() {
        let mut ladder = NudgeLadder::new(NudgeConfig::default());
        assert!(ladder.tick(1., 46., AppCategory::Code));
        assert!(!ladder.tick(1., 0., AppCategory::Code));
        assert!(ladder.tick(1., 46., AppCategory::Code));
    }
}
