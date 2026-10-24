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
  plus scenario search in Pet Lab. History initially renders ten entries;
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
- **Linux on a real desktop.** The adapter and the app have run only under
  virtual display servers in CI; see the Linux adapter section above for
  exactly what that covered.
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
