//! What a coding agent says about itself.
//!
//! Output alone can only guess whether an agent is working (it is printing) or
//! waiting for you (it rang the bell). Claude Code can say so itself, through
//! hooks. Ledgeling hands each `claude` it starts a settings file for that run
//! only (`--settings`), so the user's own configuration is never edited. Each
//! hook runs `ledgeling --agent-event`, which appends one small line to a file
//! in Ledgeling's own folder; the session reads the file as it grows. Nothing
//! leaves the machine, and only the event name, tool name and notification
//! text are kept: never prompts, file contents or tool input.
//!
//! The same line format is provider-neutral, so another agent that can run a
//! command on its events can feed it too.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// The events file a session's hooks append to.
pub const EVENTS_ENV: &str = "LEDGELING_EVENTS";
/// Passed to Ledgeling's own binary by an agent's hook.
pub const HOOK_FLAG: &str = "--agent-event";
const MAX_MESSAGE: usize = 160;

/// One thing an agent reported.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub event: String,
    #[serde(default)]
    pub tool: Option<String>,
    /// Why a notification fired, e.g. `permission_prompt` or `idle_prompt`.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Started, or between turns with nothing to report.
    Idle,
    Working,
    /// Needs the user: a permission to grant or a question to answer.
    Waiting,
    /// Finished its turn.
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AgentStatus {
    pub state: State,
    /// The tool in use, or what it is waiting for.
    pub detail: Option<String>,
}

impl AgentStatus {
    pub fn idle() -> Self {
        Self {
            state: State::Idle,
            detail: None,
        }
    }
}

/// How an event moves the status. Unknown events change nothing, so a newer
/// agent version sending something new cannot confuse it.
pub fn advance(status: &AgentStatus, event: &Event) -> AgentStatus {
    let state = |state, detail| AgentStatus { state, detail };
    match event.event.as_str() {
        "SessionStart" | "SessionEnd" => AgentStatus::idle(),
        "UserPromptSubmit" => state(State::Working, None),
        "PreToolUse" => state(State::Working, event.tool.clone()),
        "PostToolUse" | "SubagentStop" => state(State::Working, status.detail.clone()),
        // Claude also says "waiting for your input" a minute after it finished.
        // That is not asking for anything, so it stays done.
        "Notification" if event.kind.as_deref() == Some("idle_prompt") => state(State::Done, None),
        "Notification" => state(
            State::Waiting,
            event.message.clone().or_else(|| status.detail.clone()),
        ),
        "Stop" => state(State::Done, None),
        _ => status.clone(),
    }
}

/// Follows one session's events file as the agent appends to it.
pub struct Tracker {
    path: PathBuf,
    offset: u64,
    partial: String,
    pub status: AgentStatus,
    /// Whether the agent has reported anything at all. Until it has, the
    /// session falls back to guessing from its output.
    pub seen: bool,
}

impl Tracker {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial: String::new(),
            status: AgentStatus::idle(),
            seen: false,
        }
    }

    /// Reads whatever was appended since last time.
    pub fn poll(&mut self) {
        let Ok(mut file) = fs::File::open(&self.path) else {
            return;
        };
        let length = file.metadata().map(|m| m.len()).unwrap_or(0);
        if length < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        if length == self.offset || file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut chunk = Vec::new();
        if file.take(1 << 20).read_to_end(&mut chunk).is_err() {
            return;
        }
        self.offset += chunk.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&chunk));
        // Only whole lines count: the agent may be mid-write.
        let Some(end) = self.partial.rfind('\n') else {
            return;
        };
        let complete: String = self.partial.drain(..=end).collect();
        for line in complete.lines() {
            if let Ok(event) = serde_json::from_str::<Event>(line) {
                self.status = advance(&self.status, &event);
                self.seen = true;
            }
        }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

// ------------------------------------------------------------- installed CLIs

/// A coding agent that runs in a terminal.
struct Cli {
    id: &'static str,
    name: &'static str,
    program: &'static str,
}

/// The agents offered as one-click starts when they are on the user's PATH.
const CLIS: &[Cli] = &[
    Cli {
        id: "claude",
        name: "Claude Code",
        program: "claude",
    },
    Cli {
        id: "codex",
        name: "Codex",
        program: "codex",
    },
    Cli {
        id: "antigravity",
        name: "Antigravity",
        program: "agy",
    },
    Cli {
        id: "gemini",
        name: "Gemini",
        program: "gemini",
    },
    Cli {
        id: "grok",
        name: "Grok",
        program: "grok",
    },
    Cli {
        id: "opencode",
        name: "OpenCode",
        program: "opencode",
    },
    Cli {
        id: "cursor",
        name: "Cursor",
        program: "cursor-agent",
    },
    Cli {
        id: "copilot",
        name: "Copilot",
        program: "copilot",
    },
    Cli {
        id: "amp",
        name: "Amp",
        program: "amp",
    },
    Cli {
        id: "goose",
        name: "Goose",
        program: "goose",
    },
    Cli {
        id: "aider",
        name: "Aider",
        program: "aider",
    },
];

/// An agent found on the user's PATH.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Installed {
    pub id: &'static str,
    pub name: &'static str,
    /// What to run in a terminal to start it.
    pub command: &'static str,
}

/// Which of the known agents are installed on `path`.
pub fn detect(path: &str) -> Vec<Installed> {
    CLIS.iter()
        .filter(|cli| super::pty::find_in(cli.program, path).is_some())
        .map(|cli| Installed {
            id: cli.id,
            name: cli.name,
            command: cli.program,
        })
        .collect()
}

/// The installed agents, looked up on the login shell's PATH (the one
/// terminals use) and remembered briefly, since the window asks every second or two.
pub fn installed() -> Vec<Installed> {
    use std::{
        sync::Mutex,
        time::{Duration, Instant},
    };
    static CACHE: Mutex<Option<(Instant, Vec<Installed>)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, found)) = cache.as_ref() {
        if at.elapsed() < Duration::from_secs(20) {
            return found.clone();
        }
    }
    let path = super::pty::login_path()
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();
    let found = detect(&path);
    *cache = Some((Instant::now(), found.clone()));
    found
}

// ------------------------------------------------------------------ setup

/// Where agent settings and event files live.
pub fn directory(config_dir: &Path) -> PathBuf {
    config_dir.join("agents")
}

/// Clears event files left by a previous run. Called once at start-up.
pub fn reset(config_dir: &Path) {
    let events = directory(config_dir).join("events");
    let _ = fs::remove_dir_all(&events);
}

fn private_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn quote(text: &str) -> String {
    if cfg!(windows) {
        format!("\"{text}\"")
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

/// The settings that make Claude Code report to Ledgeling.
pub fn claude_settings(hook_command: &str) -> serde_json::Value {
    let hook = serde_json::json!([{ "hooks": [{ "type": "command", "command": hook_command }] }]);
    let mut hooks = serde_json::Map::new();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "Stop",
        "SessionEnd",
    ] {
        hooks.insert(event.into(), hook.clone());
    }
    serde_json::json!({ "hooks": hooks })
}

pub struct Prepared {
    pub command: String,
    pub env: Vec<(String, String)>,
    pub events: Option<PathBuf>,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Makes a `claude ...` command line report its events. Anything else, or any
/// failure to set up, runs unchanged and is watched by its output as before.
pub fn prepare(command: &str, config_dir: &Path) -> Prepared {
    let plain = || Prepared {
        command: command.to_string(),
        env: Vec::new(),
        events: None,
    };
    let trimmed = command.trim_start();
    let (program, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((program, rest)) => (program, rest),
        None => (trimmed, ""),
    };
    let name = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name != "claude" && name != "claude.exe" && name != "claude.cmd" {
        return plain();
    }
    let Ok(exe) = std::env::current_exe() else {
        return plain();
    };
    let base = directory(config_dir);
    let events_dir = base.join("events");
    if private_dir(&base).is_err() || private_dir(&events_dir).is_err() {
        return plain();
    }
    let settings_path = base.join("claude-hooks.json");
    let hook = format!("{} {HOOK_FLAG}", quote(&exe.to_string_lossy()));
    let Ok(text) = serde_json::to_string_pretty(&claude_settings(&hook)) else {
        return plain();
    };
    if fs::write(&settings_path, text).is_err() {
        return plain();
    }
    let events = events_dir.join(format!(
        "{}-{}.jsonl",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    Prepared {
        command: format!(
            "{program} --settings {} {rest}",
            quote(&settings_path.to_string_lossy())
        )
        .trim_end()
        .to_string(),
        env: vec![(EVENTS_ENV.into(), events.to_string_lossy().into_owned())],
        events: Some(events),
    }
}

// ------------------------------------------------------------------- hook

/// What `ledgeling --agent-event` does: read the agent's JSON from stdin and
/// append the few fields Ledgeling uses to the session's events file. It never
/// prints and never fails the hook, so it cannot get in the agent's way.
pub fn record_from_stdin() {
    let Some(path) = std::env::var_os(EVENTS_ENV).map(PathBuf::from) else {
        return;
    };
    let mut input = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut input);
    let Some(event) = parse_hook(&input) else {
        return;
    };
    let Ok(mut line) = serde_json::to_string(&event) else {
        return;
    };
    line.push('\n');
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        // One write, so concurrent hooks do not interleave inside a line.
        let _ = file.write_all(line.as_bytes());
    }
}

/// Picks the fields we use out of an agent's hook payload.
pub fn parse_hook(input: &str) -> Option<Event> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let text = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_string);
    Some(Event {
        event: text("hook_event_name")?,
        tool: text("tool_name"),
        kind: text("notification_type"),
        message: text("message").map(|m| m.chars().take(MAX_MESSAGE).collect()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str) -> Event {
        Event {
            event: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_turn_runs_from_prompt_through_tools_to_done() {
        let mut status = AgentStatus::idle();
        status = advance(&status, &event("UserPromptSubmit"));
        assert_eq!(status.state, State::Working);
        status = advance(
            &status,
            &Event {
                tool: Some("Bash".into()),
                ..event("PreToolUse")
            },
        );
        assert_eq!(status.detail.as_deref(), Some("Bash"));
        status = advance(&status, &event("PostToolUse"));
        assert_eq!(
            (status.state, status.detail.as_deref()),
            (State::Working, Some("Bash"))
        );
        status = advance(&status, &event("Stop"));
        assert_eq!(status.state, State::Done);
    }

    #[test]
    fn a_permission_request_waits_but_an_idle_reminder_does_not() {
        let asked = Event {
            message: Some("Claude needs your permission to use Bash".into()),
            kind: Some("permission_prompt".into()),
            ..event("Notification")
        };
        let status = advance(&AgentStatus::idle(), &asked);
        assert_eq!(status.state, State::Waiting);
        assert!(status.detail.unwrap().contains("permission"));
        let reminder = Event {
            kind: Some("idle_prompt".into()),
            ..event("Notification")
        };
        assert_eq!(advance(&AgentStatus::idle(), &reminder).state, State::Done);
    }

    #[test]
    fn unknown_events_change_nothing() {
        let working = AgentStatus {
            state: State::Working,
            detail: Some("Edit".into()),
        };
        assert_eq!(advance(&working, &event("SomethingNew")), working);
    }

    #[test]
    fn hook_payloads_keep_only_the_fields_ledgeling_uses() {
        let payload = r#"{"hook_event_name":"PreToolUse","tool_name":"Write","tool_input":{"content":"SECRET"},"prompt":"also secret","session_id":"x"}"#;
        let event = parse_hook(payload).unwrap();
        assert_eq!(event.tool.as_deref(), Some("Write"));
        let line = serde_json::to_string(&event).unwrap();
        assert!(!line.contains("SECRET") && !line.contains("secret"));
        assert!(parse_hook("not json").is_none());
    }

    #[test]
    fn the_tracker_reads_whole_lines_as_they_arrive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut tracker = Tracker::new(path.clone());
        tracker.poll();
        assert!(!tracker.seen);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"{\"event\":\"UserPromptSubmit\"}\n{\"event\":\"Pre")
            .unwrap();
        tracker.poll();
        assert_eq!(tracker.status.state, State::Working);
        file.write_all(b"ToolUse\",\"tool\":\"Edit\"}\n").unwrap();
        tracker.poll();
        assert_eq!(tracker.status.detail.as_deref(), Some("Edit"));
        drop(file);
        drop(tracker);
        assert!(!path.exists(), "the events file goes with its session");
    }

    #[test]
    fn installed_agents_are_found_on_the_path() {
        let dir = tempfile::tempdir().unwrap();
        for program in ["claude", "agy", "codex", "grok", "not-an-agent"] {
            fs::write(dir.path().join(program), "").unwrap();
        }
        let found: Vec<_> = detect(&dir.path().to_string_lossy())
            .into_iter()
            .map(|a| (a.id, a.command))
            .collect();
        assert_eq!(
            found,
            vec![
                ("claude", "claude"),
                ("codex", "codex"),
                ("antigravity", "agy"),
                ("grok", "grok"),
            ]
        );
        assert!(detect("").is_empty());
    }

    #[test]
    fn only_claude_commands_are_wrapped_and_the_prompt_stays_last() {
        let dir = tempfile::tempdir().unwrap();
        assert!(prepare("npm run dev", dir.path()).events.is_none());
        let wrapped = prepare("claude \"set this up\"", dir.path());
        assert!(wrapped.events.is_some());
        assert!(wrapped.command.starts_with("claude --settings "));
        assert!(wrapped.command.ends_with("\"set this up\""));
        let settings = fs::read_to_string(directory(dir.path()).join("claude-hooks.json")).unwrap();
        assert!(settings.contains(HOOK_FLAG) && settings.contains("PreToolUse"));
        assert_eq!(wrapped.env[0].0, EVENTS_ENV);
    }
}
