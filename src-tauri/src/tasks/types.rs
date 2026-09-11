use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    None,
    Ollama,
    Openai,
    Anthropic,
    Compatible,
    Codex,
    Claude,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Connection {
    pub kind: ProviderKind,
    pub model: String,
    pub endpoint: String,
    pub executable: String,
}
impl Default for Connection {
    fn default() -> Self {
        Self {
            kind: ProviderKind::None,
            model: String::new(),
            endpoint: String::new(),
            executable: String::new(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Connections {
    pub active: Connection,
    pub search_enabled: bool,
    /// Per-companion overrides; pets without an entry use `active`.
    pub per_pet: BTreeMap<u64, Connection>,
}
/// The connection a task for `pet_id` actually runs on: the pet's override
/// when one is set, otherwise the global default.
pub fn effective_connection(config: &Connections, pet_id: Option<u64>) -> Connection {
    pet_id
        .and_then(|id| config.per_pet.get(&id))
        .cloned()
        .unwrap_or_else(|| config.active.clone())
}
/// What the machine actually has: installed harnesses and saved API keys.
#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderDetection {
    pub codex_installed: bool,
    pub claude_bridge: bool,
    pub ollama_app: bool,
    pub ollama_reachable: bool,
    pub openai_key: bool,
    pub anthropic_key: bool,
    pub compatible_key: bool,
    pub brave_key: bool,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TaskRequest {
    pub prompt: String,
    pub pet_id: Option<u64>,
    pub roots: Vec<String>,
    pub attachments: Vec<String>,
    pub preset: String,
    pub mode: String,
    pub parent_id: Option<i64>,
    pub allow_cloud: bool,
    pub allow_web: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub kind: String,
    pub source: Option<String>,
    pub destination: String,
    pub content: Option<String>,
    pub fingerprint: Option<String>,
    pub result_fingerprint: Option<String>,
    pub state: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub id: u64,
    pub title: String,
    pub details: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub title: String,
    pub url: String,
    pub excerpt: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub request: TaskRequest,
    pub connection: Connection,
    pub status: String,
    pub progress: String,
    pub result: String,
    pub error: Option<String>,
    pub approval: Option<Approval>,
    pub operations: Vec<Operation>,
    pub sources: Vec<Source>,
    pub created_at: i64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub models: Vec<String>,
    pub structured: bool,
    pub advanced: bool,
    pub detail: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}
