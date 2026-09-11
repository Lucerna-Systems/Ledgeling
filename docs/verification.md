# Verification status

Updated after the M1–M4 implementation pass, the task/assistant layer
(`docs/assistant.md`), and the single-screen multi-display fix (2026-09-09,
see `docs/rendering-decision.md`'s single-screen addendum). This file says
what has actually been run, and — more importantly — what has not.

## Automated, run on every change

| Check | Result |
|---|---|
| `npm test` (`cargo test`) | 98 tests pass: 81 unit, 10 integration timeline, 7 privacy; plus 7 ignored (1 local-Ollama, 6 credential-gated connections) |
| `cargo clippy -- -D warnings` (in `npm run check`) | passes |
| `cargo fmt --check` | passes |
| `npm run build` (`vue-tsc` + Vite) | passes |
| `npm run test:ui` (vitest) | 18 tests pass (approvals, refresh timing, drafts, collection search/filtering, history expansion, automatic task mode, and sanitized Markdown) |

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
  `tasks/commands.rs`) references a network API; if a telemetry or frontend
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
- A locked session reports no screens; Perch now suspends and keeps its geometry
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

## Not verified

Stated plainly, because these are the difference between "built" and "shipped":

- **The task surface on the desktop.** The native speech bubble (right-click a
  pet), typing into it without the dashboard activating, pet overlays and the
  bubble behaving as auxiliary panels across macOS Spaces, and the Finder
  folder handoff (drag a pet onto a folder, `.perch-handoff-*` file promise,
  Shift-drag to move normally) are implemented and covered by the automated
  suites where a GUI is not needed — but have **not been driven by hand on
  this Mac**.
- **Live provider connections.** The opt-in tests in
  `tests/task_connections.rs` have not been run; no live call to Ollama,
  OpenAI, Anthropic, a Compatible endpoint, Codex, the Claude sidecar or Brave
  has been made from this build. `npm run prepare:sidecar` has run on this
  machine, but the Node bridge has not been exercised end-to-end and the
  bundle-size cost has not been measured.
- **Windows.** The Win32 adapter is written to the APIs the architecture doc
  names — `EnumWindows`, `DWMWA_EXTENDED_FRAME_BOUNDS`, `DWMWA_CLOAKED`,
  `GetLastInputInfo`, `QueryFullProcessImageNameW` — but it has never been
  compiled or run, because this is a Mac. Treat it as unreviewed code.
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
