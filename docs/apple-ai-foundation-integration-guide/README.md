# Apple Foundation Models — what was actually built

The guide in `files/` was written against assumed types and unshipped-docs
guesses about the `fm` CLI. It has been superseded by a real implementation in
`src-tauri/src/tasks/apple.rs`. This file records where the guide was wrong, so
its suggestions are not re-applied later.

`files/` is kept only as the original input. Nothing in the codebase references
it.

## Corrections, checked against `/usr/bin/fm` on macOS 27.0 (26A5425a)

| Guide assumed | Actually true |
|---|---|
| `--pcc` selects Private Cloud Compute | No such flag. `--model` takes `system` — "the default and the only supported value". There is **no PCC backend in this build**, so the whole `Backend` enum and the quota/per-pet-backend UX it implied are moot. |
| No cheap way to check readiness up front | `fm license --status` and `fm available --model system` both answer without prompting. Readiness is a real check, not a first-use error. |
| `--schema` takes a file path, so write a temp file | Half right: `fm respond --schema <file>` really is path-only. The inline form belongs to the separate `fm schema` builder subcommand. Moot either way — `fm serve` is used instead. |
| Map onto a one-shot `AiProvider::chat` | `fm serve` exposes `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` over loopback TCP or a Unix socket. |
| Nothing about licensing | **Every** `fm` subcommand, including `--help`, refuses until `sudo fm license` is accepted machine-wide. This is the single biggest setup blocker and needs its own UI state. |
| `chat_with_tools` returns a native tool call | `fm` prints "Tool calls are not supported in fm" and "No built-in tools are available in this build". The JSON-envelope approach is correct — and Perch already had one. |

The guide's assumed `AiProvider` trait (async, `AgentStep`, `async-trait`,
`once_cell`, `tokio`) matches nothing in this codebase. The real trait is
`ModelProvider` — synchronous, `complete(&[Message], &Cancel) -> Result<String,
String>` plus `capabilities`. No new dependencies were needed.

## Why `fm serve` rather than `fm respond`

`Context::model_task` calls `complete()` up to 24 times per task with a growing
`messages` array of system/user/assistant turns. `fm respond` is one-shot and
takes a flat prompt, so every step would re-flatten the whole transcript — the
lossy thing the guide proposed, and worse on a small on-device model. `fm serve`
speaks the Chat Completions shape `HttpModel` already sends, so the adapter owns
only the child process and the existing request path is reused unchanged.

The cost is a process to supervise: it is started on demand, health-checked
before use, restarted if it dies, stopped after five idle minutes, and killed on
app exit. That matters because the server holds the model resident.

## The envelope drifts, and that is designed around

Measured against the live model with Perch's real system prompt: roughly one
reply in five is not valid JSON — a misplaced quote (`"webAllowed:false"`) or a
dropped brace. `model_task` previously treated a parse failure as fatal, which
across up to 24 steps meant a task would almost never finish. It now feeds the
parse error back and retries, failing only after three unreadable replies in a
row. The capability probe retries the same way for local providers, because a
single drift is not evidence the model is unusable.

Tool *selection* is separately mediocre: "Organise the invoices" and "Rename the
photos" both chose `duplicates`. That is model quality, not parsing — wrong
tools come back as `toolError` and the loop can recover — but it is why the
on-device model is worth offering as an option rather than a default.

## What has been verified

`sudo fm license` is accepted on this machine, and `apple_foundation` in
`tests/task_connections.rs` passes: server start, health check, a real
completion and a parsed envelope, in under 3 seconds. See
`docs/verification.md` for what remains untested.

```
cargo test --test task_connections apple_foundation -- --ignored --nocapture
```
