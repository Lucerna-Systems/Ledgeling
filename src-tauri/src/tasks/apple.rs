//! Apple Foundation Models adapter: the on-device model that ships with
//! macOS 27, reached through the `fm` CLI.
//!
//! `fm respond` is one-shot and takes a flat prompt, so it would force the
//! whole transcript to be re-flattened on every step of a task. `fm serve`
//! exposes the same on-device model over a local Chat Completions API, which
//! is the shape `HttpModel` already speaks — so this module owns only the
//! child process, and `providers.rs` talks to it like any other endpoint.
//!
//! The server is started on demand and stops itself once idle, because it
//! holds the model resident and a companion app must not keep that alive for
//! a task the user ran once an hour ago.

use serde_json::{json, Value};
use std::{
    net::TcpListener,
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// The only model `fm` exposes; the CLI rejects every other value. There is
/// no Private Cloud Compute backend in this build, so nothing here can leave
/// the machine.
pub const MODEL: &str = "system";
/// The prompt a task may send: the model's context is about 4,000 tokens,
/// and the reply needs room too.
pub const CONTEXT_CHARS: usize = 10_000;
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// First use pays for loading the model, so readiness is slower than a health
/// check on an ordinary server.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// An object in the dialect `fm serve` accepts: it rejects any object without
/// `title`, `x-order` and a closed `additionalProperties`.
fn object(title: &str, required: &[&str], properties: &[(&str, Value)]) -> Value {
    json!({
        "type": "object",
        "title": title,
        "x-order": properties.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "properties": properties.iter().cloned().map(|(n, v)| (n.to_string(), v)).collect::<serde_json::Map<_, _>>(),
        "required": required,
        "additionalProperties": false,
    })
}

/// A `response_format` that only admits one of `tools`, each with the
/// arguments the task loop reads. Unknown names are left out.
pub fn action_format(tools: &[&str]) -> Value {
    let text = || json!({ "type": "string" });
    let operation = object(
        "Operation",
        &["kind", "destination"],
        &[
            (
                "kind",
                json!({ "type": "string", "enum": ["move", "write"] }),
            ),
            ("source", text()),
            ("destination", text()),
            ("content", text()),
        ],
    );
    let choices: Vec<Value> = tools
        .iter()
        .filter_map(|tool| {
            let (title, args): (&str, Vec<(&str, Value)>) = match *tool {
                "finish" => ("Finish", vec![("text", text())]),
                "list_folder" => ("ListFolder", vec![("path", text())]),
                "list_files" => ("ListFiles", vec![]),
                "read_file" => ("ReadFile", vec![("path", text())]),
                "find_files" => (
                    "FindFiles",
                    vec![("query", text()), ("content", json!({ "type": "boolean" }))],
                ),
                "duplicates" => ("Duplicates", vec![]),
                "changes" => (
                    "Changes",
                    vec![("operations", json!({ "type": "array", "items": operation }))],
                ),
                "search_web" => ("SearchWeb", vec![("query", text())]),
                "read_page" => ("ReadPage", vec![("url", text())]),
                _ => return None,
            };
            let names: Vec<&str> = args.iter().map(|(name, _)| *name).collect();
            let args = object(&format!("{title}Args"), &names, &args);
            Some(object(
                title,
                &["tool", "args"],
                &[
                    ("tool", json!({ "type": "string", "enum": [tool] })),
                    ("args", args),
                ],
            ))
        })
        .collect();
    json!({
        "type": "json_schema",
        "json_schema": { "name": "action", "schema": { "title": "Action", "anyOf": choices } },
    })
}

/// Ledgeling's own terms for the on-device provider. Bump the version whenever
/// `TERMS` changes materially: a stored acceptance below the current version
/// stops counting, so the user is asked again rather than being held to text
/// they never saw.
pub const TERMS_VERSION: u32 = 1;
/// Placeholder wording pending legal review — it states the facts the user is
/// agreeing to, but it has not been drafted by a lawyer.
pub const TERMS: &str = "\
Ledgeling passes your request to Apple's on-device Foundation Model through the \
Apple Foundation Models CLI that ships with macOS. Ledgeling is not the author of \
that model and does not control what it produces.

Output from this model may be inaccurate, incomplete, or unsuitable for your \
purpose. Do not rely on it for legal, medical, financial, or safety-critical \
decisions. You are responsible for reviewing every action before you approve \
it; Ledgeling asks for approval before changing any file, and that approval is \
yours to give or refuse.

Your use of the model is also governed by Apple's own terms, which you accept \
separately and machine-wide by running `sudo fm license`. Ledgeling cannot accept \
those terms for you.

Ledgeling is provided as is, without warranty of any kind, and its authors are not \
liable for any loss arising from output generated by this model.";

/// Why the on-device model cannot be used, in the order worth telling a user
/// about: each variant is actionable and names the next step.
#[derive(Clone, Debug, PartialEq)]
pub enum Readiness {
    Ready,
    Unsupported,
    NotInstalled,
    LicenseNotAgreed,
    NotEnabled,
    Unavailable(String),
}
impl Readiness {
    pub fn is_ready(&self) -> bool {
        *self == Readiness::Ready
    }
    /// Whether the fix lives in System Settings, so the panel only offers that
    /// shortcut when it would actually help.
    pub fn needs_system_settings(&self) -> bool {
        matches!(self, Readiness::NotEnabled | Readiness::Unavailable(_))
    }
    /// The sentence shown on the AI providers page. Every non-ready case names the thing
    /// the user has to do, since none of them is something Ledgeling can fix.
    pub fn detail(&self) -> String {
        match self {
            Readiness::Ready => "Apple Intelligence is ready. Runs entirely on this Mac.".into(),
            Readiness::Unsupported => {
                "Apple Intelligence needs a Mac with Apple silicon on macOS 27 or later.".into()
            }
            Readiness::NotInstalled => {
                "The Apple Foundation Models CLI (fm) was not found on this Mac.".into()
            }
            Readiness::LicenseNotAgreed => {
                "Apple's own terms for the Foundation Models CLI have not been accepted on this Mac. Open Terminal and run: sudo fm license".into()
            }
            Readiness::NotEnabled => {
                "Apple Intelligence is turned off. Turn it on in System Settings, then check again.".into()
            }
            Readiness::Unavailable(reason) => format!(
                "Apple Intelligence is not ready: {reason} Check that it is on in System Settings and that the model has finished downloading."
            ),
        }
    }
}

fn fm() -> Option<std::path::PathBuf> {
    super::providers::find_in_path("fm")
}

/// Runs an `fm` subcommand that is expected to answer quickly. Long-running
/// subcommands (`serve`) are spawned instead; this is only for probes.
fn probe(args: &[&str]) -> Option<(bool, String)> {
    let output = Command::new(fm()?)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    }
    Some((output.status.success(), text))
}

/// Whether this Mac could run the model at all. Kept separate from the `fm`
/// probes so the UI can distinguish "wrong hardware" from "not set up yet"
/// without spawning a process.
pub fn supported() -> bool {
    cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")
}

/// Full readiness, cheapest checks first. `fm license --status` is the one
/// probe that answers without prompting, so it is safe to call from the UI.
pub fn readiness() -> Readiness {
    if !supported() {
        return Readiness::Unsupported;
    }
    if fm().is_none() {
        return Readiness::NotInstalled;
    }
    match probe(&["license", "--status"]) {
        Some((true, _)) => {}
        // Every `fm` subcommand refuses until the machine-wide terms are
        // accepted, so this has to be reported rather than retried.
        _ => return Readiness::LicenseNotAgreed,
    }
    match probe(&["available", "--model", MODEL]) {
        Some((true, _)) => Readiness::Ready,
        Some((false, reason)) if !reason.is_empty() => {
            let reason = strip_ansi(&reason);
            // `fm` reports the System Settings toggle as prose, so the one case
            // the user can fix in two clicks is separated from the rest.
            if reason.to_lowercase().contains("not enabled") {
                Readiness::NotEnabled
            } else {
                Readiness::Unavailable(reason)
            }
        }
        _ => Readiness::Unavailable("the model reported no availability.".into()),
    }
}

/// Opens the Siri & Apple Intelligence pane. The target is fixed here; nothing
/// from the frontend reaches the opener.
pub fn open_system_settings() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("System Settings is only available on macOS.".into());
    }
    open::that("x-apple.systempreferences:com.apple.Siri-Settings.extension")
        .map_err(|_| "Could not open System Settings.".into())
}

/// `fm` colours its errors, and those escape codes would otherwise be shown
/// verbatim in the settings panel.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Server {
    child: Child,
    port: u16,
    last_used: Instant,
}
impl Drop for Server {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGTERM);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(state_file());
    }
}

fn state_file() -> std::path::PathBuf {
    std::env::temp_dir().join("ledgeling-fm-serve.pid")
}

fn legacy_state_file() -> std::path::PathBuf {
    std::env::temp_dir().join("perch-fm-serve.pid")
}

fn reap_pid_file(path: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let _ = std::fs::remove_file(path);
    let Ok(pid) = text.trim().parse::<i32>() else {
        return;
    };
    if pid <= 1 {
        return;
    }
    // Pids are reused, so the process is confirmed to still be an `fm serve`
    // before anything is signalled.
    let Ok(out) = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
    else {
        return;
    };
    if String::from_utf8_lossy(&out.stdout).contains("fm serve") {
        #[cfg(unix)]
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
}

/// A hard kill — a crash, or the dev watcher restarting the app — never runs
/// `Drop` or the exit hook, and an orphaned server keeps the model resident.
/// The pid of the one we started is recorded so the next launch can reap it.
pub fn reap_stale() {
    reap_pid_file(&state_file());
    reap_pid_file(&legacy_state_file());
}

static SERVER: OnceLock<Mutex<Option<Server>>> = OnceLock::new();
fn server() -> &'static Mutex<Option<Server>> {
    SERVER.get_or_init(|| Mutex::new(None))
}

/// A free loopback port, chosen by the OS. `fm serve` has no "pick a port"
/// mode, so the port is claimed and released immediately before handing it
/// over.
fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|_| "Could not reserve a local port for Apple Intelligence.".to_string())?;
    listener
        .local_addr()
        .map(|a| a.port())
        .map_err(|_| "Could not reserve a local port for Apple Intelligence.".to_string())
}

fn healthy(port: u16) -> bool {
    let Ok(http) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    http.get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// The base URL of a running local server, starting one if needed. Returns the
/// Chat Completions root so callers can treat it as an ordinary endpoint.
pub fn endpoint(cancel: &super::Cancel) -> Result<String, String> {
    let ready = readiness();
    if !ready.is_ready() {
        return Err(ready.detail());
    }
    let mut guard = server()
        .lock()
        .map_err(|_| "Apple Intelligence server lock poisoned".to_string())?;
    if let Some(running) = guard.as_mut() {
        // A crashed server must not be handed out as if it were live.
        if running.child.try_wait().ok().flatten().is_none() && healthy(running.port) {
            running.last_used = Instant::now();
            return Ok(format!("http://127.0.0.1:{}/v1", running.port));
        }
        *guard = None;
    }
    let port = free_port()?;
    let mut command = Command::new(fm().ok_or("The fm command is no longer available.")?);
    command
        .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command
        .spawn()
        .map_err(|_| "Could not start the Apple Intelligence server.".to_string())?;
    let _ = std::fs::write(state_file(), child.id().to_string());
    let running = Server {
        child,
        port,
        last_used: Instant::now(),
    };
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        cancel.check()?;
        if healthy(port) {
            *guard = Some(running);
            drop(guard);
            watchdog();
            return Ok(format!("http://127.0.0.1:{port}/v1"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    // `running` drops here, so a server that never came up is not left behind.
    Err("The Apple Intelligence server did not start in time. Check Apple Intelligence in System Settings.".into())
}

static WATCHDOG: OnceLock<()> = OnceLock::new();
/// Stops the server once it has gone unused, so the model is not held resident
/// for the rest of the session after a single task.
fn watchdog() {
    WATCHDOG.get_or_init(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(15));
            let mut guard = match server().lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let idle = guard
                .as_ref()
                .is_some_and(|s| s.last_used.elapsed() > IDLE_TIMEOUT);
            if idle {
                *guard = None;
            }
        });
    });
}

/// Stops the server now. Called on app exit so `fm serve` never outlives Ledgeling.
pub fn shutdown() {
    if let Some(lock) = SERVER.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_text_is_readable_and_names_the_next_step() {
        assert!(Readiness::LicenseNotAgreed
            .detail()
            .contains("sudo fm license"));
        assert!(Readiness::Unsupported.detail().contains("Apple silicon"));
        assert!(!Readiness::Ready.detail().is_empty());
    }
    #[test]
    fn the_action_format_admits_only_the_offered_tools_in_the_dialect_fm_accepts() {
        let format = action_format(&["finish", "list_files", "changes", "not_a_tool"]);
        let choices = format["json_schema"]["schema"]["anyOf"].as_array().unwrap();
        let tools: Vec<_> = choices
            .iter()
            .map(|c| c["properties"]["tool"]["enum"][0].as_str().unwrap())
            .collect();
        assert_eq!(tools, ["finish", "list_files", "changes"]);
        // `fm serve` refuses any object missing these, so every nested one is checked.
        fn objects(v: &Value, out: &mut Vec<Value>) {
            if v["type"] == "object" {
                out.push(v.clone());
            }
            match v {
                Value::Object(map) => map.values().for_each(|c| objects(c, out)),
                Value::Array(items) => items.iter().for_each(|c| objects(c, out)),
                _ => {}
            }
        }
        let mut all = vec![];
        objects(&format["json_schema"]["schema"], &mut all);
        assert!(all.len() >= 7);
        for o in all {
            let keys: Vec<_> = o["properties"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            let mut order: Vec<String> = serde_json::from_value(o["x-order"].clone()).unwrap();
            order.sort();
            assert_eq!(order, keys, "{o}");
            assert_eq!(o["additionalProperties"], false);
            assert!(o["title"].is_string());
        }
    }
    #[test]
    fn colour_codes_never_reach_the_settings_panel() {
        assert_eq!(
            strip_ansi("\u{1b}[38;2;255;107;128mModel   unavailable.\u{1b}[0m"),
            "Model unavailable."
        );
    }
    #[test]
    fn a_reserved_port_is_usable_and_released() {
        let port = free_port().unwrap();
        assert!(port > 0);
        assert!(TcpListener::bind(format!("127.0.0.1:{port}")).is_ok());
    }
    #[test]
    fn terms_count_only_at_the_version_the_user_was_shown() {
        use super::super::{
            setup::ready,
            types::{Connections, ProviderKind, ProviderSetup},
        };
        let at = |terms_version| {
            let mut config = Connections::default();
            config.providers.insert(
                ProviderKind::Apple,
                ProviderSetup {
                    terms_version,
                    ..Default::default()
                },
            );
            config
        };
        assert!(!ready(&Connections::default(), &ProviderKind::Apple));
        assert!(!ready(&at(TERMS_VERSION - 1), &ProviderKind::Apple));
        assert!(ready(&at(TERMS_VERSION), &ProviderKind::Apple));
    }
    #[test]
    fn the_terms_state_what_is_being_agreed_to() {
        // The wording is a placeholder, but it must always carry the two facts
        // the user is accepting: output may be wrong, and Apple's own terms are
        // separate and cannot be accepted on their behalf.
        assert!(TERMS.contains("inaccurate"));
        assert!(TERMS.contains("sudo fm license"));
        assert!(Readiness::NotEnabled.needs_system_settings());
        assert!(!Readiness::LicenseNotAgreed.needs_system_settings());
        assert!(!Readiness::Ready.needs_system_settings());
    }
    /// The reaper signals a pid read from a file, so the guard that matters is
    /// the one against pid reuse: this test process is recorded, and must
    /// survive because its command is not `fm serve`.
    #[test]
    fn reaping_never_kills_a_process_that_is_not_our_server() {
        std::fs::write(state_file(), std::process::id().to_string()).unwrap();
        reap_stale();
        // Reaching this line at all is half the assertion: we were not
        // signalled. The record is cleared either way, so a bad pid is retried
        // at most once.
        assert!(!state_file().exists(), "the stale record must be cleared");
    }
    #[test]
    fn readiness_never_panics_and_matches_the_platform() {
        let ready = readiness();
        if !supported() {
            assert_eq!(ready, Readiness::Unsupported);
        }
    }
}
