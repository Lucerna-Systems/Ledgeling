# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Current state

A working macOS M0 MVP is scaffolded with Vue 3, TypeScript, Tailwind 4, Tauri 2, and Rust. The folder is not a Git repository. Read `README.md` for run/build/test commands and scope, and `AGENTS.md` for current repository instructions. The roadmap remains Windows-first; the Mac adapter makes the MVP run on this development machine.

Default rendering is a transparent primary-display overlay following the recorded M0 comparison in `docs/rendering-decision.md`. The small-window renderer remains selectable for experiments. Pet packs, a second species, awareness, multiple-monitor traversal, and release distribution are still future milestones.

## Read the docs before writing code

| Doc | Read when |
|---|---|
| [00-README.md](planning_files/00-README.md) | Orientation |
| [01-product-brief.md](planning_files/01-product-brief.md) | Deciding *what* to build; scope disputes |
| [02-technical-architecture.md](planning_files/02-technical-architecture.md) | Writing any code |
| [03-pet-pack-spec.md](planning_files/03-pet-pack-spec.md) | Anything touching pets or the pack format |
| [04-awareness-and-behavior.md](planning_files/04-awareness-and-behavior.md) | Signals, energy/mood, nudges |
| [05-user-stories.md](planning_files/05-user-stories.md) | Every session — this is the backlog |

## Architecture in one page

**Rust owns all truth.** Pet position, state, and the world model live in the Rust backend. The webview is a dumb renderer that draws a sprite frame at a position it was told. Behavior stays deterministic and testable without a GUI. Dragging a pet sends pointer events *to* Rust, which moves the window — the frontend never repositions itself.

**The surface graph is the central abstraction.** Visible window edges are extracted front-to-back with z-order occlusion subtracted, then connected as `CONTIGUOUS` / `JUMPABLE` / `AIR` edges. A pet occupies `(SurfaceId, t)` with `t ∈ [0,1]`, so it rides a window for free when the window moves. Skipping the z-order subtraction (step 2 of the build pipeline) means pets perch on hidden edges and the whole illusion collapses.

**Species differ by traversal ruleset, not animation.** One graph, four classes: `flier` (ignores graph edges, air paths), `climber` (A* over contiguous + jumpable), `walker` (contiguous horizontal only), `hopper` (walker + short jumps). Adding a species is data; adding a *class* is an engine change.

**All OS calls sit behind the `Platform` trait** (`enumerate_windows`, `idle_seconds`, `foreground_process`, `monitors`, `fullscreen_app_active`) so macOS/Linux can be added later without touching pet logic. Keep the renderer behind a `PetSurface` trait too — the per-pet-window vs. single-overlay decision may still flip.

**Coordinates are physical pixels in virtual-desktop space** (origin at primary monitor's top-left; negative values are legal). Convert to logical only at the Tauri window API boundary.

## Constraints that are not negotiable

These are product-defining, not preferences. Violating one is a bug, not a tradeoff.

1. **Window titles are never read.** Foreground detection goes `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`, takes the process name, maps it to an `AppCategory`, and discards it. Titles contain email subjects, document names and URLs. M3-2 requires a test asserting no title API is called.
2. **Pet packs are data, never code.** No JS, no Lua, no eval. Conditions use a fixed, non-Turing-complete grammar (`signal OP value` joined by `&&`/`||`) over a closed signal namespace. A malicious pack must at worst look wrong.
3. **Nothing leaves the machine in v1.** No telemetry, no crash reporting, no network calls at all. Tauri capabilities stay minimal — no `shell`, no `fs` beyond the packs directory and config file.
4. **Nudges are body language only.** No popups, toasts, text, or focus steals, ever. Cooldowns on everything; escalation caps at the third tier; suppressed in `game`/`media`.
5. **Performance budget is a ceiling, not a goal.** < 0.5% CPU idle, < 2% moving, ~0% suspended; < 80 MB RAM for one pet; window scan < 3 ms. A companion app that eats battery gets uninstalled.
6. **Milestones are hard gates.** Nothing from M3 is built during M1; nothing from the deferred list before M4 ships publicly. The likeliest failure mode for this project is an unshipped, half-built, over-ambitious version.

## Pack format design test

> A person who can draw and edit JSON, but cannot program, must be able to ship a working climbing pet in under an hour.

Any proposed format feature that breaks that test does not go in. Corollaries the code must honour: a minimal pack is two sprite strips and ~20 lines of JSON; every optional animation falls back down a chain (`climb → move → idle`) so a pack never fails to load for something absent; and every validation failure names the file, the JSON path, what was wrong, and what was expected — **no error message may be only a stack trace or a raw serde error**.

## Build order

The first thing to build is not the first story in the backlog. Start with **M0-4**, the spike measuring whether a 128×128 window can be repositioned at 60 Hz without jitter or excessive CPU — the rendering architecture depends on the answer, and the decision must be recorded back into doc 02. Then the rest of M0. Within M1, build the **climber before the flier**: the flier ignores the graph, so building it first hides graph flaws until late. M2 (packs) comes before M3 (awareness), because the pack system changes engine interfaces.

## Testing shape

The architecture is deliberately arranged so the interesting logic needs no GUI: unit-test surface extraction against synthetic window layouts (full occlusion, partial overlap, nested, off-screen, multi-monitor), pathfinding invariants (a climber cannot cross a gap wider than `max_gap`; a flier can), and every malformed-manifest case. Integration tests replay recorded window-layout timelines through a mock `Platform`, so "user dragged a window while the squirrel was climbing it" is repeatable.
