# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Current state

M0–M4 are implemented as far as code can take them. The surface graph (all four
window edges, occlusion-subtracted, plus monitor and taskbar surfaces), four
locomotion classes, pack loading and validation, the declarative behavior machine,
the awareness layer and the nudge ladder, settings persistence and the tray
lifecycle all exist and are tested. Three bundled species — budgie, squirrel,
gecko — differ only in data.

What is genuinely outstanding is not code: the Windows build has had only a
first hand-run on one Windows 11 machine, the Linux adapter has run only under
Xvfb and headless sway in CI (see `docs/verification.md`), nothing
is signed, the performance budget has not
been re-certified and the soak has not been run for this build, no outside author
has shipped a pack, and nothing has been released. `docs/verification.md` is the
honest account; keep it that way and do not let README or docs drift ahead of it.

Rendering is one transparent overlay on the single designated pet screen (see
`docs/rendering-decision.md`, including the M1 and single-screen addenda). The
`PERCH_RENDERER=small` variant has been removed. Pets are confined to that one
screen by design — the engine is fed only that monitor, so window-edge surfaces
on other screens are clipped away before the graph is built.

## Read the docs before writing code

| Doc | Read when |
|---|---|
| [00-PLAN.md](planning_files/00-PLAN.md) | Every session — plan, MVP status, what's left |
| [verification.md](docs/verification.md) | What has actually been run; keep it honest |
| [development.md](docs/development.md) | Branches, worktrees, review and releases |

### Subsystem Documentation (`CLAUDE.md` Map)
To inspect or modify specific subsystems without reading the entire repository, refer to each module's dedicated guidance:
- **World & Surfaces**: [`src-tauri/src/world/CLAUDE.md`](src-tauri/src/world/CLAUDE.md)
- **Pet Runtime & Locomotion**: [`src-tauri/src/pet/CLAUDE.md`](src-tauri/src/pet/CLAUDE.md)
- **Tauri App Wiring & Loops**: [`src-tauri/src/app/CLAUDE.md`](src-tauri/src/app/CLAUDE.md)
- **Coding Mode Workspace**: [`src-tauri/src/coding/CLAUDE.md`](src-tauri/src/coding/CLAUDE.md)
- **Assistant & Tasks**: [`src-tauri/src/tasks/CLAUDE.md`](src-tauri/src/tasks/CLAUDE.md)
- **Terminal & Agents**: [`src-tauri/src/terminal/CLAUDE.md`](src-tauri/src/terminal/CLAUDE.md)
- **Pack Manifests & Validation**: [`src-tauri/src/packs/CLAUDE.md`](src-tauri/src/packs/CLAUDE.md)
- **Awareness & Nudges**: [`src-tauri/src/awareness/CLAUDE.md`](src-tauri/src/awareness/CLAUDE.md)
- **Frontend Composables**: [`src/composables/CLAUDE.md`](src/composables/CLAUDE.md)
- **Frontend Panels**: [`src/panels/CLAUDE.md`](src/panels/CLAUDE.md)

## Architecture in one page

**Preferred Design: Modular & Decoupled Architecture.**
Avoid giant controller hubs. Each system is isolated into a focused module with its own single responsibility, clear contracts, and lifecycle (`SubsystemService`). Logic is kept headless so that any subsystem can be unit-tested or extracted independently without running or mocking Tauri.

**Rust owns all truth.** Pet position, state, and the world model live in the Rust backend. The webview is a dumb renderer that draws a sprite frame at a position it was told. Behavior stays deterministic and testable without a GUI. Dragging a pet sends pointer events *to* Rust, which moves the window — the frontend never repositions itself.

**The surface graph is the central abstraction.** Visible window edges are extracted front-to-back with z-order occlusion subtracted, then connected as `CONTIGUOUS` / `JUMPABLE` / `AIR` edges. A pet occupies `(SurfaceId, t)` with `t ∈ [0,1]`, so it rides a window for free when the window moves. Skipping the z-order subtraction (step 2 of the build pipeline) means pets perch on hidden edges and the whole illusion collapses.

**Species differ by traversal ruleset, not animation.** One graph, four classes: `flier` (ignores graph edges, air paths), `climber` (A* over contiguous + jumpable), `walker` (contiguous horizontal only), `hopper` (walker + short jumps). Adding a species is data; adding a *class* is an engine change.

**All OS calls sit behind the `Platform` trait** (`enumerate_windows`, `idle_seconds`, `foreground_process`, `monitors`, `fullscreen_app_active`) so macOS/Linux can be added later without touching pet logic. Keep the renderer behind a `PetSurface` trait too — the per-pet-window vs. single-overlay decision may still flip.

**Coordinates are physical pixels in virtual-desktop space** (origin at primary monitor's top-left; negative values are legal). Convert to logical only at the Tauri window API boundary.

## Constraints that are not negotiable

These are product-defining, not preferences. Violating one is a bug, not a tradeoff.

1. **Window titles are never read.** Foreground detection goes `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`, takes the process name, maps it to an `AppCategory`, and discards it. Titles contain email subjects, document names and URLs. M3-2 requires a test asserting no title API is called.
2. **Pet packs are data, never code.** No JS, no Lua, no eval. Conditions use a fixed, non-Turing-complete grammar (`signal OP value` joined by `&&`/`||`) over a closed signal namespace. A malicious pack must at worst look wrong.
3. **Zero network calls by core companion; opt-in assistant network access strictly isolated.** The passive companion makes zero network calls — no telemetry, no crash reporting, no update checks. Network access is strictly opt-in for assistant tasks, confined exclusively to four designated adapter files (`tasks/providers.rs`, `tasks/search.rs`, `tasks/commands.rs`, `tasks/apple.rs`), and enforced by `src-tauri/tests/privacy.rs`. Tauri capabilities stay minimal — no `shell`, no `fs` beyond the packs directory and config file.
4. **Nudges are body language only.** No popups, toasts, text, or focus steals, ever. Cooldowns on everything; escalation caps at the third tier; suppressed in `game`/`media`.
5. **Performance budget is a ceiling, not a goal.** < 0.5% CPU idle, < 2% moving, ~0% suspended; < 80 MB RAM for one pet; window scan < 3 ms. A companion app that eats battery gets uninstalled.
6. **Milestones are hard gates.** Nothing from M3 is built during M1; nothing from the deferred list before M4 ships publicly. The likeliest failure mode for this project is an unshipped, half-built, over-ambitious version.

## Pack format design test

> A person who can draw and edit JSON, but cannot program, must be able to ship a working climbing pet in under an hour.

Any proposed format feature that breaks that test does not go in. Corollaries the code must honour: a minimal pack is two sprite strips and ~20 lines of JSON; every optional animation falls back down a chain (`climb → move → idle`) so a pack never fails to load for something absent; and every validation failure names the file, the JSON path, what was wrong, and what was expected — **no error message may be only a stack trace or a raw serde error**.

## Branches and PRs

`main` is testing (every push becomes the pre-alpha release) and `prod` is
releases (every push becomes an alpha). Never commit to either directly. Work on
a branch in its own worktree off `origin/main` and open a PR into `main`. The
user reviews every PR before it merges. See `docs/development.md`.

## Working on this now

The build order in the planning docs is history: M0-4 was spiked and recorded, and
M1-M3 are built. What matters when changing things now:

- **`engine.rs` is the headless core.** Anything interesting should be reachable
  from it without Tauri, because that is what the timeline tests drive through a
  mock `Platform`. Resist putting behavior in `app.rs`.
- **Never emit a Tauri event while holding the engine lock.** Emitting hops to the
  main thread, which may be inside a command handler waiting for that lock. This
  deadlock has happened once already; the update loop is shaped to avoid it.
- **macOS monitor enumeration and window creation only work on the UI thread**,
  and report nothing at all when the session is locked. `on_main` in `app.rs`
  exists for this, and an empty screen list means suspend, not an empty desktop.
- **The privacy tests are load-bearing.** They scan the source for title,
  capture, key-state and network APIs. If one fires, the fix is the code.

## Testing shape

The architecture is deliberately arranged so the interesting logic needs no GUI: unit-test surface extraction against synthetic window layouts (full occlusion, partial overlap, nested, off-screen, multi-monitor), pathfinding invariants (a climber cannot cross a gap wider than `max_gap`; a flier can), and every malformed-manifest case. Integration tests replay recorded window-layout timelines through a mock `Platform`, so "user dragged a window while the squirrel was climbing it" is repeatable.
