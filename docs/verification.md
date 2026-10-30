# Verification status

Updated after the M1–M4 implementation pass, the task/assistant layer
(`docs/assistant.md`), and the single-screen multi-display fix (2026-09-09,
see `docs/rendering-decision.md`'s single-screen addendum). This file says
what has actually been run, and — more importantly — what has not.

## Automated, run on every change

| Check | Result |
|---|---|
| `npm test` (`cargo test`) | 149 tests pass: 132 unit, 10 integration timeline, 7 privacy; plus 11 ignored (1 local-Ollama, 9 credential-gated connections, 1 Apple on-device) and, on Linux, 2 ignored live display tests that `Verify` runs explicitly |
| `cargo clippy -- -D warnings` (in `npm run check`) | passes |
| `cargo fmt --check` | passes |
| `npm run build` (`vue-tsc` + Vite) | passes |
| Platforms | `Verify` runs all of the above on `macos-latest`, `windows-latest` and `ubuntu-latest`; the Linux leg also runs the live adapter tests against Xvfb + openbox and headless sway (`scripts/linux-display-tests.sh`) |
| `npm run test:ui` (vitest) | 62 tests pass (approvals and their plain-words summaries, refresh timing, drafts, collection search/filtering, assistant clip slots in the packs panel, history expansion, automatic task mode, and sanitized Markdown) |

What those tests actually cover:

- **Surface extraction** — every edge kind, full occlusion, partial overlap that
  splits an edge in two, nested windows, off-screen clipping, the minimum segment
  length, multi-monitor with negative coordinates and mixed DPI, and the property
  that a pet's parametric position survives an edge being split underneath it.
- **Locomotion** — a pet rides a window that moves; a flier takes off when its
  window closes; a climber falls and lands on the first surface *below* it rather
  than the nearest one; a falling pet with nothing below still lands on the floor;
  a perch that shrinks under a pet makes it leave; a climber never takes a single
  jump wider than its `max_gap`; a flier reaches places a climber cannot; a walker
  is never offered a vertical surface; the gecko only ever settles on window sides.
- **Never-floating invariant** — 4 000 consecutive ticks with position asserted
  finite and inside the virtual desktop on every one.
- **Packs** — the spec's minimal manifest loads; the fallback chain resolves;
  unknown fields produce INFO and still load; broken JSON, quoted frame counts,
  missing required fields, unknown classes, surfaces, actions and events, bad
  conditions and undeclared state names each produce a specific message; and a
  property test asserts **no diagnostic is ever a bare serde error or stack trace**.
- **Timeline integration** — a mock platform replays recorded window layouts:
  dragging a window carries the squirrel, closing the last window strands nobody,
  a second monitor gets used and unplugging it recovers, a fullscreen app suspends
  and resuming works, 24 windows scan inside the budget, pets and positions
  survive a restart, and a pet whose pack was uninstalled is dropped cleanly.
- **Privacy** — the build fails if any source file references a window-title,
  screen-capture, key-state or clipboard API; if any file outside the three
  explicit task adapters (`tasks/providers.rs`, `tasks/search.rs`,
  `tasks/commands.rs`, `tasks/apple.rs`) references a network API; if a telemetry or frontend
  network crate is in `Cargo.toml`; or if the Tauri capability list grants
  anything beyond local window events. The source walk covers `.m` files in
  `native/`.
- **Task service** — queue ordering (one task at a time, including while
  awaiting approval), file-conflict rules (destinations must be new, journal
  before each operation, SHA-256 guards, undo including interrupted
  operations), cancellation semantics (an in-flight file operation finishes,
  the next is blocked), restart recovery (unfinished tasks are marked
  Interrupted, never resumed), model action validation (batch limits, forbidden
  destinations), and the scripted classification plan every direct provider
  runs through a fake endpoint. These are deterministic executor tests — they
  are not evidence of live provider quality.

## Coding mode (2026-09-27)

Coding mode is experimental and compiled only with `--features coding`, so
nothing in this section applies to release or pre-alpha builds, which leave it out.


- Automated, macOS:
  - **Rust suite.** New tests cover path confinement (including `..` and symlink
    escapes), porcelain status/numstat/worktree parsing, and a real temporary
    repository round trip (status, stage, unstage, discard, commit, and adding
    and removing a worktree). They also cover gitignore marking, save-conflict
    detection, lsof/netstat/ps parsing, `run.json` validation messages and
    project-type guesses, pet↔worktree bindings across a restart, window
    placement, and a real PTY running a command to exit.
  - **Privacy tests** pass. The capabilities test now checks every capability
    file.
  - **UI suite**, including `src/code/boundary.test.ts` (no pet or dashboard page
    imports Monaco, xterm or `src/code`) and the Developer tools switch.
  - **Production build** confirmed by inspection: the chunks `index.html` loads
    contain no Monaco or xterm code, and the coding entry is about 15 KB before
    its lazily loaded tabs.
- Checked as headless-Chrome screenshots of `code.html` against a mocked
  backend: Files, Changes (a real Monaco diff), Worktrees, Run, the worktree
  switcher and Chat, in light and dark.
- Started with `tauri dev` on this Mac, and every coding module compiles in the
  dev server. **Not yet exercised by hand in the real app:** opening the window
  from the tray, sidebar or pet bubble; Claude Code in the Terminal tab and the
  pet's busy/attention clips while it works; a real Run with port detection;
  worktree creation from the UI; the install scripts. Windows and
  Linux builds of coding mode have not run at all (the PTY, `netstat` and
  PowerShell paths are written but unverified).

## Coding mode: focus mode and split terminals (2026-09-27)

- The Orca hand-off is gone: its button, command, add-on entry and install
  scripts were removed, so coding mode no longer points people at another app.
- Unit-tested: the split layout (insert, nest, close, prune, resize limits,
  malformed stored layouts) in `src/code/layout.test.ts`; the process-table
  parser and descendant walk; Tauri and Electron detection, their dev and
  build commands per package manager, and finding the newest build on each OS
  (`coding/launch.rs`). `vue-tsc`, the Vite build, `cargo test`,
  `cargo clippy -- -D warnings` and the privacy tests pass.
- Checked as headless-Chrome screenshots of `code.html` against a mocked IPC:
  the narrow top-tab layout at 460 px, the wide VS Code-style layout with the
  side panel open and hidden at 1400 px, and focus mode at 1600 px, with a
  three-pane split and the open button showing a port and a desktop app.
- The pet side chat: session notes (started, waiting, finished, failed) and
  the note store are unit-tested in `coding/notes.rs`, and `full_prompt` in
  `tasks/types.rs`. Screenshots against a mocked IPC cover the worktree list
  with Files and Changes under each worktree, Files with its back link,
  Changes in the main area, and the side chat with notes and a "What's going
  on?" summary. Asking the chat model with the summary attached has not been
  tried against a real provider.
- The terminal close button did nothing on macOS: WKWebView under Tauri has
  no dialog for `confirm()`, which answered no. Every coding-mode confirmation
  now uses an in-app dialog; closing a pane was checked in the mocked page.
- **Not run in the real app yet:** focus mode maximizing and restoring the
  window, dragging dividers, the split keys, the per-terminal open button
  finding a port or a `tauri dev` window, bringing that window forward, and
  opening a production build. Bringing an app forward is written for macOS
  and Windows and is not supported on Linux.

## Coding mode: GitHub view (2026-09-28)

- The GitHub view drives the user's own `gh` CLI. Every `gh` operation is a
  fixed argument list built in `coding/github.rs`; the window cannot pass
  arguments through. Unit-tested there: flag smuggling through repository
  names, text going on stdin, every action's argument list, auth and repo
  parsing, board columns and cards, and drafting a pull request from commits.
  `src/code/github/format.test.ts` covers the diff parser, check roll-up,
  in-app link routing and issue branch names.
- Run against the real `Lucerna-Systems/lupetty` repository from Rust through
  `gh` 2.93: sign-in status, repository, pull request and issue lists and
  search, a pull request with checks, a diff, the branch's pull request,
  labels and assignees, the org's boards, and a board with its fields. Every
  board change (move, clear, add a draft, add an issue, archive, link, unlink)
  was sent to GitHub against a nonexistent project, which GitHub rejects only
  after it has checked the request's shape; no real board was changed.
- Checked as headless-Chrome screenshots of `code.html` with the IPC answering
  from that captured real data: the pull request list and detail in light and
  dark, the Files diff, an issue, and the board (its cards were placed for the
  screenshot, since the real board is empty). This caught file headers
  covering the first diff lines and missing list bullets, both fixed.
- **Not run in the real app yet:** signing in, installing `gh`, granting board
  access, commenting, reviewing, merging, closing, editing labels, checking a
  pull request out into a worktree, creating an issue or a pull request
  (including the push), starting work on an issue, dragging cards, and
  re-running failed checks. None of these writes has been sent to GitHub.

## Simpler companion window and Pet Studio (2026-09-26)

- Navigation cut to Home, Pets, Assistant, History and Settings; advanced pages sit
  under Settings. Pets and History gained status tabs and filter pills; the Assistant
  picks a pet from a carousel and changes its model in place.
- Pet Studio (Pets → Make your own pet): a pixel editor for each animation and a
  block editor for behaviours and reactions. `save_studio_pack` writes the design
  as an ordinary pack and loads it through the normal validator before it is moved
  into the packs directory; built-in packs cannot be overwritten.
- Automated: 83 UI tests, the Rust suite (including four Studio save tests),
  Clippy and the frontend build pass on macOS.
- Checked only as screenshots of the frontend against a mocked backend in headless
  Chrome. Not yet run in the real app: no Studio save has been made through Tauri,
  and no Studio-made pet has been watched on the desktop.

## Pet Studio AI helper (2026-09-30)

- The Behaviour tab has a **Describe it** box (issue #24). It starts an ordinary
  Assistant task on the default connection with no folders, asks for a JSON design,
  and `studio/ai.ts` filters the reply through the editor's closed lists of clips,
  actions, events, signals and operators before it replaces the behaviours. Only
  the description and the behaviour outline are sent, never the drawings. Undo
  restores the previous behaviours. No manifest or Rust change.
- Automated: `studio/ai.test.ts` (parsing, rejection of out-of-list values, undo,
  prompt contents) and a Studio test with the assistant task mocked.
- Not run against any real model, so how well each provider follows the JSON
  format is unknown, and it has not been run in the real app.

## Linux on Wayland: layer-shell overlay and compositor window sources (2026-10-24)

On Wayland the overlay is now a `wlr-layer-shell` surface (through
gtk-layer-shell) wherever the compositor offers that protocol: COSMIC, KDE
Plasma, sway, Hyprland, niri and most others apart from GNOME. Before, it was
always pushed through XWayland. The overlay's input region is the pets' sprite
boxes, so the compositor routes the pointer to the overlay only over a pet,
and the overlay reports the position itself (`pet_pointer`). This fixes
hovering and grabbing pets over native Wayland windows. The bubble is a layer
surface too, placed by margins. GNOME has no layer-shell and keeps the XWayland
route. The diagnostics line now says which overlay is in use
(`"overlay": "wayland-layer"` or `"window"`).

Wayland has no common protocol for seeing other apps' windows, so each
compositor that offers one has its own source (`platform/linux/sources`):

| Compositor | Source | Windows | Focused app | Fullscreen |
|---|---|---|---|---|
| COSMIC | `zcosmic_toplevel_info_v1` v2+ with `ext_foreign_toplevel_list_v1` and `ext_workspace_v1` | yes | app id | yes |
| Hyprland | `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock`: `j/clients`, `j/monitors` | yes | process name | yes |
| sway | `SWAYSOCK`, `GET_TREE` | yes | process name | yes |
| niri | `NIRI_SOCKET`: `Windows`, `Workspaces`, `Outputs` | tiles in view | process name | no |
| anything else | none | screen edges | not reported | XWayland apps only |

No compositor reports stacking order, so windows are ordered by focus
recency, with floating windows above tiled ones. The COSMIC protocol XML is
vendored under `src-tauri/protocols/`. Its notice is permissive, unlike the
GPL-3.0 `cosmic-protocols` crate, which is not used.

**Window titles.** `ext_foreign_toplevel_list_v1` pushes every window's title,
and the IPC replies include titles too. There is no way to opt out of
receiving them. Each source matches the title event and drops it, or leaves
the field out of what it parses. A new privacy test fails if any non-test
line in these sources mentions a title other than as an empty match arm. With
the user's agreement, the focused window's app id stands in for the process
name where there is no process id (COSMIC). It is mapped to a category and
dropped, as the process name is elsewhere.

What has run:

- **By hand on Pop!_OS 24.04, COSMIC, one 1920×1080 screen at 100%**
  (debug build, 2026-10-24): the layer overlay drew the pets transparently
  above every window. Clicks around a pet reached the window underneath. A
  pet could be clicked, dragged, and right-clicked for the bubble, which
  opened next to it and took typing. With the COSMIC source, pets perched on
  window edges and moved with a window when it was dragged. They ignored
  windows that were covered or on another workspace. A scan took about
  0.2 ms. COSMIC's geometry includes its server-side title bar (a 640×480 X
  window was reported as 640×516), which is the edge a pet should stand on.
- The live Wayland test (`wayland_session_uses_the_compositor_it_finds`)
  passed against that COSMIC session: idle time, layer-shell detection, and
  an XWayland window it opened being listed by COSMIC.
- Unit tests: the shared Wayland model (ordering, filtering, scaling,
  fullscreen, app-id names), the input boxes, and the Hyprland, sway and niri
  parsers against fixtures. **The fixtures were written from each
  compositor's documented format, not recorded from a running compositor.**
- In CI, `scripts/linux-display-tests.sh` now runs the Wayland test under
  headless sway twice: without `SWAYSOCK` (screen edges) and with it, where
  sway's IPC must list an XWayland window. This had not run in CI when this
  entry was written.

What has not run:

- Hyprland, niri, a real sway desktop, KDE Plasma, and any other layer-shell
  compositor. They should get the layer overlay, but only COSMIC has been
  seen.
- GNOME on Wayland since this change (it should behave as before, through
  XWayland), and window sources for KDE (would need a KWin script) and GNOME
  (would need a Shell extension). Both still use the screen edges.
- Fractional scaling or more than one monitor on Wayland. Compositor layouts
  are converted with the pet screen's scale only.
- The overlay covers panels and docks (`exclusive_zone -1`), and the work area
  GTK reports on Wayland is the whole screen. A pet's floor can therefore sit
  over a dock rather than above it.
- Transparent margins inside a sprite's box catch clicks on the layer overlay,
  because the input region is made of rectangles and not the sprite's alpha.
- CPU and RAM against the budget, packaging, and any soak.

## Linux adapter (2026-09-24)

`platform/linux/` replaces the stub for Linux. On X11 it reads
`_NET_CLIENT_LIST_STACKING`, window type and state, `_NET_FRAME_EXTENTS` and
`_GTK_FRAME_EXTENTS`, XScreenSaver idle time, the active window's process
(X-Resource client PID, else `_NET_WM_PID`, then `/proc/<pid>/exe` or `comm`)
and the pointer. It reads no titles, no key or button state, no command line
and no pixels. On Wayland it reports window edges as unavailable, so pets use
the screen edges. Idle time comes from `ext-idle-notify-v1`, or from GNOME's
`org.gnome.Mutter.IdleMonitor`. The foreground category is not reported, and
GDK is steered to XWayland so the overlay can be positioned and kept on top.
The privacy scan now also rejects `WM_NAME`, icon and visible names,
`/cmdline`, `/environ`, and X image, keymap, key-grab, record and XInput calls.

**Nothing here has run on a physical Linux desktop.** Everything below ran on
GitHub's `ubuntu-latest` runners (Ubuntu 24.04, x86_64) against virtual
display servers.

- **Automated, every PR:** `npm run check`, `npm test` and `npm run test:ui`
  pass on Linux. Twelve model tests (any OS) cover the stacking reversal,
  frame and GTK-shadow extents, which windows are skipped, Ledgeling's own overlay,
  fullscreen, process names and session detection. One of them builds the
  engine's surfaces from a synthetic GNOME-like desktop.
- **Live X11 test (Xvfb 1920×1080 + openbox), every PR:** real windows are
  listed front to back. A dock, a utility palette and an always-on-top,
  taskbar-skipping window with Ledgeling's PID are excluded. The top edge includes
  openbox's frame (left 1, top 20). Minimising removes a window and activating
  one brings it to the front. The foreground process is named from its
  executable, fullscreen is detected and so is its end, the cursor matches a
  pointer warp, and idle time is read. **Scanning 22 windows took a median of
  1.45 ms in a debug build** (budget 3 ms).
- **Live Wayland test (headless sway 1.9 + XWayland), every PR:** enumeration
  returns the Wayland message, and no foreground process is reported. Idle
  time from `ext-idle-notify-v1` grew from 0 to 2.5 s with no input, and
  `prepare_process` selects the X11 GDK backend.
- **The app itself, run once in CI** (temporary workflow, since removed):
  - Under Xvfb + openbox + picom (xrender), the transparent overlay composites
    over the desktop. The budgie perched on an xterm's openbox title bar
    rather than its client area.
  - A click on a window body under the overlay reached that window
    (click-through). With the pointer on a pet's opaque pixel, the overlay
    caught it; moving off made it click-through again.
  - A pet could be dragged and, on release, fell to the screen bottom. The
    release came from the overlay's pointer capture, since `mouse_down` is
    `None`.
  - A fullscreen xterm suspended the pets, and leaving fullscreen resumed them.
  - Scans took 0.36–0.58 ms with two windows.
  - Under headless sway, sway reported the overlay as an `xwayland` floating
    window at 0,0 1920×1080. It was transparent over a native Wayland terminal,
    with the pet on the screen bottom.
  - This run found, and the build now fixes, a GTK menu bar that Tauri
    attached to the overlay: a stray "Ledgeling" menu at the top-left of the screen.
- **Not verified:**
  - Any real distribution or desktop environment. GNOME/Mutter, KDE/KWin and
    Xfwm have not run it, whether on X11 or on Wayland.
  - The GNOME idle-monitor path, which has never been called. Also
    `ext-idle-notify` version 2: sway offered version 1.
  - Client-side-decorated GTK windows (`_GTK_FRAME_EXTENTS`), covered by unit
    tests only.
  - Whether the X-Resource PID or `_NET_WM_PID` answered: both gave the same PID
    in the test, so sandboxed (Flatpak) apps are untested.
  - Multiple monitors, HiDPI and fractional scaling.
  - Hovering or grabbing a pet under Wayland. XWayland only learns the pointer
    position over X windows, so over native Wayland windows the cursor is
    stale and pets probably cannot be picked up there. Headless sway had no
    pointer to try it with.
  - CPU and RAM against the budget. One reading from the CI runner (debug
    build, software rendering): the main process's RSS was 193 MB, which is
    not evidence of meeting the 80 MB ceiling.
  - Any soak.
  - The pet bubble on Linux. The desktop run predates it on this branch, so
    its transparency, placement and focus have not been seen under any X11
    or Wayland compositor. The adapter skips it like the overlay (Ledgeling's PID
    plus always-on-top or skip-taskbar); that is covered only by the model
    tests.
  - Linux packaging, which remains out of scope.

## Linux builds again (2026-09-24)

- The Linux fallback platform did not compile: `app.rs` constructs
  `NativePlatform` as a unit struct, which the macOS and Windows adapters are
  and the fallback was not. It now keeps its monitor cache in a static, like
  the Windows adapter, and the full Rust suite, UI tests, Clippy and the build
  pass on Linux. `Verify` now runs on `ubuntu-latest` too, so it stays that
  way.
- **What Linux was then:** the fallback adapter, with no windows, idle time,
  foreground app, cursor or fullscreen state. The Linux adapter below
  replaced it. No Linux bundle is built or released.

## One pet bubble on every platform (2026-09-24)

- The right-click speech bubble is now a Vue page in its own transparent,
  undecorated, always-on-top webview window, the same on macOS, Windows and
  Linux. It replaces the ~1,500-line Objective-C panel and the Windows
  stopgap that opened the dashboard instead. `native/pet_ui.m` keeps only the
  overlay panel, a helper that makes the bubble a non-activating key panel,
  and the Finder folder drag.
- The backend owns placement (`pet_ui::place`): above the pet with the tail
  down, below it with the tail up when the top of the work area is in the way,
  clamped on-screen with the tail still aimed at the pet. Five Rust tests
  cover the placement cases and the sprite-to-logical anchor conversion. The
  update loop re-places the bubble when the pet's window moves.
- Twenty-one UI tests (`src/bubble`) cover what each mode still needs before
  it can run, the start event, progress, approvals rendered from the summary
  and never the payload, undo, follow-ups, the model picker, Escape and
  focus-loss closes (including a stale close not closing a newer opening), and
  closing without a fade under reduced motion.
- Automated verification on Linux: the Rust suite, 62 UI tests, `vue-tsc`,
  the Vite build, Clippy and `cargo fmt --check`. Clippy also passes for the
  Windows target cross-compiled from Linux (`x86_64-pc-windows-gnu`), which
  compiles the Windows window-style code. The page was rendered in headless
  Chromium with a mocked backend, in light and dark themes and both tail
  positions.
- **Not verified:** nothing about this has run in the real app on any
  platform. In particular: the macOS panel conversion (the Objective-C was not
  compiled here), keyboard focus without activating Ledgeling on macOS, focus on
  Windows and whether the tool-window style survives showing, the hidden-caption
  handling on the bubble, transparency and the drop shadow in WKWebView and
  WebView2, the folder dialog not dismissing the bubble, and the bubble
  following a dragged window.

## Pets that stay with you, and readable approvals (2026-09-11)

- A pet is pinned while its bubble is open and while its own task is running or
  waiting on an answer, and plays the new optional `busy` / `attention` clips in
  the latter two cases. Covered by five pet tests: it stays put across two
  minutes of behavior ticks, a walking pet stops the moment its bubble opens, a
  mid-air pet keeps its locomotion clip until it lands, a task outranks and
  outlives the bubble, and a two-animation pack still draws through the fallback
  chain. Pack validation raises a note naming the fallback for an empty slot,
  and the packs panel shows what each slot resolves to.
- Every approval is turned into a headline, capped item list, note and button
  label before it is stored (`tasks::summary`), and the pet bubble and task
  history both render that instead of the producer's payload. Eight unit tests
  cover the file planner, Codex commands and file changes, Claude tool hooks and
  an unknown shape, all asserting no raw JSON reaches the text a user reads;
  a UI test asserts the same for the dashboard card.
- Automated verification: 118 Rust tests, 28 UI tests, `vue-tsc`, Clippy and
  `cargo fmt --check` pass on macOS, and the Objective-C bubble compiles.
- **Not verified by hand:** the bubble has not been opened on this Mac since
  these changes. The pinning, the released-on-close behaviour, the new clips
  playing, and the reworded approval panel have been driven only through the
  engine and UI tests. No approval from a live Codex or Claude runtime has been
  summarised — those two payload shapes are covered by fixtures written from
  the protocol and the bridge's own message, not from a recorded session.

## Compact companion redesign (2026-09-11)

- Replaced the scattered styles with shared colour, typography, spacing, card,
  and control tokens; see [UI system](ui-system.md). Added compact navigation,
  pet avatars, content-sized control cards, expandable pack tools, and aligned
  forms across companion pages.
- Added combined search/filter controls for companions, packs, and task history,
  plus scenario search in the Studio Test tab. History initially renders ten entries;
  additional entries are revealed explicitly.
- Task replies render sanitized Markdown, with long responses expanded inline.
  Tests verify formatting, link handling, and removal of active HTML, remote
  images, and unsafe URL schemes. Task mode follows the selected connection;
  built-in actions remain standard/local.
- Automated verification: 18 UI tests, 98 Rust tests (7 ignored), frontend
  build, Clippy, and Rust formatting pass on macOS.
- The supplied screenshots were used to identify the previous layout issues.
  Fresh live visual verification remains outstanding: no browser is connected
  to computer-use, and native Chrome access timed out again. This pass has no
  fresh screenshots, measured interaction latency, or accessibility audit.

## UI responsiveness pass (2026-09-11)

- Removed the extra snapshot round-trip before displaying action/settings
  responses. Tests cover stale polls, overlapping commands, failed actions,
  refresh on focus, listener cleanup, and keeping assistant drafts across tabs.
- Native contextual UI now opens, dismisses, switches modes, and lays out at
  its final geometry without animated window/control movement. Hover tracking
  follows resized controls. This Objective-C code compiled on macOS through
  the build/Clippy check.
- Companion typography, contrast, control feedback, and narrow-window wrapping
  were updated. Frontend build, Clippy, Rust tests (91 passing, 7 ignored), and
  UI tests (9 passing) pass.
- Visual and live interaction verification is still outstanding: the local
  preview server started, but computer-use had no connected browser and native
  Chrome access timed out. No screenshots or measured input latency were
  obtained. This does not re-certify the performance budget.

## Run by hand on this Mac

- The app launches, restores its pets from `settings.json`, enumerates real
  windows, and scans in **1.2–2.8 ms** with 9–11 windows open (budget: < 3 ms
  typical, < 8 ms ceiling).
- Built-in packs are written into the packs directory on first run and load.
- A deadlock between the render loop and the IPC command handlers was found and
  fixed by never emitting while the engine lock is held. Confirmed by the app
  continuing to tick while the companion window polls it once a second.
- Monitor enumeration must happen on the UI thread on macOS; doing it from the
  update loop silently returns nothing. Fixed and confirmed.
- A locked session reports no screens; Ledgeling now suspends and keeps its geometry
  instead of losing the world. Confirmed while the machine was locked.
- **A second display was plugged in (2026-09-09)** — the first real
  multi-display session, and it broke exactly the way the mock never could:
  pets duplicated across both screens and flashing while display-connect
  churn tore down and rebuilt every overlay, pets wandering off both screens
  and turning invisible (a mixed-DPI coordinate mismatch — tao scales each
  monitor by its own scale, the adapter scaled window coordinates by the
  primary's), and window-perching dead on the secondary. The single-screen
  fix (one designated pet screen, one persistent overlay that is moved rather
  than rebuilt) was driven by this session; all automated suites pass on it.
  What is **not** hand-verified yet: the *fixed* build running live — on this
  single-display machine at the time of writing (the external is unplugged),
  and on two displays once one is attached again. The dual-screen picker, the
  overlay relocation on a screen switch, and the unplug/replug round-trip of
  the pet screen have been exercised only through the engine tests, not by
  hand.
- **The pet screen was unplugged live (2026-09-10)** with the external picked:
  the overlay and the pets did fall back to the built-in display, but the
  pick itself stayed bound to the dead display, so the Desktop panel showed
  an empty picker plus a note reading as pets-in-limbo, and a climber standing
  outside the fallback screen's bounds could wedge motionless at its edge
  (found by the timeline test, not live). The pick is now rewritten to the
  screen pets actually fell back to, out-of-bounds pets are rehomed onto it,
  and the note is gone. The replug half — picking the external again — is
  still engine-test-only.

## Terminal mode (2026-10-10)

Built, unit-tested and exercised in part. **Not driven by hand on macOS inside
the bubble, nor at all on Windows or Linux.**

- Rust: 226 library tests (agent event parsing, the tracker following a file as it
  grows, command wrapping, pet folders surviving a restart), 7 privacy, clippy
  clean with and without the `coding` feature. Frontend: 135 tests, `vue-tsc`
  clean, production build splits xterm out of the main bundle.
- **Claude Code hooks, run for real:** with Claude Code 2.1.296,
  `claude -p … --settings <file>` fired `SessionStart`, `UserPromptSubmit`,
  `Stop` and `SessionEnd` through `ledgeling --agent-event`, and the recorded lines
  carried only the fields we keep. **Not observed:** `PreToolUse`/`PostToolUse`
  and `Notification` payloads (the `notification_type` and `message` fields are
  read as documented, not seen), and an interactive session.
- **Not verified:** typing into xterm inside the non-activating bubble panel on
  macOS, the wider bubble's placement, the terminal's focus and Escape handling
  there, and the pop-out window. Agent events on Windows are untested (the hook
  runs the app's own binary, so it should work, but it has not been run).

## Pack hardening for a gallery (2026-10-24)

Branch `feat/pack-hardening`. Nothing here has been run by hand in the app.

- **Run:** `cargo test` in `src-tauri` (274 lib tests plus the integration
  suites, all passing, 15 of them new in `packs::tests::hardening`), `cargo fmt
  --check`. They cover: archive entry-count, per-file and total-size limits with
  oversized data, traversal and stray entries, symlink entries, a PNG over the
  size limit refused from its header, behaviour and text bounds, semver ordering,
  update and downgrade, a refused install leaving the installed pack intact,
  byte-identical exports with the same SHA-256 after modification times change,
  each publish rule failing on its own, the four bundled pets passing the publish
  check, a snapshot of the event, signal, action and class names, and the JSON
  schema carrying the same limits as the code.
- **Online gate and contract (same branch).** `cargo test`: 286 lib tests and 10
  privacy tests passing; `vitest run` 164 passing; `vue-tsc --noEmit` clean. New
  tests cover: every scope off by default and an offline request opening no
  connection (checked against a local server), a scope reaching only its own
  hosts, a redirect off the list not followed, a response over the cap failing,
  switching a scope off stopping a download in flight, analytics refusing
  session-only consent, only remembered choices surviving a restart and a damaged
  consent file meaning off, a marketplace download installing and recording its
  source, a wrong hash installing nothing, an offline or error-status download
  being refused with its own code. `privacy.rs` now also checks that the HTTP
  client lives in one file, that `send` checks consent first, and that consent is
  not in `Settings`. Server hosts are now build-time variables
  (`LEDGELING_MARKETPLACE_HOSTS`, `LEDGELING_ANALYTICS_HOSTS`, wired to the
  `pre-alpha` and `alpha` GitHub Environments in `release.yml`); parsing and the
  unavailable-without-hosts behaviour are tested. **Not run:** the release
  workflow change has not run (the variables are not set anywhere yet), and no
  build has been made with real hosts, so nothing here has contacted a real server and TLS, real
  redirects and a real marketplace were never exercised; no Vue UI uses
  `net_set`, `gallery_install` or the new commands yet; the commands were not
  driven from the running app.
- **Not run:** `cargo clippy --all-targets -D warnings` is red on four
  pre-existing lints outside `packs` (`platform/handoff.rs`, `settings.rs`,
  `tasks/mod.rs` twice); none are in the pack code. The new commands
  `update_pack` and `validate_pack_for_publish` have no UI yet, so they have
  only been exercised through the Rust tests. A mid-copy I/O failure during
  install was not simulated; only the validation-failure and refused-update paths
  are tested. The schema is checked against the limits constants but no JSON
  Schema validator runs over the bundled packs.

## Path-filtered CI (2026-10-25)

`scripts/ci-changes.mjs` (6 vitest tests: Rust/pack/schema → matrix, Vue →
frontend only, docs → nothing, unknown or build-wide files → both, no earlier
run or a base missing from the history → both, nothing changed → nothing). The
`gh run list` lookups were run against this repository's real history and gave the
expected base commit, and the classifier was run on a real diff. The workflow
YAML parses. **Not run:** the workflow itself. The first run after merge will
compare against an older commit and, because `verify.yml` changed, run the full
matrix; the skipping paths (a Vue-only PR, a docs-only PR, an already-released
prod push) have not been seen in Actions yet, and `verify-result` has not been
required in branch protection.

## Local .env flow (2026-10-25)

`scripts/with-env.mjs` (4 vitest tests: parsing, shell precedence, no values
printed, exit code passed through) and a `privacy.rs` test that fails when
anything other than the two public host lists is read at compile time (checked
once by planting a fake `option_env!("LEDGELING_API_TOKEN")`, which it caught).
`.gitignore` was checked with `git check-ignore`. **Not run:** the loader on
Windows by hand (CI runs its tests there); `npm run dev:app` and
`npm run build:app` were not launched.

## Decor page, prop animations, shared Studio (2026-10-30)

Automated:
- Rust: the animation names (a closed list, duplicates dropped, a stranger noted
  with the valid names, a non-list refused by path); each animation moves only
  its own part and stays in range; opening the bundled Meadow and saving it in
  place keeps one copy in its own folder, while saving without "in place" is
  refused; a timeline where decorating a biome from the list brings its window
  forward and opens it for editing, and a closed app is explained.
- Vitest: the Decor page lists packs with their animations and use, offers a
  copy or the original for bundled decor, deletes your own only on a second
  click, decorates an open window and greys out a closed one; and Settings now
  links to the page.

**Not run:** the page, the animations and bringing a window forward have not
been seen in the app. Already-installed copies of Meadow keep their old
`decor.json` (bundled packs are only written when missing), so they have no
animations until edited or reinstalled.

## Studio Decor mode (2026-10-30)

Automated: a Rust test saves a decor pack from Studio, saves it again (it replaces
the first, and never shows up as a broken pet), and refuses a bad sway by naming
`props.tree.sway`. Vitest covers the draft model (unique prop names, the save
problems in plain words, resizing from the base, the manifest and sprite payload)
and the component (problems listed, then a painted prop saved through
`save_decor_pack` with the right pixel). Writing it found that the decor scan
did not skip dot folders, so a save in progress was mistaken for an old save; the
scan now skips them.

**Not run:** Decor mode has not been opened in the app; drawing feel, the ledge
preview and the canvas size were not seen. There is no export to a `.petpack`
for decor packs.

## Make room at the top (2026-10-30)

Automated: `room.rs` unit tests with the mock platform:
- a window flush with the top that has something on it is moved down just
  enough, once;
- nothing moves while the mouse is down or without permission;
- a window is put back ten seconds after nothing stands on it;
- a window the user moved is neither put back nor moved again;
- switching off puts windows back, and a refusal is remembered;
- a short window is never squeezed.

Timelines: off never touches a window; on, it moves a biome's window down by its
tallest prop and back when switched off; without permission nothing moves, pets
keep off that edge, the props are still drawn (only not perchable), and Decorate
mode says the window needs the Accessibility permission.

First hand-run (macOS, the user, 2026-10-30) found that an earlier version hid
every prop on a window against the menu bar while the permission was missing,
which looked like biomes not working at all. Props are now always drawn.

**Not run:**
- No window has actually been moved: the macOS Accessibility calls
  (`platform/macos_ax.rs`), the permission prompt, and the Windows `SetWindowPos`
  border maths have only been compiled (macOS) or not at all (Windows, by CI).
- How apps react to having their top edge moved (some snap back, some constrain
  their size) is unknown.
- A Windows maximised window keeps its maximised state while moved; untested.
- Restore-on-quit has not been seen to happen.

## Decorate mode (2026-10-30)

Automated: `npm test` 359 lib + 12 privacy + 29 timeline passing, `npm run
test:ui` 187 passing, `npm run check` clean. Tests cover adopting an app (same
app twice, the eight-biome limit, a path refused), placing (clamped, unknown pack
or prop refused, the sixteen-prop limit), moving, flipping, removing, the palette
order, a timeline from pick (empty space explained, then a Safari window) through
placing a bush a quarter along the edge, and the overlay: a pick sends its point,
a drop above the edge places at the right fraction, a drop elsewhere does
nothing, and move, flip, remove and Done each send one command.

**Not run:** Decorate mode has not been opened in the app. Whether the overlay
really receives every click while decorating (it is made fully interactive), the
tray's placement near the top of the screen, Escape reaching a non-focusable
overlay, and dark mode were not seen. There is no Decorate entry in the pet
bubble; it is in Settings → Biomes.

## Biomes in the engine (2026-10-30)

Automated: `npm test` 355 lib + 12 privacy + 28 timeline passing, `npm run
test:ui` 179 passing, `npm run check` clean. New tests cover:
- the decor format: defaults, every bad value named by path, backwards ledges,
  the prop limit, a missing sprite, and the bundled Meadow loading cleanly;
- the app resolver: only biome ids are cached, never names; each process is named
  once; nothing is named with no biomes;
- `sanitize`, and that prop owner ids stay clear of monitors;
- timelines:
  - a biome stands on its app's front window and moves when another of its
    windows comes forward;
  - a prop behind another window is neither drawn nor perchable;
  - biomes off draws nothing and names no process;
  - a squirrel sits in the oak and rides it when the window moves;
  - a pet with a home spends most of its settled time there;
- a privacy test: only a biome's own app name reaches `settings.json`, and only
  the platform adapters and the resolver ask for names.

**Not run:** nothing seen in the app. How the props look and sway, the overlay's
prop layer, the Home picker, and the Biomes settings were not opened. The Windows
`GetWindowThreadProcessId` change and the Linux pid plumbing were not compiled
here (macOS only); CI builds them. There is no way to make a biome yet other than
editing `settings.json` by hand; Decorate mode is the next PR.

## Squish and wobble (2026-10-30)

Automated: `npm test` 345 lib + 11 + 23 timeline passing, `npm run test:ui` 176
passing, `npm run check` clean. New tests cover the springs (settle, overshoot,
no overshoot when off, teleport snap, shake threshold by grip, bounded squash,
breathing), the optional `jelly` block (defaults, clamping, every bad value
named), the settings default, the switch in Settings, and three timelines: a
dragged window no longer moves the drawn pet more than 60 px in one frame (it
used to jump 224 px), a hard shake knocks the squirrel off but not the gecko,
and with the switch off nothing squashes, leans or falls off.

**Not run:** nothing here has been seen in the app. `npm run dev:app` was not
launched, so how the wobble *looks*, whether the grip values feel right on a
real drag, the reduce-motion path, and the frame rate while springs settle are
all unchecked. No CPU measurement was taken; springs add work only while one is
moving, but that is reasoning, not a number. Windows and Linux untried.

## Pet sprint (2026-10-24)

Baseline before the sprint: `npm test` 227 lib + 7 + 11 passing, `npm run test:ui`
136 passing. All hand runs below are macOS, one machine, run once.

- **Phase 1, the first minute.** fmt, clippy, `npm test` (229 lib, 14 timeline)
  and `npm run test:ui` (139) green. Hand-run once on macOS: settings.json moved
  aside, `npm run tauri dev`; the companion opened on Pets with the tip card and
  the squirrel was seen standing on the top edge of the frontmost window. The
  fall and greeting animation were not watched frame by frame; settings.json was
  restored afterwards.
- **Phase 2, pick up, throw, pet.** fmt, clippy, `npm test` (246 lib, 14
  timeline) and `npm run test:ui` (139) green. Hand-run once on macOS with
  synthesized mouse events (a small Swift CGEvent script, not a real hand): the
  squirrel was grabbed at the screen edge and let go while the pointer moved
  fast; it was seen released and on the floor about 1,200 px to the left, which
  fits a throw, but its dizzy clip was not caught in a screenshot. Petting was
  tried the same way and **not** confirmed by eye (the pets had wandered off the
  spot aimed at); it is covered by unit tests only. Budgie not hand-tried.
- **Phase 3, the cat.** fmt, clippy, `npm test` (248 lib, 15 timeline) and
  `npm run test:ui` (139) green. The cat loads from the embedded files with no
  errors or warnings, is written into an empty packs folder, and in a replayed
  layout hops a 100 px gap between two windows but never crosses a 200 px one.
  The same replay on the squirrel found a bug already on `main`: a pet walking
  to a window edge to hop was declared "surface lost" there and stayed frozen in
  `startled` for good; fixed in `Pet::ride` with a regression test. Hand run:
  the app was launched once with the cat and squirrel in settings and the cat
  folder was written into the packs directory, but the Mac session was locked
  so the screen captured black. A second launch later, with the screen
  unlocked, showed Miso standing on screen as a tabby cat (one screenshot); the
  hop, nap and fling were **not** watched by hand.
- **Phase 4, pets noticing each other.** fmt, clippy, `npm test` (258 lib, 18
  timeline; the `tasks::apple` port test failed once and passed on re-run, it
  is unrelated) and `npm run test:ui` (139) green. Unit tests cover
  `pet_distance` (infinite alone, own id ignored), `pet_nearby` hysteresis
  (140 px in, 200 px out), `move_toward_pet` (a cat reaches its peer on one
  window, does not hop a gap wider than 140 px and finishes at once, finishes at
  once with no peer), and two pets greeting through the engine. Hand run: the
  squirrel and cat were launched together once; no screenshot caught them
  greeting or visiting, so the pet-to-pet behaviour is **not** hand-verified.

- **Phase 5, remembering you.** fmt, clippy, `npm test` (263 lib, 19 timeline, 7
  privacy) and `npm run test:ui` (140) pass. Hand-run once on macOS (2026-10-24)
  with a seeded settings file: the Pets page showed the cat's name, "Together 5
  days" and an affection meter near 60%, and `settings.json` held the same bond
  after quitting. Not confirmed: a second launch reading it back by eye, the
  meter fill measured, a real midnight rollover, and the affectionate behaviors
  seen playing. Those are covered by unit and timeline tests only.

- **Phase 6, Studio.** fmt, clippy, `npm test` (266 lib, 19 timeline, 7 privacy)
  and `npm run test:ui` (144) pass. Covered by tests only: opening each bundled
  pack and a saved 48 px pack with pixels intact, the plain-words refusal of
  other sizes, `draftFromPack` reproducing the squirrel's states, reactions and
  frame counts, template copies getting a new id, and Export after save. Hand-run
  once on macOS (2026-10-24, clicks synthesized, screenshots viewed): the
  Start-from dropdown listed the four bundled pets; Miso loaded as "Copy of Miso
  the Cat" with its sprites; Save showed Export .petpack, which wrote a
  `.petpack`; and the saved copy's menu offered Edit in Studio, which reopened
  it with its frames. Not checked: re-importing the exported file, or a
  hand-drawn edit round trip. The live preview was not changed (Play/Pause
  already existed).

- **Performance, end of sprint (2026-10-24): over budget.** `scripts/benchmark.py
  600` on the release `.app` bundle, macOS, four pets out (squirrel, budgie,
  gecko, cat), all moving normally: CPU mean 2.57%, median 2.0%, p95 6.5%; peak
  RSS 83.3 MiB. The script counted one process, so WebKit helpers and
  WindowServer are not in these numbers. That misses the < 0.5% idle and < 2%
  moving ceilings, and RSS is just over 80 MiB with four pets. There is no
  earlier measurement of this build, so `main` was measured the same way (300 s
  each, same three pets): `main` CPU mean 1.98%, median 1.8%, p95 2.7%, peak RSS
  83.8 MiB; this branch 1.93%, 1.9%, 2.8%, 84.1 MiB. The sprint adds nothing
  measurable; **both are about 2% and over the idle ceiling, a gap that was
  already there and is not closed.** One pet on this branch: mean 2.34%, p95 6.2%,
  82.8 MiB (RSS over 80 MiB with one pet too). A single screenshot per run showed
  pets on screen; movement was not confirmed from it.

## Not verified

Stated plainly, because these are the difference between "built" and "shipped":

- **The task surface on the desktop.** The speech bubble (right-click a pet,
  now one webview window on every platform), typing into it without the
  dashboard activating, pet overlays and the bubble behaving as auxiliary
  panels across macOS Spaces, and the Finder folder handoff (drag a pet onto a
  folder, `.perch-handoff-*` file promise, Shift-drag to move normally) are
  implemented and covered by the automated suites where a GUI is not needed —
  but have **not been driven by hand on macOS or Windows**.
- **Live provider connections.** The opt-in tests in
  `tests/task_connections.rs` have not been run; no live call to Ollama,
  OpenAI, Anthropic, a Compatible endpoint, Codex, the Claude sidecar or Brave
  has been made from this build.
- **Apple Foundation Models — partly verified.** `sudo fm license` is accepted
  on this Mac (macOS 27.0, build 26A5425a), and `apple_foundation` in
  `tests/task_connections.rs` passes: `fm serve` starts, `/health` and
  `/v1/models` answer, a real completion runs and its envelope parses, in under
  3 seconds. What is **not** verified is a full multi-step task against real
  files through the UI — every run so far has been the single-step capability
  probe or a scripted transcript. Measured envelope drift is roughly 1 reply in
  5 (4/5 valid, then 7/7 on a second pass), which the retry in `model_task` is
  sized for; sustained behaviour over 24 steps has not been observed. Tool
  choice is visibly weak on this model (two unrelated requests both chose
  `duplicates`), and no soak has been run with a server resident. `npm run prepare:sidecar` has run on this
  machine, but the Node bridge has not been exercised end-to-end and the
  bundle-size cost has not been measured.
- **Windows — first run only.** On 2026-09-24 the build was compiled and run
  in dev mode on one Windows 11 Pro machine (one 1920×1080 display at 100%
  scale, taskbar auto-hidden). What was checked by hand: the pet renders,
  perches on window tops and the screen bottom, hovers into a clickable state,
  can be grabbed and dragged and falls when released; the auto-hidden taskbar
  still reveals; no title bar is painted over the screen. The full Rust suite
  passes on Windows. Not checked: the pet bubble (see the 2026-09-24 bubble
  section above), an installed NSIS build, window riding while a window is dragged,
  occlusion with overlapping windows beyond casual use, multiple monitors,
  scaling other than 100%, fullscreen suspension, the task layer against real
  files, CPU and RAM against the budget, and any soak. The Windows folder
  handoff (an OLE drag of a virtual `.perch-handoff-*` file, located through
  shell change notifications) is written and type-checked for the Windows
  target, and its path matching and cleanup are unit-tested, but it has **not
  been run on Windows at all**: whether Explorer accepts the drop, reports the
  file, and the pet keeps following the cursor during the drag are all untried.
- **Linux beyond one COSMIC desktop.** The Wayland overlay and the COSMIC
  window source have run by hand on one Pop!_OS COSMIC machine. X11 desktops,
  GNOME, KDE, Hyprland, sway and niri have run only in CI or not at all; see
  the two Linux sections above for exactly what was covered.
- **Multi-monitor on real hardware, post-fix.** The engine remains
  multi-monitor-capable and the timeline tests prove it, but the shipped
  product is single-screen by design. The fixed build has not been run by hand
  on two displays; the external display used to find the bugs is currently
  unplugged. Unplug/replug of the pet screen itself is replayed only in the
  timeline tests (the 2026-09-10 live unplug ran the build from before the
  pick-rewrite fix).
- **The performance budget.** Scan cost is measured and inside budget. Sustained
  CPU and RAM for the current build are **not** re-measured since the M0 renderer
  comparison; `scripts/benchmark.py` exists to do it and has not been run for ten
  minutes on this build.
- **The eight-hour soak.** `scripts/soak.py` exists and has not been run.
- **Signing and notarisation.** No certificate is available. Bundles are unsigned.
- **A non-programmer following the authoring guide.** `docs/pack-authoring.md` is
  written for that test and has not been given to an actual non-programmer.
- **Public release.** Nothing has been published anywhere.

## 2026-10-24 memory measurement (macOS)

Physical footprint per process from `python3 scripts/benchmark.py`
(`footprint_by_process`), release build, macOS, one pet idle. Each figure is a
single 90 s run; noise is about ±1 MiB. The process-tree finder launches with
`open -n`, so the WebKit helpers are attributed to the app through their
responsible pid.

| Case | Total MiB | Breakdown |
|---|---|---|
| Before (companion hidden at launch) | 171.4 | app 24.0, GPU 63.5, WebContent 35.6 + 43.1, Networking 5.2 |
| After lazy companion (no companion window) | 124.6-125.4 | app ~19, GPU ~60, overlay WebContent ~40, Networking ~5 |
| Companion open | 177.3 (+52.7) | |
| Companion hidden after close | 206.8 | held until the window is destroyed |
| Zero-pet floor | 74.5 | WebKit floor for the transparent overlay |

Variants tried on the overlay, none of which helps: a small overlay box leaves
the GPU process unchanged (60.0 MiB) and only drops WebContent by about 20 MiB;
a 1x1 background-image costs the same as the sprite (GPU 60.0 vs 61.1); CSS and
canvas drawing variants made no difference.

The old "83 MiB" figure was the Rust process RSS only, not the process tree.
Windows and Linux were **not** measured. The destroy grace is now 60 s (it was
10 minutes while measuring above); see `docs/development.md`.

Hand-run once on macOS (2026-10-24, release build, synthesized clicks, one pet):
launch 126.5 MiB with 4 processes (app, GPU, Networking, one WebContent); companion
opened from the tray 179.5 MiB; after typing an unsent assistant prompt, going to
Pets and closing it, still 220 MiB 30 s later (hidden, not freed); destroyed about
65 s after the close, leaving 135 MiB with 4 processes; `ui_state.json` held the
Pets page and the prompt. Reopening after the destroy restored the Pets page and
the prompt text; closing and reopening within 12 s reused the same WebContent
process. Not checked: the Studio draft restore, the prompt's first letter (stored
and restored as `Draft-xyz-123`, probably the composer capitalising it), and why
the footprint after destroy (135 MiB) is about 10 MiB above the never-opened
figure (125 MiB).
