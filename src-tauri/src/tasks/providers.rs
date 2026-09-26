use super::{apple, types::*, Cancel};
use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};
use std::{io::Read, path::PathBuf, time::Duration};

pub trait ModelProvider {
    fn complete(&self, messages: &[Message], cancel: &Cancel) -> Result<String, String>;
    fn capabilities(&self, cancel: &Cancel) -> Result<Capabilities, String>;
}
pub fn secret_name(kind: &ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Openai => "openai",
        ProviderKind::Anthropic | ProviderKind::Claude => "anthropic",
        ProviderKind::Compatible => "compatible",
        _ => "unused",
    }
}
pub fn set_secret(name: &str, value: &str) -> Result<(), String> {
    if !matches!(name, "openai" | "anthropic" | "compatible" | "brave") {
        return Err("Unknown credential.".into());
    }
    let entry = keyring::Entry::new("com.perch.desktop.tasks", name)
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
    keyring::Entry::new("com.perch.desktop.tasks", name)
        .and_then(|e| e.get_password())
        .map_err(|_| format!("Add the {name} API key in Connections."))
}
pub fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialise HTTP client".into())
}
pub fn response_json(response: Response) -> Result<Value, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Provider returned HTTP {}. Check your connection, model, credentials and usage limits.", status.as_u16()));
    }
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
    keyring::Entry::new("com.perch.desktop.tasks", name)
        .and_then(|e| e.get_password())
        .is_ok()
}
/// A connection of `kind` anywhere in the config — the default or any per-pet
/// override — so detection reflects what a task could actually run on.
fn configured(config: &Connections, kind: ProviderKind) -> Option<Connection> {
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
impl ModelProvider for HttpModel {
    fn complete(&self, messages: &[Message], cancel: &Cancel) -> Result<String, String> {
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
            return Err("Choose a model in Connections.".into());
        }
        let http = client()?;
        if config.kind == ProviderKind::Ollama {
            let model_info = response_json(
                http.post(format!("{endpoint}/api/show"))
                    .json(&json!({"model":config.model}))
                    .send()
                    .map_err(|_| "Could not inspect the local Ollama model".to_string())?,
            )?;
            if config.model.contains(":cloud")
                || model_info.get("remote_host").is_some_and(|v| !v.is_null())
                || model_info.get("remote_model").is_some_and(|v| !v.is_null())
            {
                return Err("This is an Ollama cloud model. The local connection only runs installed local models; choose a local model.".into());
            }
            cancel.check()?;
        }
        let request = match config.kind {
            ProviderKind::Ollama => http.post(format!("{endpoint}/api/chat")).json(&json!({"model":config.model,"messages":messages,"stream":false,"format":"json","options":{"num_predict":4096},"keep_alive":"1m"})),
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
            ProviderKind::Apple => http.post(format!("{endpoint}/chat/completions")).json(&json!({"model":apple::MODEL,"messages":messages,"stream":false})),
            _ => return Err("Choose an AI model connection, or use a built-in preset without AI.".into()),
        };
        let v = response_json(request.send().map_err(|_| "Could not reach the provider (90-second timeout). Check the endpoint and connection.".to_string())?)?;
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
            let response = self.complete(
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
