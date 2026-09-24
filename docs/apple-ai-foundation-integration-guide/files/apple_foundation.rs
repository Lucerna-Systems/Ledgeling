//! AiProvider implementation backed by Apple's on-device Foundation Model,
//! reached via the `fm` CLI shipped with macOS 27 "Golden Gate".
//!
//! Design notes:
//! - `fm respond` is a one-shot, flat-string-in / string-out call. There's
//!   no messages array and no native tool-calling API. So instead of
//!   mapping straight onto your OpenAI/Anthropic adapters, this renders
//!   the whole transcript + tool definitions into one prompt and forces
//!   the reply into a small JSON envelope via `--schema`, then Rust parses
//!   that envelope and either returns the final answer or a tool call for
//!   your existing tool executor to run. This is the same trick used for
//!   local models (Ollama/llama.cpp) that lack native function calling.
//! - Beta caveat: `--pcc` / `--schema` flag names are from WWDC26 session
//!   coverage, not shipped docs. Verify with `fm --help` on-device.
//! - Concurrency: there is exactly one on-device model shared by every
//!   pet. A semaphore below serializes calls so five pets thinking at
//!   once doesn't mean five concurrent `fm` processes fighting over the
//!   Neural Engine. Raise `MAX_CONCURRENT_CALLS` if you find it's overly
//!   conservative once you can measure real latency.

use crate::provider_types::{AgentStep, AiProvider, Message, Role, ToolSpec};
use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use tokio::sync::Semaphore;

const MAX_CONCURRENT_CALLS: usize = 1;
static MODEL_LOCK: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(MAX_CONCURRENT_CALLS));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// On-device, free, unlimited. Default.
    System,
    /// Larger model on Private Cloud Compute. Subject to daily quota.
    PrivateCloudCompute,
}

pub struct AppleFoundationProvider {
    backend: Backend,
}

impl AppleFoundationProvider {
    /// Returns `Some` only if this Mac can plausibly run the model:
    /// macOS 27+, Apple Silicon, and the `fm` binary present. This does
    /// NOT confirm Apple Intelligence is turned on or the model weights
    /// are downloaded — that failure mode is surfaced as a normal error
    /// from the first real call, since there's no cheap way to check it
    /// up front.
    pub fn detect(backend: Backend) -> Option<Self> {
        if !cfg!(target_os = "macos") {
            return None;
        }
        if !is_apple_silicon() {
            return None;
        }
        if !macos_version_at_least(27, 0) {
            return None;
        }
        if which_fm().is_none() {
            return None;
        }
        Some(Self { backend })
    }
}

#[async_trait::async_trait]
impl AiProvider for AppleFoundationProvider {
    fn id(&self) -> &'static str {
        "apple-foundation-models"
    }

    fn is_available(&self) -> bool {
        which_fm().is_some()
    }

    async fn chat(&self, messages: &[Message]) -> Result<String> {
        let prompt = render_transcript(messages, &[]);
        let backend = self.backend;
        run_locked(move || fm_respond(&prompt, backend, None)).await
    }

    async fn chat_with_tools(&self, messages: &[Message], tools: &[ToolSpec]) -> Result<AgentStep> {
        let prompt = render_transcript(messages, tools);
        let schema = agent_step_schema(tools);
        let backend = self.backend;

        let raw = run_locked(move || fm_respond_structured(&prompt, &schema, backend)).await?;
        parse_agent_step(&raw)
    }
}

/// Runs a blocking `fm` call on a blocking thread, holding the shared
/// model semaphore for the duration so concurrent pets queue instead of
/// racing.
async fn run_locked<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let _permit = MODEL_LOCK.acquire().await.expect("semaphore not closed");
    tokio::task::spawn_blocking(f).await.context("fm task panicked")?
}

fn fm_respond(prompt: &str, backend: Backend, image: Option<&std::path::Path>) -> Result<String> {
    let mut cmd = Command::new("fm");
    cmd.arg("respond").arg(prompt);
    if backend == Backend::PrivateCloudCompute {
        cmd.arg("--pcc");
    }
    if let Some(path) = image {
        cmd.arg("--image").arg(path);
    }
    let output = cmd.output().context("failed to spawn `fm respond`")?;
    if !output.status.success() {
        bail!(
            "fm respond failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn fm_respond_structured(prompt: &str, schema_json: &str, backend: Backend) -> Result<String> {
    // fm respond --schema expects a path in the beta CLI, so we write the
    // schema to a temp file rather than assuming an inline-JSON flag exists.
    let schema_path = write_temp_schema(schema_json)?;
    let mut cmd = Command::new("fm");
    cmd.arg("respond").arg(prompt).arg("--schema").arg(&schema_path);
    if backend == Backend::PrivateCloudCompute {
        cmd.arg("--pcc");
    }
    let output = cmd.output().context("failed to spawn `fm respond --schema`")?;
    let _ = std::fs::remove_file(&schema_path);
    if !output.status.success() {
        bail!(
            "fm respond --schema failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write_temp_schema(schema_json: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("pet-app-fm-schema-{}.json", uuid_like()));
    std::fs::write(&path, schema_json)?;
    Ok(path)
}

fn uuid_like() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

/// Flattens a message history (plus optional tool definitions) into a
/// single prompt. Tune the wording here freely — this is the one place
/// where prompt quality directly drives whether the on-device model
/// picks tools sanely.
fn render_transcript(messages: &[Message], tools: &[ToolSpec]) -> String {
    let mut out = String::new();

    if !tools.is_empty() {
        out.push_str("You can either answer directly or call one tool.\nAvailable tools:\n");
        for t in tools {
            out.push_str(&format!(
                "- {}: {} | arguments schema: {}\n",
                t.name, t.description, t.parameters_schema
            ));
        }
        out.push_str(
            "Respond ONLY with JSON matching the required schema: either \
             {\"type\":\"final_answer\",\"content\":\"...\"} or \
             {\"type\":\"tool_call\",\"name\":\"...\",\"arguments\":{...}}.\n\n",
        );
    }

    for m in messages {
        let label = match &m.role {
            Role::System => "System".to_string(),
            Role::User => "User".to_string(),
            Role::Assistant => "Assistant".to_string(),
            Role::Tool { tool_name } => format!("Tool[{tool_name}] result"),
        };
        out.push_str(&format!("{label}: {}\n", m.content));
    }
    out.push_str("Assistant:");
    out
}

/// JSON Schema for the tagged-union envelope the model must reply with
/// when tools are available.
fn agent_step_schema(_tools: &[ToolSpec]) -> String {
    let schema = json!({
        "type": "object",
        "oneOf": [
            {
                "properties": {
                    "type": { "const": "final_answer" },
                    "content": { "type": "string" }
                },
                "required": ["type", "content"]
            },
            {
                "properties": {
                    "type": { "const": "tool_call" },
                    "name": { "type": "string" },
                    "arguments": { "type": "object" }
                },
                "required": ["type", "name", "arguments"]
            }
        ]
    });
    schema.to_string()
}

fn parse_agent_step(raw: &str) -> Result<AgentStep> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .with_context(|| format!("model reply wasn't valid JSON: {raw}"))?;
    match value.get("type").and_then(|v| v.as_str()) {
        Some("final_answer") => {
            let content = value["content"].as_str().unwrap_or_default().to_string();
            Ok(AgentStep::FinalAnswer(content))
        }
        Some("tool_call") => {
            let name = value["name"].as_str().unwrap_or_default().to_string();
            let arguments = value["arguments"].clone();
            Ok(AgentStep::ToolCall { name, arguments })
        }
        other => bail!("unexpected envelope type from model: {other:?} (raw: {raw})"),
    }
}

fn which_fm() -> Option<PathBuf> {
    let output = Command::new("which").arg("fm").output().ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn is_apple_silicon() -> bool {
    std::env::consts::ARCH == "aarch64"
}

fn macos_version_at_least(major: u32, minor: u32) -> bool {
    let output = match Command::new("sw_vers").arg("-productVersion").output() {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    let version = String::from_utf8_lossy(&output.stdout);
    let mut parts = version.trim().split('.');
    let got_major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let got_minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (got_major, got_minor) >= (major, minor)
}
