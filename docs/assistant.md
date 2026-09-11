# Perch tasks

Perch now has an opt-in task service, independent of pet animation and awareness.
Any pet can present a task. Removing or hiding that pet does not stop the worker.
One task runs at a time, including while awaiting approval; additional tasks queue.

## Start from a pet

On macOS, right-click a desktop pet for a native speech bubble. It takes keyboard
input without activating the dashboard. The dashboard starts hidden; open it
explicitly from the tray menu, Cmd+1 while Perch is active, or the bubble's
Connections button. Pet overlays and the bubble are auxiliary panels across
Spaces; neither can become the application's main window.

Drag a pet into an open Finder folder or onto a Finder folder icon to give that
pet a working folder. The drop uses a native file promise, not window inspection.
Finder briefly receives a uniquely named hidden `.perch-handoff-*` file, removed
one second after completion. No existing file is overwritten. A crash during this
handoff can leave that harmless hidden file behind; it can be removed manually.
The folder binding is session-only, preventing a different pet from inheriting
access after a restart. The chosen folder is visible in the bubble. Choosing a
folder through its ellipsis button is an alternative. Hold Shift while dragging
to move a pet normally without initiating a Finder handoff.

The **Actions / Ask** toggle determines execution:

- **Ask:** a question for the connected model, optionally with web research.
  Nothing on disk is read. The loop follows from the connection — a local or
  API model runs Standard tools; Codex/Claude runtimes run Advanced mode with
  a separate start approval.
- **Actions:** without a connection the adjacent menu chooses the built-in
  offline actions — sort by type, sort by modified month, batch rename, find
  files, or exact duplicates — which need a chosen folder; the typed text is
  the filename prefix for rename or search phrase for find. With any
  connection, local or cloud, the model runs the request itself with file
  tools in the chosen folder — or, when none was chosen, in the Documents/Perch
  folder Perch creates, where it may also add folders. "My instructions" in
  the menu sends free-form instructions. A cloud API model requires the
  content checkbox before a folder is read; an agent's start approval is its
  consent.

The bubble displays progress, task results and approval details. Approve or reject
there; completed file changes offer Undo. Further requests include the previous
result as context. Closing the bubble does not cancel work.

When a model is connected, the bubble also shows a quiet **Select another model**
link. Clicking it fetches the models of this pet's effective provider and swaps
the link for a native picker; a choice is saved immediately as that pet's
override and applies from the next task (a running task keeps the connection it
started with). The web panel's connection state refreshes through the
`connections:changed` event when the bubble changes it.

The dashboard's **Your assistant** panel offers attachments, multiple folders,
full history, source links, connection settings, and the operation journal.

## Connections

Run `npm install` and `npm ci --prefix sidecar`, then `npm run prepare:sidecar`.
The packaging hook copies the build machine's Node executable into the app with
the pinned Claude SDK. Build natively for the target OS/architecture; do not copy
this Mac bundle to Windows. Codex is user-installed and selected by executable
path. The bridge introduces a substantial bundle-size cost; it is loaded only
when needed, not during ordinary pet animation.

- **Ollama:** start a local Ollama service; default endpoint is localhost:11434.
  Models are listed automatically from the running service; Test connection
  verifies structured responses. Remote/cloud model references are rejected in
  local mode.
- **OpenAI:** choose from the models listed for your key. Uses the Responses API
  with `store: false`; provider-side policies still apply.
- **Anthropic:** choose from the models listed for your key. Uses the Messages API.
- **Compatible:** enter an HTTPS base URL ending in the provider's API prefix,
  then pick or type a model ID with an optional key. The adapter uses chat
  completions and verifies JSON task responses with a probe; it does not assume
  all compatibility claims work.
- **ChatGPT / Codex:** detected on your PATH automatically. The panel reports the
  actual ChatGPT sign-in state from `account/read`; the managed browser login and
  Check account are there for when signing in is genuinely outstanding. No token
  extraction is used.
- **Claude SDK:** the runtime is bundled with Perch and detected as installed.
  Uses the saved Anthropic API key. Subscription authentication is deliberately
  not offered because official third-party guidance conflicts.
- **Brave:** enable search and save a Brave Search API key. Search is independent
  of the AI provider. Public HTTPS text/HTML pages can also be read when allowed.

Keys are saved in the OS credential store. Settings contain only connection
metadata. The password field is cleared after saving and saved keys are never
returned to the UI. Normalised network errors do not expose provider response
bodies or credentials. Model calls are limited to 90 seconds and 4,096 output
tokens; Standard tasks stop at 24 steps / 350 KB accumulated context. There is
no silent provider fallback. Tests/probes can incur provider usage.

The Connections panel is split into **Installed apps** (Codex, the bundled Claude
runtime, Ollama) and **API keys** (OpenAI, Anthropic, any OpenAI-compatible
endpoint). Installed apps are detected automatically — PATH probes for Codex and
Ollama, a short reachability check against the Ollama endpoint, a packaged-files
check for the Claude sidecar, and credential-store look-ups for saved API keys —
so each select shows only what this machine can actually run. A saved provider
stays selectable even when detection misses it (a stopped Ollama, a Codex at a
custom path), so detection can never lock a configured connection away.

Model lists load automatically when a provider is selected, through the
provider's free listing endpoint (`/v1/models`, Ollama `/api/tags`, Codex
`model/list`). Listing does not run a completion and does not incur provider
usage; **Test connection** is the action that spends tokens, verifying structured
task responses. The model field also accepts a typed ID that the listing missed,
for compatible endpoints that do not implement `/models` or use custom names.

### Per-companion models

One connection is the default for every pet, and any pet can carry its own
override. A task runs on the pet's override when one is set, otherwise on the
default; mode selection and cloud-consent rules follow that effective
connection. Overrides are stored with the connection metadata in the assistant
database and persist independently of the default — changing the default later
does not re-point pets that have their own model. The assistant panel shows the
chosen companion's effective connection in its badge and request summary, and
Companion models in Connections lists every pet with its effective connection,
where an override can be set or cleared. Overrides for pets that are removed from
the desktop remain stored but are not shown anywhere.

## Execution and recovery

Standard tools list/read/find selected files, detect byte-identical duplicates,
propose moves/new files, search, fetch public pages, or finish with a text answer.
Text, Markdown, CSV and text PDFs are supported; scanned PDFs/OCR are not.
Limits are 2,000 files, 20 directory levels, 10 MB document input, 120 KB extracted
text, 1 GB per hashed file, and 500 changes per model-proposed batch. Built-in sort
presets change only immediate files, preserving existing subfolder structures.

File actions are previewed as an exact batch, revalidated after approval, and
journalled before each operation. Destinations must be new. Moves use hard links
followed by unlinking the original, so cross-volume moves are rejected. SHA-256
checks protect against source changes and undo conflicts. Created empty folders
may remain after undo. Cancellation finishes an already-started file operation
but prevents the next one. An HTTP request may run until its timeout while the
UI displays Cancelling. A restart marks unfinished tasks Interrupted; it never
resumes mutations. Undo checks the journal, including interrupted operations.

Advanced runtimes use their native protocols, sandbox and approval controls.
Codex starts with a read-only sandbox and untrusted approval policy. Claude uses
an isolated settings configuration, a required sandbox and a PreToolUse preview
for every enabled tool. Unsupported permission requests are denied. Arbitrary
commands are previewed as commands, not represented as predictable file diffs;
they have no universal undo. Runtime processes are terminated with their process
group on macOS when cancelled. Advanced work has a 30-minute transport deadline;
Claude also has a 24-turn / $2 per-query cap.

History, selected paths, generated content and journals live in
`<app config>/assistant/tasks.sqlite`, including SQLite WAL files. They are local
and not encrypted by Perch; OS disk protection applies. Do not delete the database
while Perch is running or while you need its undo records.

## Validation

`npm run check`, `npm test`, and `npm run test:ui` cover builds, geometry/awareness,
privacy boundaries, file conflicts, cancellation, restart recovery, queue ordering,
model action validation, and UI approval controls. Scripted model tests run the
same classification plan for every direct-provider kind; they are deterministic
executor tests, not evidence of live provider quality.

Opt-in tests are in `src-tauri/tests/task_connections.rs`; each names its required
credential/model environment. They send synthetic readiness prompts. Run one
explicitly, for example:

```
cargo test --manifest-path src-tauri/Cargo.toml --test task_connections claude_bridge -- --ignored
```

See `docs/verification.md` for checks actually performed. Public release signing,
Windows verification, real multi-monitor validation, and performance recertification
remain separate work. Advanced runtimes and document processing warrant further
adversarial testing before a public release.
