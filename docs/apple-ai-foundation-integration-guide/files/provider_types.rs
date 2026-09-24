//! Assumed shapes for your existing provider system. Swap these for your
//! real `Message` / `ToolSpec` / `AiProvider` types — this file exists so
//! `apple_foundation.rs` compiles standalone and shows what it expects.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    /// Result of a tool call, fed back into the conversation.
    Tool { tool_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for this tool's arguments object.
    pub parameters_schema: serde_json::Value,
}

/// What a provider hands back for one turn when tools are in play.
#[derive(Debug, Clone)]
pub enum AgentStep {
    FinalAnswer(String),
    ToolCall { name: String, arguments: serde_json::Value },
}

#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &'static str;
    /// Cheap, synchronous readiness check for showing/hiding this provider in the UI.
    fn is_available(&self) -> bool;
    async fn chat(&self, messages: &[Message]) -> anyhow::Result<String>;
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> anyhow::Result<AgentStep>;
}
