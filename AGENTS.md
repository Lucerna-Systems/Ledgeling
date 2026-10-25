# Repository Guidelines

## Project Structure & Module Organization

The directory is `lupetty`; the product is **Ledgeling**. M0-M4 are implemented as far
as code goes; `docs/verification.md` records what has actually been run and what
has not. `planning_files/00-PLAN.md` is the consolidated plan and the authority
on intent; `planning_files/01-RELEASE-PLAN.md` tracks active release work packages.

- `src-tauri/src/world/` surface extraction, the traversal graph, scan diffing.
- `src-tauri/src/pet/` pet runtime, locomotion classes, pathfinding, behavior FSM.
- `src-tauri/src/packs/` manifest reading, validation, condition grammar, install.
- `src-tauri/src/awareness/` idle, session, categories, nudges.
- `src-tauri/src/platform/` all OS calls behind one trait (macOS, Windows, mock).
- `src-tauri/src/engine.rs` headless core; `src-tauri/src/app.rs` Tauri wiring only.
- `src/` companion UI and overlay renderer; `packs/` bundled packs; `schema/` the
  published JSON Schema.

Keep authoritative pet and world state in Rust. The webview renders and forwards
pointer events; it never decides where a pet is.

## Build, Test, and Development Commands

The Vue 3 / TypeScript / Tailwind 4 frontend and Tauri 2 / Rust backend are scaffolded:

- `npm install`: install frontend dependencies.
- `npm run dev`: run the frontend development server.
- `npm run tauri dev`: launch the desktop app.
- `npm run build`: type-check and build the frontend (`vue-tsc --noEmit && vite build`).
- `npm test`: run the Rust test suite (`cargo test --manifest-path src-tauri/Cargo.toml --features coding`, 149 tests).
- `npm run test:ui`: run the 62 UI tests via Vitest (`vitest run`).
- `npm run check`: frontend build plus Rust Clippy (default and `--features coding`) with warnings denied.
- `npm run tauri build`: build desktop bundles.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run default release Rust tests without the `coding` feature.

## Coding Style & Naming Conventions

Use two-space indentation in Vue, TypeScript, and JSON; use `rustfmt` defaults for Rust. Prefer Vue Composition API with `<script setup lang="ts">`, PascalCase component filenames, camelCase TypeScript functions, and snake_case Rust modules. Use Tailwind utilities for UI styling. Run `cargo fmt --manifest-path src-tauri/Cargo.toml` and `npm run check` (frontend type-check/build plus Clippy with warnings denied).

## Testing Guidelines

149 Rust tests: 132 unit tests beside the code, 10 in `src-tauri/tests/timeline.rs`
for window-layout replays through the mock platform, and 7 in `src-tauri/tests/privacy.rs`,
which fails the build if a window-title, screen-capture, key-state or unisolated network API
appears anywhere in the source. 62 UI tests in Vitest (`npm run test:ui`) cover
dashboard, assistant, approvals, and bubble logic with colocated `*.test.ts` files.
Add tests for behavior changes — surface geometry, traversal rules, pack validation
messages — and prefer driving `engine::Engine` with `platform::mock::MockPlatform`
over anything needing a GUI.

## Commit & Pull Request Guidelines

Never commit to `main` (testing, publishes pre-alpha) or `prod` (releases, publishes alpha). Work on a branch in its own worktree off `origin/main` and open a PR into `main`; see `docs/development.md`. Use concise imperative commits, optionally prefixed with `feat:`, `fix:`, or `docs:`. PRs should identify the milestone or issue, explain behavior changes, list validation results, and include screenshots for UI changes.

## Agent-Specific Instructions

Before code discovery, use `memory-mcp`: derive repo `lupetty` and the current Git branch; pass branch explicitly where accepted. Call `search_memory`, then `search_code` with `includeText: true`. Read returned ranges with small surrounding context; use `get_entity_graph` for relationships. Incrementally `index_codebase` if results are empty or stale before shell fallback. If unavailable, explicitly announce the exception before manual search.

Write only durable findings through `ingest_note` with repo, branch, and `source: "codex"`. Promote selectively at merge; review stale scopes before pruning.

## Boundaries to keep honest

macOS is the only platform that has been run. The Windows adapter is written to
the documented Win32 APIs but has never been compiled. Nothing is signed, the
performance budget is not re-certified for the current build, the soak has not
been run, and no release exists. Do not describe any of those as done; update
`docs/verification.md` when one of them changes.
