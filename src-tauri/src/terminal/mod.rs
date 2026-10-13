//! Terminals as a feature of their own, so that terminal mode (a pet's own
//! terminal window) and coding mode run the same code: real pseudo-terminals
//! running the user's login shell and the agents they installed, with each
//! agent's events feeding its pet.
pub mod agent;
pub mod commands;
pub mod pty;
pub mod window;

use serde::Serialize;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

/// Running terminals by id. Dereferences to the map, so callers read and
/// remove sessions directly; only handing out ids lives here.
#[derive(Default)]
pub struct Registry {
    sessions: HashMap<u32, pty::Session>,
    last_id: u32,
}

impl Registry {
    pub fn next_id(&mut self) -> u32 {
        self.last_id += 1;
        self.last_id
    }

    /// Adds a session and returns its id.
    pub fn add(&mut self, session: pty::Session) -> u32 {
        let id = self.next_id();
        self.sessions.insert(id, session);
        id
    }

}

impl crate::service::SubsystemService for Registry {
    /// Stops every terminal and everything it started.
    fn shutdown(&mut self) {
        for (_, mut session) in self.sessions.drain() {
            session.kill();
        }
    }
}

impl Deref for Registry {
    type Target = HashMap<u32, pty::Session>;
    fn deref(&self) -> &Self::Target {
        &self.sessions
    }
}

impl DerefMut for Registry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sessions
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        use crate::service::SubsystemService;
        self.shutdown();
    }
}

/// The pet a terminal window belongs to, and where it works.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub pet_id: u64,
    pub pet_name: String,
    pub folder: String,
}

/// Everything terminal mode holds. Behind its own lock, never the engine's, so
/// terminal output can stream without touching pets.
#[derive(Default)]
pub struct Terminal {
    pub sessions: Registry,
    /// Folders a terminal may start in, each with the pet that opened it.
    pub folders: HashMap<String, u64>,
    pub context: Option<Context>,
    /// Each folder's split layout, as the window last drew it.
    pub layouts: HashMap<String, serde_json::Value>,
    /// Whether the loop that lets pets show their agents' state is running.
    pub watching: bool,
}

impl crate::service::SubsystemService for Terminal {
    fn shutdown(&mut self) {
        self.sessions.shutdown();
        self.folders.clear();
        self.layouts.clear();
        self.context = None;
        self.watching = false;
    }
}

pub type SharedTerminal = Arc<Mutex<Terminal>>;
