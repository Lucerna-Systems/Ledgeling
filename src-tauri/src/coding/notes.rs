//! What a worktree's pet has to say: short notes about what happened there,
//! shown in its side chat. They stay in the chat, never as a popup, and live
//! only as long as the app, like the terminals they describe.
use super::pty::{Kind, Session};
use crate::pet::Attend;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Tone {
    Info,
    /// Something wants the user.
    Ask,
    Good,
    Bad,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: u64,
    /// Milliseconds since the epoch.
    pub at: u64,
    pub worktree: String,
    pub text: String,
    pub tone: Tone,
}

/// Enough for a long session; the oldest notes go first.
const KEEP: usize = 500;

#[derive(Default)]
pub struct Notes {
    list: VecDeque<Note>,
    next: u64,
}

impl Notes {
    pub fn add(&mut self, worktree: &str, text: impl Into<String>, tone: Tone) {
        self.next += 1;
        self.list.push_back(Note {
            id: self.next,
            at: super::pty::now_ms(),
            worktree: worktree.to_string(),
            text: text.into(),
            tone,
        });
        if self.list.len() > KEEP {
            self.list.pop_front();
        }
    }

    pub fn latest(&self) -> u64 {
        self.next
    }

    pub fn for_worktree(&self, worktree: &str, after: u64) -> Vec<Note> {
        self.list
            .iter()
            .filter(|n| n.worktree == worktree && n.id > after)
            .cloned()
            .collect()
    }
}

/// A session as last seen, to tell what changed since.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub kind: Kind,
    pub title: String,
    /// Started with a command, rather than an interactive shell.
    pub command: bool,
    pub attention: Option<Attend>,
    pub exit: Option<Option<u32>>,
}

impl Seen {
    pub fn of(session: &Session) -> Self {
        Self {
            kind: session.kind,
            title: session.title.clone(),
            command: session.command,
            attention: session.attention(),
            exit: session.exit(),
        }
    }
}

/// The note, if any, for a session going from `before` to `now`.
pub fn change(before: Option<&Seen>, now: &Seen) -> Option<(String, Tone)> {
    let title = &now.title;
    let Some(before) = before else {
        return match now.kind {
            Kind::Agent => Some((format!("{title} started."), Tone::Info)),
            Kind::Run => Some((format!("Started {title}."), Tone::Info)),
            Kind::Shell if now.command => Some((format!("Started {title}."), Tone::Info)),
            Kind::Shell => None,
        };
    };
    if before.exit.is_none() {
        if let Some(code) = now.exit {
            return Some(match (now.kind, code) {
                (Kind::Shell, _) if !now.command => (format!("{title} closed."), Tone::Info),
                (_, Some(0)) => (format!("{title} finished."), Tone::Good),
                (_, Some(code)) => (format!("{title} stopped with code {code}."), Tone::Bad),
                (_, None) => (format!("{title} stopped."), Tone::Info),
            });
        }
    }
    if now.kind == Kind::Agent
        && now.attention == Some(Attend::NeedsUser)
        && before.attention != Some(Attend::NeedsUser)
    {
        return Some((format!("{title} is waiting for you."), Tone::Ask));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(kind: Kind, command: bool) -> Seen {
        Seen {
            kind,
            title: "Claude Code".into(),
            command,
            attention: None,
            exit: None,
        }
    }

    #[test]
    fn an_agent_is_announced_and_its_questions_reported_once() {
        let started = seen(Kind::Agent, true);
        assert_eq!(change(None, &started).unwrap().0, "Claude Code started.");
        let asking = Seen {
            attention: Some(Attend::NeedsUser),
            ..started.clone()
        };
        assert_eq!(change(Some(&started), &asking).unwrap().1, Tone::Ask);
        assert_eq!(change(Some(&asking), &asking), None);
    }

    #[test]
    fn a_plain_shell_is_quiet_but_a_failed_command_is_not() {
        assert_eq!(change(None, &seen(Kind::Shell, false)), None);
        let build = Seen {
            title: "Preview build".into(),
            ..seen(Kind::Shell, true)
        };
        let failed = Seen {
            exit: Some(Some(1)),
            ..build.clone()
        };
        assert_eq!(
            change(Some(&build), &failed).unwrap(),
            ("Preview build stopped with code 1.".into(), Tone::Bad)
        );
        let done = Seen {
            exit: Some(Some(0)),
            ..build.clone()
        };
        assert_eq!(change(Some(&build), &done).unwrap().1, Tone::Good);
        assert_eq!(change(Some(&done), &done), None);
    }

    #[test]
    fn old_notes_make_room() {
        let mut notes = Notes::default();
        for i in 0..KEEP + 5 {
            notes.add("/a", format!("{i}"), Tone::Info);
        }
        notes.add("/b", "b", Tone::Info);
        assert_eq!(notes.for_worktree("/a", 0).len(), KEEP - 1);
        assert_eq!(notes.for_worktree("/b", 0).len(), 1);
        assert_eq!(notes.for_worktree("/a", notes.latest()).len(), 0);
    }
}
