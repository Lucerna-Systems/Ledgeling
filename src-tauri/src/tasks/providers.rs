use super::{apple, types::*, Cancel};
use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    sync::mpsc::{self, RecvTimeoutError},
    time::Duration,
};

pub trait ModelProvider {
    fn complete(&self, messages: &[Message], cancel: &Cancel) -> Result<String, String>;
    /// One task action, held to `tools` by providers that can constrain their
    /// output; the rest are only told about them in the system prompt.
    fn act(&self, messages: &[Message], tools: &[&str], cancel: &Cancel) -> Result<String, String> {
        let _ = tools;
        self.complete(messages, cancel)
    }
    fn capabilities(&self, cancel: &Cancel) -> Result<Capabilities, String>;
    /// Whether the model answers in one turn from context Ledgeling gathers,
    /// instead of exploring with tools.
    fn single_turn(&self) -> bool {
        false
    }
}
pub fn secret_name(kind: &ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Openai => "openai",
        ProviderKind::Anthropic | ProviderKind::Claude => "anthropic",
        ProviderKind::Compatible => "compatible",
        _ => "unused",
    }
}
/// Where API keys live in the OS credential store.
const SECRET_SERVICE: &str = "technology.lucerna-systems.ledgeling.tasks";
const CREDENTIALS: [&str; 4] = ["openai", "anthropic", "compatible", "brave"];

/// Copies keys saved under the pre-rename service into the current one, then
/// removes the originals. A key already saved under the new service wins.
pub fn migrate_legacy_secrets() {
    let legacy = format!("{}.tasks", crate::migrate::LEGACY_IDENTIFIER);
    for name in CREDENTIALS {
        let Ok(old) = keyring::Entry::new(&legacy, name) else {
            continue;
        };
        let Ok(value) = old.get_password() else {
            continue;
        };
        if !has_secret(name) && set_secret(name, &value).is_err() {
            continue;
        }
        let _ = old.delete_credential();
    }
}
pub fn set_secret(name: &str, value: &str) -> Result<(), String> {
    if !CREDENTIALS.contains(&name) {
        return Err("Unknown credential.".into());
    }
    let entry = keyring::Entry::new(SECRET_SERVICE, name)
        .map_err(|_| "Credential store unavailable".to_string())?;
    if value.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not delete credential".into()),
        }
    } else {
        entry
            .set_password(value)
            .map_err(|_| "Could not save credential in the OS store".into())
    }
}
pub fn secret(name: &str) -> Result<String, String> {
    keyring::Entry::new(SECRET_SERVICE, name)
        .and_then(|e| e.get_password())
        .map_err(|_| format!("Add the {name} API key on the AI providers page."))
}
pub fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialise HTTP client".into())
}
fn check_status(response: &Response) -> Result<(), String> {
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("Provider returned HTTP {}. Check your connection, model, credentials and usage limits.", status.as_u16()))
    }
}
/// Runs a blocking provider call on its own thread, so a cancel is noticed
/// within 100 ms instead of after the reply or the 90-second timeout. The one
/// task worker is shared by every chat, so a call it cannot leave holds them all.
/// The thread is abandoned on cancel and ends when the call does.
fn cancellable<T: Send + 'static>(
    cancel: &Cancel,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (done, wait) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(work());
    });
    loop {
        match wait.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => return result,
            Err(RecvTimeoutError::Timeout) => cancel.check()?,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("The request stopped unexpectedly.".into())
            }
        }
    }
}
/// The text of an Ollama streamed reply (one JSON object per line). Returning
/// early drops the connection, which is what makes Ollama stop generating.
fn read_stream(reader: impl Read, cancel: &Cancel) -> Result<String, String> {
    let mut text = String::new();
    for line in BufReader::new(reader.take(2_000_001)).lines() {
        cancel.check()?;
        let line = line.map_err(|_| "Provider response interrupted".to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let chunk: Value = serde_json::from_str(&line)
            .map_err(|_| "Provider returned invalid JSON.".to_string())?;
        if let Some(error) = chunk["error"].as_str() {
            return Err(format!("Ollama reported an error: {error}"));
        }
        text.push_str(chunk["message"]["content"].as_str().unwrap_or(""));
        if text.len() > 2_000_000 {
            return Err("Provider response exceeded 2 MB.".into());
        }
        if chunk["done"].as_bool() == Some(true) {
            break;
        }
    }
    Ok(text)
}
pub fn response_json(response: Response) -> Result<Value, String> {
    check_status(&response)?;
    let mut body = Vec::new();
    response
        .take(2_000_001)
        .read_to_end(&mut body)
        .map_err(|_| "Provider response interrupted".to_string())?;
    if body.len() > 2_000_000 {
        return Err("Provider response exceeded 2 MB.".into());
    }
    serde_json::from_slice(&body).map_err(|_| "Provider returned invalid JSON.".into())
}
pub fn base(config: &Connection) -> Result<String, String> {
    let value = match config.kind {
        ProviderKind::Openai => "https://api.openai.com/v1",
        ProviderKind::Anthropic => "https://api.anthropic.com/v1",
        ProviderKind::Ollama if config.endpoint.is_empty() => "http://127.0.0.1:11434",
        _ => config.endpoint.trim_end_matches('/'),
    };
    let url =
        reqwest::Url::parse(value).map_err(|_| "Enter a valid provider endpoint.".to_string())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Endpoints cannot contain credentials, query strings or fragments.".into());
    }
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("Use HTTPS, or HTTP on localhost only.".into());
    }
    if config.kind == ProviderKind::Ollama && !local {
        return Err(
            "Local Ollama must use localhost. Use a compatible connection for a remote server."
                .into(),
        );
    }
    Ok(value.into())
}
/// Whether task content would leave this machine. Apple's model is on-device
/// and `fm` exposes no Private Cloud Compute backend, so it counts as local
/// and never triggers the cloud confirmation.
pub fn is_remote(config: &Connection) -> bool {
    !matches!(
        config.kind,
        ProviderKind::None | ProviderKind::Ollama | ProviderKind::Apple
    )
}
/// Lists a provider's model ids without running a completion probe, so the
/// picker can populate before the user commits to anything. Model-listing
/// endpoints are free; "test connection" is the part that spends tokens.
pub fn list_models(config: &Connection) -> Result<Vec<String>, String> {
    // The on-device model is the only one `fm` serves, so the picker can be
    // populated without starting the server.
    if config.kind == ProviderKind::Apple {
        return Ok(vec![apple::MODEL.to_string()]);
    }
    let endpoint = base(config)?;
    let http = client()?;
    let value = match config.kind {
        ProviderKind::Ollama => response_json(
            http.get(format!("{endpoint}/api/tags"))
                .send()
                .map_err(|_| "Ollama is not reachable. Start Ollama first.".to_string())?,
        )?,
        ProviderKind::Openai => response_json(
            http.get(format!("{endpoint}/models"))
                .bearer_auth(secret("openai")?)
                .send()
                .map_err(|_| "Could not reach OpenAI. Check your connection.".to_string())?,
        )?,
        ProviderKind::Anthropic | ProviderKind::Claude => response_json(
            http.get(format!("{endpoint}/models"))
                .header("x-api-key", secret("anthropic")?)
                .header("anthropic-version", "2023-06-01")
                .send()
                .map_err(|_| "Could not reach Anthropic. Check your connection.".to_string())?,
        )?,
        ProviderKind::Compatible => {
            let mut request = http.get(format!("{endpoint}/models"));
            if let Ok(key) = secret("compatible") {
                request = request.bearer_auth(key);
            }
            response_json(
                request
                    .send()
                    .map_err(|_| "Could not reach the endpoint. Check the URL.".to_string())?,
            )?
        }
        _ => return Err("Choose a provider first.".into()),
    };
    Ok(parse_model_ids(&value))
}
/// Accepts both listing shapes: OpenAI-compatible `{"data":[{"id"}]}` and
/// Ollama's `{"models":[{"name"}]}`.
pub fn parse_model_ids(value: &Value) -> Vec<String> {
    value["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str())
        .chain(
            value["models"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| m["name"].as_str()),
        )
        .map(str::to_owned)
        .collect()
}
/// Whether an Ollama server answers at this connection's endpoint. Short
/// timeout: detection must never make the panel feel slow.
pub fn ollama_reachable(config: &Connection) -> bool {
    let Ok(endpoint) = base(config) else {
        return false;
    };
    let Ok(http) = Client::builder()
        .timeout(Duration::from_secs(2))
        .connect_timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return false;
    };
    http.get(format!("{endpoint}/api/tags"))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
/// First match for `name` on PATH, so detection reports what a shell would run.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    if name.trim().is_empty() {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        if candidate.with_extension("exe").is_file() {
            return Some(candidate.with_extension("exe"));
        }
    }
    None
}
fn has_secret(name: &str) -> bool {
    keyring::Entry::new(SECRET_SERVICE, name)
        .and_then(|e| e.get_password())
        .is_ok()
}
/// The added provider of `kind`, else a connection of it anywhere in the
/// config, so detection reflects what a task could actually run on.
fn configured(config: &Connections, kind: ProviderKind) -> Option<Connection> {
    if let Some(setup) = config.providers.get(&kind) {
        return Some(Connection {
            kind,
            model: setup.model.clone(),
            endpoint: setup.endpoint.clone(),
            executable: setup.executable.clone(),
        });
    }
    if config.active.kind == kind {
        return Some(config.active.clone());
    }
    config.per_pet.values().find(|c| c.kind == kind).cloned()
}
pub fn detect(config: &Connections) -> ProviderDetection {
    let apple = apple::readiness();
    let codex = configured(config, ProviderKind::Codex)
        .map(|c| c.executable)
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "codex".into());
    let ollama = configured(config, ProviderKind::Ollama).unwrap_or_else(|| Connection {
        kind: ProviderKind::Ollama,
        ..Default::default()
    });
    ProviderDetection {
        codex_installed: find_in_path(&codex).is_some(),
        claude_code: super::runtimes::CLAUDE_CODE_LOGIN && super::runtimes::bridge_ready(),
        ollama_app: find_in_path("ollama").is_some(),
        ollama_reachable: ollama_reachable(&ollama),
        openai_key: has_secret("openai"),
        anthropic_key: has_secret("anthropic"),
        compatible_key: has_secret("compatible"),
        brave_key: has_secret("brave"),
        apple_supported: apple::supported(),
        apple_ready: apple.is_ready(),
        apple_detail: apple.detail(),
    }
}
pub struct HttpModel(pub Connection);
impl HttpModel {
    /// A completion; `format` is a `response_format`, sent only to the
    /// on-device model, the one provider here whose output is constrained.
    fn request(
        &self,
        messages: &[Message],
        format: Option<Value>,
        cancel: &Cancel,
    ) -> Result<String, String> {
        cancel.check()?;
        let config = &self.0;
        // Apple's endpoint is a server Ledgeling starts on demand, not a saved
        // address, and `system` is the only model it accepts.
        let endpoint = if config.kind == ProviderKind::Apple {
            apple::endpoint(cancel)?
        } else {
            base(config)?
        };
        if config.model.trim().is_empty() && config.kind != ProviderKind::Apple {
            return Err(
                "Choose a default model for this provider on the AI providers page.".into(),
            );
        }
        let http = client()?;
        if config.kind == ProviderKind::Ollama {
            let show = http
                .post(format!("{endpoint}/api/show"))
                .json(&json!({"model":config.model}));
            let model_info = cancellable(cancel, move || {
                response_json(
                    show.send()
                        .map_err(|_| "Could not inspect the local Ollama model".to_string())?,
                )
            })?;
            if config.model.contains(":cloud")
                || model_info.get("remote_host").is_some_and(|v| !v.is_null())
                || model_info.get("remote_model").is_some_and(|v| !v.is_null())
            {
                return Err("This is an Ollama cloud model. The local connection only runs installed local models; choose a local model.".into());
            }
            cancel.check()?;
        }
        let request = match config.kind {
            ProviderKind::Ollama => http.post(format!("{endpoint}/api/chat")).json(&json!({"model":config.model,"messages":messages,"stream":true,"format":"json","options":{"num_predict":4096},"keep_alive":"1m"})),
            ProviderKind::Openai => http.post(format!("{endpoint}/responses")).bearer_auth(secret("openai")?).json(&json!({"model":config.model,"input":messages,"max_output_tokens":4096,"store":false})),
            ProviderKind::Anthropic => {
                let system = messages.iter().filter(|m| m.role=="system").map(|m|m.content.as_str()).collect::<Vec<_>>().join("\n");
                let input: Vec<_> = messages.iter().filter(|m|m.role!="system").collect();
                http.post(format!("{endpoint}/messages")).header("x-api-key",secret("anthropic")?).header("anthropic-version","2023-06-01").json(&json!({"model":config.model,"system":system,"messages":input,"max_tokens":4096}))
            }
            ProviderKind::Compatible => {
                let mut r = http.post(format!("{endpoint}/chat/completions")).json(&json!({"model":config.model,"messages":messages,"stream":false,"max_tokens":4096}));
                if let Ok(key) = secret("compatible") { r = r.bearer_auth(key); } r
            }
            // Local server, no credential: `fm serve` speaks Chat Completions
            // and binds to loopback only.
            ProviderKind::Apple => {
                // Bounded so a model that copies a listing out cannot take a minute.
                let mut body = json!({"model":apple::MODEL,"messages":messages,"stream":false,"max_tokens":600});
                if let Some(format) = format {
                    body["response_format"] = format;
                }
                http.post(format!("{endpoint}/chat/completions")).json(&body)
            }
            _ => return Err("Choose an AI model connection, or use a built-in preset without AI.".into()),
        };
        let streamed = config.kind == ProviderKind::Ollama;
        let stop = cancel.clone();
        let v = cancellable(cancel, move || {
            let response = request.send().map_err(|_| "Could not reach the provider (90-second timeout). Check the endpoint and connection.".to_string())?;
            if streamed {
                check_status(&response)?;
                read_stream(response, &stop).map(|text| json!({"message":{"content":text}}))
            } else {
                response_json(response)
            }
        })?;
        cancel.check()?;
        let output = match config.kind {
            ProviderKind::Ollama => v["message"]["content"].as_str().unwrap_or("").to_owned(),
            ProviderKind::Openai => v["output"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|item| item["content"].as_array().into_iter().flatten())
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            ProviderKind::Anthropic => v["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => v["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
        };
        if output.is_empty() {
            Err("The model returned no text. Check model compatibility and output limits.".into())
        } else {
            Ok(output)
        }
    }
}
impl ModelProvider for HttpModel {
    fn complete(&self, messages: &[Message], cancel: &Cancel) -> Result<String, String> {
        self.request(messages, None, cancel)
    }
    // The on-device model is small enough to drift off the JSON envelope, but
    // `fm serve` supports guided generation, so its replies are made valid by
    // construction instead of being retried.
    fn act(&self, messages: &[Message], tools: &[&str], cancel: &Cancel) -> Result<String, String> {
        if self.0.kind == ProviderKind::Apple {
            self.request(messages, Some(apple::action_format(tools)), cancel)
        } else {
            self.complete(messages, cancel)
        }
    }
    // Measured on the real model: exploring with tools answered 1 of 20
    // folder questions, one look at a gathered overview 15 of 16.
    fn single_turn(&self) -> bool {
        self.0.kind == ProviderKind::Apple
    }
    fn capabilities(&self, cancel: &Cancel) -> Result<Capabilities, String> {
        if self.0.kind == ProviderKind::Apple {
            let ready = apple::readiness();
            if !ready.is_ready() {
                return Err(ready.detail());
            }
        }
        let models = if self.0.kind == ProviderKind::Ollama {
            list_models(&self.0)?
        } else if self.0.kind == ProviderKind::Apple {
            vec![apple::MODEL.to_string()]
        } else {
            vec![self.0.model.clone()]
        };
        if self.0.model.is_empty() && self.0.kind != ProviderKind::Apple {
            return Ok(Capabilities {
                models,
                structured: false,
                advanced: false,
                detail: "Choose a model, then test again to verify structured task responses."
                    .into(),
            });
        }
        // Small local models drift off the envelope often enough that one miss
        // is not evidence the model is unusable — the task loop retries too.
        // Cloud attempts cost money, so only free local ones are repeated.
        let attempts = if matches!(self.0.kind, ProviderKind::Apple | ProviderKind::Ollama) {
            3
        } else {
            1
        };
        let mut valid = false;
        for _ in 0..attempts {
            let response = self.act(
                &[
                    Message {
                        role: "system".into(),
                        content:
                            "Return only JSON: {\"tool\":\"finish\",\"args\":{\"text\":\"ready\"}}"
                                .into(),
                    },
                    Message {
                        role: "user".into(),
                        content: "Check task capability.".into(),
                    },
                ],
                &["finish"],
                cancel,
            )?;
            valid = parse_call(&response)
                .is_ok_and(|c| c.tool == "finish" && c.args["text"].is_string());
            if valid {
                break;
            }
        }
        let detail = match (valid, self.0.kind == ProviderKind::Apple) {
            (true, _) => "Connected. Structured task response verified.",
            // There is no other on-device model to move to, so the advice has
            // to be something the user can actually act on.
            (false, true) => "Connected, but the on-device model did not return a usable task action in three attempts. Try again; if it keeps failing, use another connection for tasks.",
            (false, false) => "Connected, but this model failed the structured task response check. Choose another model.",
        };
        Ok(Capabilities {
            models,
            structured: valid,
            advanced: false,
            detail: detail.into(),
        })
    }
}
pub fn parse_call(text: &str) -> Result<ToolCall, String> {
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(text)
        .trim();
    let call: ToolCall = serde_json::from_str(text).map_err(|_| {
        "Model returned an invalid task action; no action was executed.".to_string()
    })?;
    if !call.args.is_object() {
        return Err("Task action arguments must be an object.".into());
    }
    Ok(call)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Instant,
    };

    fn read_request(stream: &mut TcpStream) -> String {
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                break;
            }
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head).into_owned();
        let length = head
            .lines()
            .find_map(|l| {
                l.to_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap_or(0))
            })
            .unwrap_or(0usize);
        let mut body = vec![0u8; length];
        let _ = stream.read_exact(&mut body);
        head
    }
    /// A stand-in Ollama that streams one token every 50 ms for as long as
    /// the client stays connected, and records when the client went away.
    fn slow_ollama() -> (String, Arc<AtomicBool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let dropped = Arc::new(AtomicBool::new(false));
        let seen = dropped.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let head = read_request(&mut stream);
                if head.starts_with("POST /api/show") {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    );
                    continue;
                }
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n");
                loop {
                    std::thread::sleep(Duration::from_millis(50));
                    let line = b"{\"message\":{\"content\":\"a\"},\"done\":false}\n";
                    if stream.write_all(line).and_then(|_| stream.flush()).is_err() {
                        seen.store(true, Ordering::SeqCst);
                        return;
                    }
                }
            }
        });
        (address, dropped)
    }
    fn ollama(endpoint: &str) -> HttpModel {
        HttpModel(Connection {
            kind: ProviderKind::Ollama,
            model: "test".into(),
            endpoint: endpoint.into(),
            ..Default::default()
        })
    }
    fn message() -> Vec<Message> {
        vec![Message {
            role: "user".into(),
            content: "hi".into(),
        }]
    }

    #[test]
    fn cancelling_stops_a_streaming_reply_at_once_and_closes_the_connection() {
        let (endpoint, dropped) = slow_ollama();
        let cancel = Cancel::default();
        let worker = cancel.clone();
        let model = ollama(&endpoint);
        let started = Instant::now();
        let handle = std::thread::spawn(move || model.complete(&message(), &worker));
        std::thread::sleep(Duration::from_millis(400));
        cancel.cancel();
        let result = handle.join().unwrap();
        assert_eq!(result, Err("Task cancelled".into()));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "cancel took {:?}",
            started.elapsed()
        );
        for _ in 0..40 {
            if dropped.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the server was still being read after the task was cancelled");
    }
    #[test]
    fn a_call_that_never_returns_cannot_hold_the_worker_after_a_cancel() {
        let cancel = Cancel::default();
        let stop = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            stop.cancel();
        });
        let started = Instant::now();
        let result: Result<(), String> = cancellable(&cancel, || {
            std::thread::sleep(Duration::from_secs(20));
            Ok(())
        });
        assert_eq!(result, Err("Task cancelled".into()));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn a_finished_call_returns_its_value() {
        assert_eq!(cancellable(&Cancel::default(), || Ok(7)), Ok(7));
    }
    #[test]
    fn streamed_replies_are_joined_and_errors_surface() {
        let cancel = Cancel::default();
        let body = "{\"message\":{\"content\":\"{\\\"to\"},\"done\":false}\n\n{\"message\":{\"content\":\"ol\\\":1}\"},\"done\":true}\nignored";
        assert_eq!(
            read_stream(body.as_bytes(), &cancel).unwrap(),
            "{\"tool\":1}"
        );
        let failed = "{\"error\":\"model not found\"}\n";
        assert!(read_stream(failed.as_bytes(), &cancel)
            .unwrap_err()
            .contains("model not found"));
        cancel.cancel();
        assert_eq!(
            read_stream(body.as_bytes(), &cancel),
            Err("Task cancelled".into())
        );
    }
    #[test]
    fn model_id_parsing_covers_both_listing_shapes() {
        assert_eq!(
            parse_model_ids(&json!({"data":[{"id":"a"},{"id":"b"}]})),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            parse_model_ids(&json!({"models":[{"name":"llama"}]})),
            vec!["llama".to_string()]
        );
        assert!(parse_model_ids(&json!({})).is_empty());
    }
    #[test]
    #[cfg(unix)]
    fn path_probe_finds_a_standard_binary_and_ignores_fiction() {
        assert!(find_in_path("sh").is_some());
        assert!(find_in_path("definitely-not-a-perch-binary").is_none());
        assert!(find_in_path("  ").is_none());
    }
    #[test]
    fn rejects_invalid_actions_and_insecure_endpoints() {
        assert!(parse_call("Run rm -rf").is_err());
        assert!(parse_call(r#"{"tool":"finish","args":[],"extra":true}"#).is_err());
        let c = Connection {
            kind: ProviderKind::Compatible,
            endpoint: "http://example.com".into(),
            ..Default::default()
        };
        assert!(base(&c).is_err());
        assert!(base(&Connection {
            endpoint: "https://user:secret@example.com".into(),
            ..c
        })
        .is_err());
    }
    #[test]
    #[ignore = "Requires locally running Ollama and PERCH_TEST_MODEL"]
    fn live_ollama() {
        let p = HttpModel(Connection {
            kind: ProviderKind::Ollama,
            model: std::env::var("PERCH_TEST_MODEL").expect("PERCH_TEST_MODEL"),
            ..Default::default()
        });
        assert!(p.capabilities(&Cancel::default()).unwrap().structured);
    }
}
