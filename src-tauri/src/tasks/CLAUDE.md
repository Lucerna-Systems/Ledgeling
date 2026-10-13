# Tasks Submodule (`src-tauri/src/tasks/`)

## Purpose & Scope
The `tasks` module manages the AI Assistant: background workers, tool execution, multi-provider communication (Apple Foundation, OpenAI, Anthropic, Ollama, Codex, Claude CLI), approvals, and file modification diffs.

## Key Submodules
- `commands.rs`: Thin Tauri IPC command handlers for tasks, conversations, approvals, and connections.
- `providers.rs`: Provider implementations (HTTP adapters, streaming SSE, prompt framing).
- `store.rs`: SQLite persistence for conversation turns, operations, and config.
- `summary.rs`: Human-readable summaries of tool executions and file operations.
- `mod.rs`: `TaskService` implementation and worker dispatch thread. Implements `SubsystemService`.

## Constraints & Privacy Gates
- **Network Confinement**: Network calls in the entire project are strictly confined to `tasks/providers.rs`, `tasks/search.rs`, `tasks/commands.rs`, and `tasks/apple.rs`. Enforced by `tests/privacy.rs`.
- **Approval Gate**: All destructive operations (writing files, running commands) require explicit approval steps before execution.
