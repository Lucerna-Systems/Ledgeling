//! The declarative behavior machine. States, durations, weighted transitions
//! and reactions come from the pack; the interpreter is the engine's.
use crate::packs::{
    manifest::{Event, Reaction, State},
    Action, Manifest, Signals,
};
use std::collections::HashMap;

/// Deterministic, seedable noise: behavior has to be reproducible in tests.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    /// Not `Iterator::next`: this is a plain 0..1 sample.
    pub fn sample(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % 1_000_000) as f64 / 1_000_000.
    }
    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        if high <= low {
            low
        } else {
            low + self.sample() * (high - low)
        }
    }
    pub fn pick(&mut self, count: usize) -> usize {
        if count == 0 {
            0
        } else {
            (self.sample() * count as f64) as usize % count
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entered {
    pub state: String,
    pub clip: String,
    pub action: Option<Action>,
    pub sound: Option<String>,
}

pub struct Machine {
    pub state: String,
    pub clip: String,
    pub action: Option<Action>,
    /// Priority band of whatever put the pet in this state; a reaction may only
    /// interrupt something at or below its own band, so survival always wins.
    pub priority: i32,
    timer: f64,
    awaiting_action: bool,
    cooldowns: HashMap<&'static str, f64>,
}

impl Machine {
    pub fn new(manifest: &Manifest, rng: &mut Rng) -> Self {
        let mut machine = Machine {
            state: manifest.behavior.initial.clone(),
            clip: "idle".into(),
            action: None,
            priority: 0,
            timer: 0.,
            awaiting_action: false,
            cooldowns: HashMap::new(),
        };
        machine.enter(&manifest.behavior.initial.clone(), 0, manifest, rng);
        machine
    }

    fn state_def<'a>(&self, manifest: &'a Manifest) -> Option<&'a State> {
        manifest.behavior.states.get(&self.state)
    }

    fn enter(&mut self, name: &str, priority: i32, manifest: &Manifest, rng: &mut Rng) -> Entered {
        let fallback = manifest.behavior.initial.clone();
        let name = if manifest.behavior.states.contains_key(name) {
            name.to_string()
        } else {
            fallback
        };
        let state = manifest.behavior.states.get(&name);
        let (clip, action, sound, duration) = match state {
            Some(state) => (
                manifest.resolve_clip(&state.clip).to_string(),
                state.action,
                state.sound.clone(),
                state.duration,
            ),
            None => ("idle".to_string(), None, None, (4., 10.)),
        };
        self.state = name.clone();
        self.clip = clip.clone();
        self.action = action;
        self.priority = priority;
        self.awaiting_action = action.is_some();
        self.timer = if action.is_some() && duration.1 <= 0. {
            // An action with no duration runs until the movement finishes.
            f64::INFINITY
        } else {
            rng.range(duration.0, duration.1)
        };
        Entered {
            state: name,
            clip,
            action,
            sound,
        }
    }

    /// Advance one tick. `action_done` reports whether any movement this state
    /// requested has finished. Returns the state entered, if it changed.
    pub fn tick(
        &mut self,
        dt: f64,
        manifest: &Manifest,
        signals: &Signals,
        events: &[Event],
        action_done: bool,
        rng: &mut Rng,
    ) -> Option<Entered> {
        for remaining in self.cooldowns.values_mut() {
            *remaining = (*remaining - dt).max(0.);
        }
        if let Some(reaction) = self.pick_reaction(manifest, events) {
            self.cooldowns
                .insert(reaction.on.as_str(), reaction.cooldown);
            let goto = reaction.goto.clone();
            let priority = reaction.priority;
            return Some(self.enter(&goto, priority, manifest, rng));
        }
        if action_done {
            self.awaiting_action = false;
        }
        self.timer -= dt;
        if self.awaiting_action || self.timer > 0. {
            return None;
        }
        let next = self.pick_transition(manifest, signals, rng);
        match next {
            Some(name) => Some(self.enter(&name, 0, manifest, rng)),
            None => {
                // Nothing is currently legal: hold this state and try again.
                self.timer = 1.;
                None
            }
        }
    }

    fn pick_reaction<'a>(&self, manifest: &'a Manifest, events: &[Event]) -> Option<&'a Reaction> {
        manifest
            .reactions
            .iter()
            .filter(|r| events.contains(&r.on))
            .filter(|r| self.cooldowns.get(r.on.as_str()).copied().unwrap_or(0.) <= 0.)
            .filter(|r| r.priority >= self.priority)
            .filter(|r| r.goto != self.state || r.priority >= 90)
            .max_by_key(|r| r.priority)
    }

    fn pick_transition(
        &self,
        manifest: &Manifest,
        signals: &Signals,
        rng: &mut Rng,
    ) -> Option<String> {
        let state = self.state_def(manifest)?;
        let options: Vec<(&str, f64)> = state
            .transitions
            .iter()
            .filter(|t| t.when.as_ref().is_none_or(|c| c.evaluate(signals)))
            .filter(|t| t.weight > 0.)
            .map(|t| (t.to.as_str(), t.weight))
            .collect();
        if options.is_empty() {
            return None;
        }
        let total: f64 = options.iter().map(|(_, w)| w).sum();
        let mut roll = rng.sample() * total;
        for (name, weight) in &options {
            roll -= weight;
            if roll <= 0. {
                return Some((*name).to_string());
            }
        }
        options.last().map(|(name, _)| (*name).to_string())
    }

    /// Force a state, used by Pet Lab's scenario buttons and direct interaction.
    pub fn force(
        &mut self,
        name: &str,
        priority: i32,
        manifest: &Manifest,
        rng: &mut Rng,
    ) -> Option<Entered> {
        manifest
            .behavior
            .states
            .contains_key(name)
            .then(|| self.enter(name, priority, manifest, rng))
    }

    pub fn cooldown_remaining(&self, event: Event) -> f64 {
        self.cooldowns.get(event.as_str()).copied().unwrap_or(0.)
    }
}
