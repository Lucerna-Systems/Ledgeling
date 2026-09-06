# Perch

A little company for your desktop. Perch is a Vue 3 + TypeScript + Tailwind CSS + Tauri 2 app with a Rust-owned desktop bird.

## Run it

Requirements: Node.js 22.12+ (or a compatible newer version), Rust stable, and the native [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS, install Xcode Command Line Tools.

```sh
npm install
npm run tauri dev
```

The native app opens a companion screen and a transparent bird overlay. Pip automatically flies between visible window tops on the **primary display**. Move or close a supporting window and Pip rides it or finds another perch. Open a normal, non-maximized window with some space above its title bar to give Pip somewhere to land.

- Drag Pip to pick them up; release to let them find a perch.
- Right-click Pip to say hello.
- Use **Find a new perch**, **Pause**, **Hide Pip**, and the flight-speed slider in the companion screen.
- **Desktop view** shows live window geometry and can draw available perches on the desktop.
- While Perch is focused, **Cmd/Ctrl+Shift+H** shows/hides Pip and **Cmd/Ctrl+Shift+P** pauses/resumes. **Cmd/Ctrl+1** reopens the companion screen.
- Closing the companion screen leaves Pip running. Reopen it from the menu-bar icon. Quit from that menu or **Quiet by design → Quit Perch**.

`npm run dev` starts a browser preview at http://127.0.0.1:1420. Desktop controls require the native app; browser preview does not pretend to enumerate windows.

## Build and check

```sh
npm run build
npm test
npm run check
npm run tauri build
```

On this Mac, the application bundle is written to `src-tauri/target/release/bundle/macos/Perch.app`. Open it normally, or:

```sh
open src-tauri/target/release/bundle/macos/Perch.app
```

This is a local, unsigned development build, not a signed or notarized public release. No development server is required by the release app.

`npm run check` builds/type-checks the frontend and runs Rust Clippy with warnings treated as errors. `npm test` runs focused Rust tests for occlusion, clipping, DPI geometry, landing, moving/closed windows, lost destinations, dragging, pause, and alpha hit-testing.

## MVP scope

This implements the **M0 desktop-bird proof**, with the interaction and lifecycle controls needed to use it. The supplied roadmap remains in [planning_files](planning_files/00-README.md).

Working here: macOS real window geometry, one bundled bird, transparent click-through overlay, animated flights, live top-edge detection, primary-display scale conversion, pause/hide, drag, menu-bar controls, and debug surfaces.

Not implemented: Windows native window enumeration, multiple-monitor traversal, squirrels/climbing, installable packs, awareness, persistent preferences, automatic fullscreen/lock suspension, or distribution/signing. Other platforms currently have a desktop-edge fallback and have not been built or runtime-verified. This does **not** claim M1–M4 completion.

The Mac adapter was added to make the requested MVP run on the available development machine; it does not establish cross-platform parity. Keep the Windows-first roadmap for future product work.

## How it works

- `src-tauri/src/platform/`: OS queries behind `Platform`. The Mac adapter extracts only geometry, window IDs, layer and opacity from CoreGraphics. It never reads window titles or captures the screen. Cursor position/button state are used only for pet interactions.
- `src-tauri/src/world.rs`: front-to-back occlusion subtraction, visible top edges, and a permanent desktop-edge fallback.
- `src-tauri/src/pet.rs`: authoritative position, flight, `(owner, t)` anchoring, recovery, and interaction state.
- `src-tauri/src/windows_mgr.rs`: renderer boundary and alpha hit-testing from the actual sprite pixels.
- `src/pet/`: rendering-only webview. The companion controls and live geometry view are in `src/Companion.vue`.
- `public/idle.png`, `public/move.png`: original four-frame bird sprite strips. `scripts/make_sprites.py` reproduces the art and its Rust alpha masks (requires Pillow).

Runtime data stays in memory. No accounts, telemetry, network services, window contents, or keystroke collection. Tauri capabilities allow only local event subscriptions; application commands are implemented in Rust. Runtime errors are shown in the companion screen.

## Rendering spike

The MVP defaults to a transparent primary-display overlay. `PERCH_RENDERER=small npm run tauri dev` enables the alternative 128×128 moving window for development comparison. Desktop debug lines are intended for the overlay renderer.

The benchmark runs a fixed 60 Hz path and captures native-process CPU/RSS once a second:

```sh
npm run tauri build
python3 scripts/benchmark.py small 600
python3 scripts/benchmark.py overlay 600
```

Reports are written to `docs/benchmark-small.json` and `docs/benchmark-overlay.json`. The sampler uses macOS `ps` and excludes WebKit helper and WindowServer costs. See [the architecture decision](docs/rendering-decision.md) for results and limitations.

For local verification, `PERCH_START_HIDDEN=1` starts with Pip hidden and `PERCH_DIAGNOSTICS=1` prints a geometry-count/scan-time summary to stderr every five seconds. Both are off by default.
