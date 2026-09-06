# MVP verification — 2026-09-06

## Automated checks

- `npm run build`: passes Vue/TypeScript checking and production asset build.
- `npm test`: 13 Rust tests pass, covering occlusion, clipping, physical/negative coordinates, minimum perch width, landing, window movement/loss, lost flight destinations, pause, drag bounds/release, and sprite-alpha hit-testing.
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`: passes.
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`: passes.
- `npm run tauri build -- --bundles app`: produces a runnable macOS `Perch.app` with embedded frontend assets.
- `npm ls --depth=0`: all declared frontend dependencies installed without missing/extraneous package errors.

## Native runtime checks

Verified in the packaged application, using its real Rust backend:

- Companion screen renders and connects to Rust without a development server dependency.
- Pause changes to Resume; resuming restores normal activity.
- Flight-speed slider responds to keyboard input and retains the value across status refreshes.
- Show/hide changes both runtime state and the companion control.
- Find a new perch changes the bird into its flight state.
- Live desktop map reports nine windows and a visible real window perch on this desktop.
- Native diagnostics confirm repeated landings at the real window edge (`y = 292` physical pixels) and at the desktop edge (`y = 1912`), with no enumeration errors.
- Debug-surface switch changes state and the map shows the available top edge and bird marker.
- Native application menu and Cmd+Shift+H / Cmd+Shift+P shortcuts work after separating tray menu items and registering the menu callback on the builder.
- Closing the companion window leaves the process alive; Cmd+1 reopens it and preserves the session. Cmd+Q cleanly exits the app.
- Original sprite strip visually inspected in a transparent native pet window; companion and live-map layouts inspected via screenshots.

The current desktop generally scanned in roughly 1–4 ms, with some observed scans around 7 ms and a cold/wakeup sample around 22 ms. This is not the requested 20+-window performance qualification.

## Measurements and remaining qualification

Both ten-minute rendering comparisons completed. See `rendering-decision.md` and the raw benchmark JSON files. The overlay is the default; CPU/RAM ceilings across the full process tree are **not** certified.

Some capture-tool operations failed when a transparent overlay was selected. Native controls and the map were verified by focusing the companion screen; `PERCH_START_HIDDEN=1` also permits isolated UI verification. Visual smoothness, physical clicking through to other applications, and dragging the bird across different apps still warrant manual testing on the target machine. Alpha-mask correctness, drag/release bounds, and moved/lost-window recovery have focused Rust tests.

No Windows runtime check, multi-monitor traversal qualification, fullscreen/lock/sleep qualification, long stability soak, signing, or public release was performed. These remain roadmap gates, not hidden completion claims.
