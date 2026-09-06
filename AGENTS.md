# Repository Guidelines

## Project Structure & Module Organization

The directory is `lupetty`; the product is **Perch**. A working macOS M0 MVP now exists. The authoritative roadmap remains in `planning_files/`; implementation and limits are documented in `README.md`.

- Start with `planning_files/00-README.md` for the document map.
- Consult `02-technical-architecture.md` for architecture and `05-user-stories.md` for milestone acceptance criteria.
- The requested stack is Vue 3, TypeScript, Tailwind CSS, and Tauri 2 with Rust. Reserve `src/` for frontend components and assets, and `src-tauri/` for Rust, capabilities, and desktop configuration.
- Keep authoritative pet behavior and world state in Rust; use the frontend for rendering and settings. Isolate OS integration behind platform traits.

## Build, Test, and Development Commands

The Vue 3 / TypeScript / Tailwind 4 frontend and Tauri 2 / Rust backend are scaffolded:

- `npm install`: install frontend dependencies.
- `npm run dev`: run the frontend development server.
- `npm run tauri dev`: launch the desktop app.
- `npm run build`: type-check and build the frontend.
- `npm test`: run the focused Rust tests.
- `npm run check`: frontend build and Rust Clippy with warnings denied.
- `npm run tauri build`: build desktop bundles.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust tests.

## Coding Style & Naming Conventions

Use two-space indentation in Vue, TypeScript, and JSON; use `rustfmt` defaults for Rust. Prefer Vue Composition API with `<script setup lang="ts">`, PascalCase component filenames, camelCase TypeScript functions, and snake_case Rust modules. Use Tailwind utilities for UI styling. Run `cargo fmt --manifest-path src-tauri/Cargo.toml` and `npm run check` (frontend type-check/build plus Clippy with warnings denied).

## Testing Guidelines

Rust tests cover surface geometry, flier recovery, dragging, pause, and alpha hit-testing. No frontend test framework or coverage threshold is configured. Add focused tests for behavior changes, especially surface geometry, traversal, and future pack validation. Use colocated `*.test.ts` frontend tests when a runner is introduced; use Rust unit tests and `src-tauri/tests/` for integration tests. Verify milestone acceptance criteria manually where native window behavior matters.

## Commit & Pull Request Guidelines

This folder is not yet a Git repository. No Git history exists to establish conventions. Use concise imperative commits, optionally prefixed with `feat:`, `fix:`, or `docs:`. PRs should identify the milestone or issue, explain behavior changes, list validation results, and include screenshots for UI changes.

## Agent-Specific Instructions

Before code discovery, use `memory-mcp`: derive repo `lupetty` and the current Git branch; pass branch explicitly where accepted. Call `search_memory`, then `search_code` with `includeText: true`. Read returned ranges with small surrounding context; use `get_entity_graph` for relationships. Incrementally `index_codebase` if results are empty or stale before shell fallback. If unavailable, explicitly announce the exception before manual search.

Write only durable findings through `ingest_note` with repo, branch, and `source: "codex"`. Promote selectively at merge; review stale scopes before pruning.

## Current MVP boundaries

The native platform adapter currently supports macOS primary-display window geometry. Windows/Linux have a desktop-edge fallback and are not validated. The default renderer is a transparent primary-display overlay, chosen after the M0 comparison; `PERCH_RENDERER=small` keeps the alternate implementation available. Rust owns all world and pet state. No pack installation, awareness, persistence, fullscreen suspension, or multiple-monitor traversal is implemented yet. Do not describe M1–M4 as complete. See `docs/rendering-decision.md`.
