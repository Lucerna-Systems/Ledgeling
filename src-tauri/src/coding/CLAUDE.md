# Coding Submodule (`src-tauri/src/coding/`)

## Purpose & Scope
The `coding` module powers Ledgeling's Coding Mode: git worktree management, dev server runners, Monaco file access, terminal split layouts, and GitHub integration.

## Modular Structure
- `commands/`:
  - `projects.rs`: Project registration, worktree creation/removal, and pet binding.
  - `fs_ops.rs`: Sandboxed Monaco file reading/writing with strict root boundary checks.
  - `process.rs`: Dev server launcher, port detection, tools installer, and window pinning.
  - `mod.rs`: IPC router and attention coordination loop.
- `github/`:
  - `models.rs`: Typed PR, Issue, Board, and Card domain models (no subprocess logic).
  - `client.rs`: `gh` CLI runner, sandboxed execution, and error message translation.
  - `board.rs`: GitHub Projects v2 GraphQL querying and column card moves.
  - `mod.rs`: Public API methods and tests.
- `git.rs`: Pure domain helpers for git status, worktrees, and branch operations.
- `store.rs`: Persistent metadata store for projects and worktrees.
- `mod.rs`: Implements `SubsystemService` (`shutdown(&mut self)`) to cleanly terminate active watchers, servers, and processes.

## Constraints
- **Subprocess Isolation**: Zero raw CLI arguments exposed to IPC; all parameters travel as typed checked arguments or via stdin.
- **Sandboxed File Access**: `inside(root, relative)` ensures Monaco requests never escape the worktree root.
