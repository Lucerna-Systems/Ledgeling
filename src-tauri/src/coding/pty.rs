//! Terminals: real pseudo-terminals running the user's own shell and tools,
//! including the Claude Code they installed themselves.
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::ipc::Channel;

#[derive(Clone, Serialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Output {
    Data { data: String },
    Exit { code: Option<u32> },
}

#[derive(Clone, Copy, Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Shell,
    /// An AI agent such as `claude`: its pet shows it working.
    Agent,
    /// The project's own run command, from the Run tab.
    Run,
}

pub struct Session {
    pub kind: Kind,
    pub worktree: String,
    pub title: String,
    pub pid: Option<u32>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    /// When the process last printed, in ms since the epoch; 0 once it exited.
    pub last_output: Arc<AtomicU64>,
    /// Set when the program rang the terminal bell: it wants the user.
    pub bell: Arc<AtomicU64>,
    sink: Arc<Mutex<Sink>>,
}

/// Where output goes. The window can close and reopen while a program keeps
/// running, so recent output is kept for the next viewer.
struct Sink {
    channel: Option<Channel<Output>>,
    scrollback: String,
    exit: Option<Option<u32>>,
}

const SCROLLBACK: usize = 256 * 1024;

impl Sink {
    fn push(&mut self, text: String) {
        self.scrollback.push_str(&text);
        if self.scrollback.len() > SCROLLBACK {
            let mut cut = self.scrollback.len() - SCROLLBACK;
            while !self.scrollback.is_char_boundary(cut) {
                cut += 1;
            }
            self.scrollback.drain(..cut);
        }
        if let Some(channel) = &self.channel {
            if channel.send(Output::Data { data: text }).is_err() {
                self.channel = None;
            }
        }
    }
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: u32,
    pub kind: Kind,
    pub worktree: String,
    pub title: String,
    pub pid: Option<u32>,
    pub alive: bool,
    /// For agents: `busy` while printing, `needs_user` after a bell.
    pub attention: Option<crate::pet::Attend>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Programs started from a GUI app on macOS inherit launchd's bare PATH; the
/// login shell's is what the user's terminal (and `claude`) expects.
pub fn login_path() -> Option<String> {
    #[cfg(unix)]
    {
        static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        PATH.get_or_init(|| {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
            let output = std::process::Command::new(shell)
                .args(["-ilc", "printf %s \"$PATH\""])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!text.is_empty()).then_some(text)
        })
        .clone()
    }
    #[cfg(not(unix))]
    {
        None
    }
}

pub struct Spawn<'a> {
    pub cwd: &'a Path,
    /// `None` runs the user's login shell.
    pub command: Option<&'a str>,
    pub kind: Kind,
    pub worktree: String,
    pub title: String,
    pub cols: u16,
    pub rows: u16,
    pub env: Vec<(String, String)>,
}

pub fn spawn(options: Spawn, channel: Channel<Output>) -> Result<Session, String> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: options.rows.max(2),
            cols: options.cols.max(10),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Could not open a terminal: {e}"))?;
    let mut builder = match options.command {
        None => CommandBuilder::new_default_prog(),
        Some(line) => shell_line(line),
    };
    builder.cwd(options.cwd);
    builder.env("TERM", "xterm-256color");
    builder.env("COLORTERM", "truecolor");
    builder.env("TERM_PROGRAM", "Ledgeling");
    if let Some(path) = login_path() {
        builder.env("PATH", path);
    }
    // Ledgeling's own Claude runtime marker must not leak into the user's tools.
    builder.env_remove("CLAUDECODE");
    for (key, value) in &options.env {
        builder.env(key, value);
    }
    let mut child = pair
        .slave
        .spawn_command(builder)
        .map_err(|e| format!("Could not start the program: {e}"))?;
    drop(pair.slave);
    let pid = child.process_id();
    let killer = child.clone_killer();
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let last_output = Arc::new(AtomicU64::new(now_ms()));
    let bell = Arc::new(AtomicU64::new(0));
    let sink = Arc::new(Mutex::new(Sink {
        channel: Some(channel),
        scrollback: String::new(),
        exit: None,
    }));
    let (last, rang, output) = (last_output.clone(), bell.clone(), sink.clone());
    std::thread::spawn(move || {
        let mut buffer = [0u8; 16 * 1024];
        // A multi-byte character can be split across two reads.
        let mut carry: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    last.store(now_ms(), Ordering::Relaxed);
                    if buffer[..n].contains(&0x07) {
                        rang.store(now_ms(), Ordering::Relaxed);
                    }
                    carry.extend_from_slice(&buffer[..n]);
                    let valid = match std::str::from_utf8(&carry) {
                        Ok(_) => carry.len(),
                        Err(error) if error.error_len().is_none() => error.valid_up_to(),
                        Err(_) => carry.len(),
                    };
                    let text = String::from_utf8_lossy(&carry[..valid]).into_owned();
                    carry.drain(..valid);
                    if let Ok(mut sink) = output.lock() {
                        sink.push(text);
                    }
                }
            }
        }
        // Waiting only after the output drained keeps the last lines ahead of
        // the exit notice.
        let code = child.wait().ok().map(|status| status.exit_code());
        last.store(0, Ordering::Relaxed);
        if let Ok(mut sink) = output.lock() {
            sink.exit = Some(code);
            if let Some(channel) = &sink.channel {
                let _ = channel.send(Output::Exit { code });
            }
        }
    });
    Ok(Session {
        kind: options.kind,
        worktree: options.worktree,
        title: options.title,
        pid,
        master: pair.master,
        writer,
        killer,
        last_output,
        bell,
        sink,
    })
}

/// Run a command line through the user's shell, so pipes, `&&` and their
/// aliases behave as in their own terminal.
fn shell_line(line: &str) -> CommandBuilder {
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut builder = CommandBuilder::new(shell);
        builder.args(["-lc", line]);
        builder
    }
    #[cfg(windows)]
    {
        let mut builder = CommandBuilder::new("cmd.exe");
        builder.args(["/d", "/s", "/c", line]);
        builder
    }
}

impl Session {
    pub fn write(&mut self, data: &str) -> Result<(), String> {
        // Typing answers whatever the bell asked for.
        self.bell.store(0, Ordering::Relaxed);
        self.writer
            .write_all(data.as_bytes())
            .and_then(|_| self.writer.flush())
            .map_err(|e| e.to_string())
    }

    /// Point output at a new viewer, replaying what it missed.
    pub fn attach(&self, channel: Channel<Output>) {
        if let Ok(mut sink) = self.sink.lock() {
            if !sink.scrollback.is_empty() {
                let _ = channel.send(Output::Data {
                    data: sink.scrollback.clone(),
                });
            }
            if let Some(code) = sink.exit {
                let _ = channel.send(Output::Exit { code });
            }
            sink.channel = Some(channel);
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows: rows.max(2),
                cols: cols.max(10),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    pub fn alive(&self) -> bool {
        self.last_output.load(Ordering::Relaxed) != 0
    }

    /// Ends the whole process group: a dev server's children must not
    /// outlive the tab that started them.
    pub fn kill(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid {
            // The child is a session leader (setsid), so its pid is the group.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
            let pid = pid as i32;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            });
        }
        let _ = self.killer.kill();
    }

    pub fn view(&self, id: u32) -> SessionView {
        SessionView {
            id,
            kind: self.kind,
            worktree: self.worktree.clone(),
            title: self.title.clone(),
            pid: self.pid,
            alive: self.alive(),
            attention: self.attention(),
        }
    }

    /// What this session means for its pet right now.
    pub fn attention(&self) -> Option<crate::pet::Attend> {
        if self.kind != Kind::Agent || !self.alive() {
            return None;
        }
        let now = now_ms();
        if self.bell.load(Ordering::Relaxed) != 0 {
            return Some(crate::pet::Attend::NeedsUser);
        }
        // Printing within the last few seconds reads as working.
        (now.saturating_sub(self.last_output.load(Ordering::Relaxed)) < 4000)
            .then_some(crate::pet::Attend::Busy)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_command_runs_in_a_terminal_and_reports_its_exit() {
        let dir = tempfile::tempdir().unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let exit = Arc::new(Mutex::new(None));
        let (s, e) = (seen.clone(), exit.clone());
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(text) = body {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                match value["type"].as_str() {
                    Some("data") => s.lock().unwrap().push_str(value["data"].as_str().unwrap()),
                    Some("exit") => *e.lock().unwrap() = Some(value["code"].as_u64()),
                    _ => {}
                }
            }
            Ok(())
        });
        let session = spawn(
            Spawn {
                cwd: dir.path(),
                command: Some("printf 'hello-%s' $((40+2))"),
                kind: Kind::Shell,
                worktree: String::new(),
                title: "t".into(),
                cols: 80,
                rows: 24,
                env: vec![],
            },
            channel,
        )
        .unwrap();
        for _ in 0..100 {
            if exit.lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(seen.lock().unwrap().contains("hello-42"));
        assert_eq!(*exit.lock().unwrap(), Some(Some(0)));
        assert!(!session.alive());
    }
}
