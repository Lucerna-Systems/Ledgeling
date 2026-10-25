# Working on Ledgeling: run, debug, build

Written so a smaller model (or a new person) can pick up a task from
[`planning_files/01-RELEASE-PLAN.md`](../planning_files/01-RELEASE-PLAN.md)
without guessing. Commands here come from `package.json`, `.github/workflows/`
and the README. The machine this was written on had no Rust toolchain and no
`node_modules`, so **none of these commands were executed while writing this
guide**. If one fails, believe the error, fix the guide in your PR, and say so.

## 0. Read this first

1. Read `CLAUDE.md` (the six non-negotiables) and skim `docs/verification.md`.
2. Work in your own worktree on your own branch. Never commit to `main` or `prod`.
3. One task = one PR into `main`. Keep it small.
4. Before you say "done": run the full check in section 3 and paste the real output.
5. If you did not run something, say "not run". Never edit `docs/verification.md`
   to claim a run that did not happen.

## 1. What the project is, in a paragraph

A Tauri 2 desktop app. **Rust owns all state**: pet position, behaviour, the
window-edge "surface graph". The Vue 3 webview only draws sprites where Rust says.
Pets walk, climb and fly along the edges of the user's real windows. All OS calls
go through the `Platform` trait. An opt-in assistant ("tasks") can talk to AI
providers; the pets themselves never touch the network. The app is called
Ledgeling; the repo and some identifiers still say `lupetty` or `perch`.

### Where things are

| Path | What |
|---|---|
| `src-tauri/src/engine.rs` | Headless core. World scan, pets, awareness, packs. No Tauri. **Put logic here.** |
| `src-tauri/src/app.rs` | Tauri wiring, commands, the update loop. Keep thin. |
| `src-tauri/src/world/` | Window-edge extraction, occlusion, traversal graph, scan diffing |
| `src-tauri/src/pet/` | Locomotion (flier, climber, walker, hopper), A*, behaviour state machine |
| `src-tauri/src/packs/` | Pet pack manifest, validation, condition grammar, install |
| `src-tauri/src/awareness/` | Idle, session, app category, nudges |
| `src-tauri/src/platform/` | `macos.rs`, `windows.rs`, `linux/`, `mock.rs`; overlay and bubble windows in `pet_ui.rs`; folder drag handoff in `windows_drop.rs` and `handoff.rs` |
| `src-tauri/src/tasks/` | The opt-in assistant: providers, queue, file actions, approvals |
| `src-tauri/src/coding/` | Experimental coding mode. Compiled only with `--features coding` |
| `src-tauri/tests/` | `timeline.rs` (mock platform replays), `privacy.rs`, `task_connections.rs` (ignored, needs credentials), `linux_desktop.rs` (Linux only) |
| `src/` | Vue frontend. `Companion.vue` dashboard, `panels/`, `assistant/`, `bubble/`, `studio/`, `pet/` overlay renderer, `code/` coding mode page |
| `packs/` | Bundled pets: budgie-classic, squirrel-red, gecko-glass |
| `sidecar/` | Node bridge for the Claude runtime; `scripts/prepare-sidecar.mjs` copies `node` next to it |
| `schema/` | Pet pack JSON Schema |
| `scripts/` | `benchmark.py`, `soak.py` (macOS-only today), `linux-display-tests.sh`, `make_packs.py` |

### Rules that break the build or the product

- **Window titles are never read**, nor screen contents, key state or clipboard.
  `tests/privacy.rs` scans the source. If it fails, change your code, not the test.
- **Network calls only in** `tasks/providers.rs`, `tasks/search.rs`,
  `tasks/commands.rs`, `tasks/apple.rs`. No telemetry, no new network crates.
- **Packs are data, never code.** No scripting in the condition grammar.
- **Nudges are body language only**: no popups, toasts, text, focus steals.
- **Never emit a Tauri event while holding the engine lock** (deadlock, has happened).
- **Monitor enumeration and window creation run on the main thread only** (`on_main`
  in `app.rs`). An empty screen list means "suspend", not "empty desktop".
- Coordinates are physical pixels in virtual-desktop space. Negative is legal.
  Convert to logical only at the Tauri window API boundary.
- Pack validation errors must name file, JSON path, problem and expectation.
  Never surface a raw serde error.

## 2. Set up a machine

### Requirements

| Tool | Version | Notes |
|---|---|---|
| Node.js | 22.12+ (CI uses 24) | `node -v` |
| Rust | stable, via `rustup` | `cargo -V`. Components `clippy` and `rustfmt` |
| macOS | Xcode Command Line Tools | `xcode-select --install` |
| Windows | Visual Studio Build Tools ("Desktop development with C++"), WebView2 runtime (preinstalled on Windows 11) | Use PowerShell or Git Bash |
| Linux | `libwebkit2gtk-4.1-dev libgtk-3-dev libgtk-layer-shell-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev libdbus-1-dev` (Debian/Ubuntu names) | Run by hand only on COSMIC so far; see `docs/verification.md` |

Tauri's own prerequisites page covers anything missing: https://v2.tauri.app/start/prerequisites/

### First-time install in a fresh worktree

```bash
git fetch origin
git worktree add ../ledgeling-<topic> -b <topic> origin/main
cd ../ledgeling-<topic>
npm ci
cd sidecar && npm ci && cd ..
npm run prepare:sidecar
```

`prepare:sidecar` copies your `node` binary to `sidecar/node` (`node.exe` on
Windows). The Tauri build refuses to compile if a bundled resource listed in
`tauri.conf.json` is missing, so skipping this gives a confusing build error.
The first Rust build downloads and compiles many crates; expect several minutes.

## 3. Check your work (do this before every PR)

This is exactly what CI runs on macOS, Windows and Linux.

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
npm run check        # vue-tsc + vite build, then clippy twice (default and --features coding), warnings denied
npm test             # cargo test with --features coding
npm run test:ui      # vitest
```

If `fmt --check` fails, run `cargo fmt --manifest-path src-tauri/Cargo.toml` and re-run.

Useful narrower commands while working:

```bash
# one Rust test by name (substring match)
cargo test --manifest-path src-tauri/Cargo.toml --features coding <name>

# one test file
cargo test --manifest-path src-tauri/Cargo.toml --test timeline
cargo test --manifest-path src-tauri/Cargo.toml --test privacy

# the release configuration: no coding feature
cargo test --manifest-path src-tauri/Cargo.toml

# show println!/eprintln! output from passing tests
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture

# one UI test file
npx vitest run src/assistant/ProvidersPanel.test.ts
```

Tests marked `#[ignore]` need credentials or a running model (`task_connections.rs`)
and are not part of `npm test`. Do not run them against anyone's real account
without being told to.

## 4. Run the app

```bash
npm run tauri dev           # the real app: Rust + webview, hot-reloading frontend
npm run dev:coding          # same, with the experimental coding mode compiled in
```

`tauri dev` starts Vite on `http://127.0.0.1:1420` (strict port; if something
else uses 1420 it fails) and builds the Rust side. The dashboard starts
**hidden**; open it from the tray/menu-bar icon, or Cmd+1 on macOS. Pets appear
on the one designated pet screen, on the edges of your open windows.

- **macOS** asks for no special permission for window geometry. If you get no
  pets, check the session is unlocked (a locked session reports no screens and
  the engine suspends on purpose).
- **Windows**: run `npm run prepare:sidecar` once, or `tauri dev` fails on the
  missing `node.exe`. `tauri.windows.conf.json` is merged automatically.
- Only Rust changes need a rebuild (automatic under `tauri dev`). Vue changes
  hot-reload.
- `npm run dev` alone runs only the frontend in a browser. The backend commands
  will not exist there, so most screens will not work. It is useful for layout,
  not behaviour.

### Reading the app's state without a debugger

| Want | Do |
|---|---|
| Engine and screen diagnostics on stderr | Start with `LEDGELING_DIAGNOSTICS=1` (or `PERCH_DIAGNOSTICS=1`) (macOS/Linux: `LEDGELING_DIAGNOSTICS=1 npm run tauri dev`; PowerShell: `$env:LEDGELING_DIAGNOSTICS=1; npm run tauri dev`) |
| Rust panic location | `RUST_BACKTRACE=1` |
| Webview console and network | In a `tauri dev` build, right-click the **dashboard** window and choose Inspect. The pet overlay is click-through, so inspect the dashboard, not the pets |
| See the surface graph, paths and state labels | Pets → Make your own pet → Test → debug overlay |
| Pack validation messages | Pets → Make your own pet → Test; the same diagnostics come from `src-tauri/src/packs/diagnostics.rs` |
| Settings, installed packs, assistant data | Tauri's `app_config_dir()` for identifier `technology.lucerna-systems.ledgeling`: macOS `~/Library/Application Support/technology.lucerna-systems.ledgeling/`, Windows `%APPDATA%\technology.lucerna-systems.ledgeling\`. Subfolders `packs/` and `assistant/`. Delete this folder to simulate a first run (back it up first) |

`migrate.rs` moves data from an older folder name on first launch; if you rename
folders, keep that path working.

## 5. Debug a bug

Work from the cheapest, most repeatable setup to the dearest.

1. **Reproduce it as a timeline test.** Pet behaviour bugs ("squirrel climbs a
   hidden edge", "pet floats after a window closes") almost never need a GUI.
   `src-tauri/tests/timeline.rs` drives `engine.rs` through `platform/mock.rs`
   with a scripted window layout over time. Copy the closest existing test, make
   it fail, then fix the code. This is the preferred way and the only one that
   is repeatable in CI.
2. **Unit-test the layer.** Surface extraction and occlusion: `src-tauri/src/world/`.
   Pathfinding and locomotion: `src-tauri/src/pet/tests.rs`. Packs and validation:
   `src-tauri/src/packs/tests.rs`. Settings: `settings.rs`. Linux window logic:
   `platform/x11_model.rs` (pure functions, testable anywhere).
3. **Run the real app** with `LEDGELING_DIAGNOSTICS=1` when the bug depends on a real
   OS (window styles, DPI, tray, drag and drop, overlay click-through). This is
   the only way to test `platform/macos.rs`, `windows.rs`, `windows_drop.rs` and
   `pet_ui.rs`.
4. **Frontend bugs**: reproduce in a vitest test (`*.test.ts` beside the
   component), then fix. For layout, use the Inspect window.

Common causes, so you check them first:

- Pet floating or perched on nothing: occlusion subtraction skipped, or a
  coordinate in logical instead of physical pixels.
- App freezes: an event emitted while holding the engine lock, or a monitor/window
  call made off the main thread.
- Pets gone after lock/sleep: empty screen list treated as an empty desktop.
- `privacy` test fails: you referenced a title, capture, key-state or network API
  in a file that may not. Remove the call; do not edit the test or the allowlist.
- Windows-only misbehaviour: you cannot reproduce it on macOS. Say so, write the
  logic so it is testable with the mock, and ask for a human to run it on Windows.

## 6. Build

### A local release build

```bash
npm run tauri build
```

This runs `prepare:sidecar` then the frontend build, then compiles Rust in
release mode (thin LTO, so it is slow) and bundles. It does **not** include
coding mode and it is unsigned.

Outputs, under `src-tauri/target/release/bundle/`:

| OS | Output |
|---|---|
| macOS | `macos/Ledgeling.app`, `dmg/*.dmg` |
| Windows | `nsis/*-setup.exe` (per-user install, no admin) |

macOS users must right-click → Open the unsigned app, or run
`xattr -dr com.apple.quarantine /Applications/Ledgeling.app`. Windows shows a
SmartScreen "unknown publisher" warning: More info → Run anyway.

### Cross-target and CI builds

CI (`.github/workflows/release.yml`) builds `aarch64-apple-darwin`,
`x86_64-apple-darwin` and `x86_64-pc-windows-msvc`. On Windows it sets
`TAURI_CONFIG` so the NSIS target, `.ico` icon and `node.exe` resource apply,
and it passes a version override file to `tauri build --config`. Do not change
this workflow without a human reviewing it: a broken release job costs a
rolling pre-alpha.

### Releases

- Push to `main` (via a squash-merged PR) publishes the rolling **pre-alpha**
  GitHub release. Install that to test; it is the fastest way to get a Windows
  installer without a Windows machine of your own.
- A merge commit from `main` into `prod` publishes `v<version>-alpha.<n>` and,
  if `ITCH_ENABLED=true`, pushes to itch.io. **Only a human does this.**
- Bump the version in `package.json`, `src-tauri/Cargo.toml` and
  `src-tauri/tauri.conf.json` together or not at all.

### Performance and soak

`scripts/benchmark.py` and `scripts/soak.py` currently work on **macOS only** (they
read `ps` and `lsof` and default to the `.app` path). Both honour
`LEDGELING_BENCH_BINARY` (falling back to `PERCH_BENCH_BINARY`). Run against a release build, never `tauri dev`. A pass is
the budget in `CLAUDE.md` constraint 5. Porting them to Windows is WP-4 in the
release plan.

## 7. Make a change safely

1. Pick a package from the release plan; read every file it lists.
2. Write or extend a failing test first where the area allows it.
3. Make the smallest change. No drive-by refactors, no reformatting unrelated code.
4. Match surrounding style, comment density and naming. The code explains *why*
   in comments, not *what*.
5. Run section 3 in full.
6. Open the PR into `main`:

```bash
git push -u origin <topic>
gh pr create --base main
```

PR description: what changed, why, exactly which commands you ran with their
result, and what you could not run (for example "not run on Windows"). Fill in
the PR template checklist honestly. A human reviews and squash-merges.

### Stop and ask a human when

- The change would touch `CLAUDE.md`, a privacy test, a Tauri capability file
  or the release workflow.
- A fix seems to need weakening a test or an assertion.
- The bug only reproduces on hardware you do not have.
- You are about to add a dependency, a network call, or new window-API usage.
- The task needs a product decision (names, prices, what unlocks, what is in
  or out). Those are listed as **[human decides]** in the plan.

## 8. A prompt to give a smaller model

> You are working on Ledgeling in `<worktree path>`. First read
> `docs/working-on-ledgeling.md` and `CLAUDE.md`. Your task is work package
> `WP-<n>` in `planning_files/01-RELEASE-PLAN.md`. Stay inside the files it
> lists. Write a failing test first where possible. When done, run the four
> commands in section 3 of the guide and show their real output. Do not edit
> `docs/verification.md`, privacy tests, capability files or workflows. If
> something is ambiguous or needs a decision, stop and ask instead of choosing.
> Do not commit to `main`; open a PR into `main`.
