# Terminal Submodule (`src-tauri/src/terminal/`)

## Purpose & Scope
The `terminal` module manages real pseudo-terminals (PTYs), shell sessions, and agent hooks for both pet terminal windows and Coding Mode.

## Key Submodules
- `pty.rs`: Cross-platform PTY allocation, process spawning, stdin/stdout piping, and resize events.
- `agent.rs`: Claude and Codex agent protocol tracking, hook parsing, and activity detection.
- `commands.rs`: Tauri IPC wrappers for terminal operations.
- `mod.rs`: `Registry` and `Terminal` structs implementing `SubsystemService` (`shutdown(&mut self)`) to cleanly terminate active sessions.

## Constraints
- **Session Cleanup**: PTY child processes must be explicitly killed on drop or application shutdown.
- **Independence**: Terminal management operates behind its own locks, never blocking the core simulation engine.
