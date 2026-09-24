# Verification status

Updated after the M1–M4 implementation pass, the task/assistant layer
(`docs/assistant.md`), and the single-screen multi-display fix (2026-09-09,
see `docs/rendering-decision.md`'s single-screen addendum). This file says
what has actually been run, and — more importantly — what has not.

## Automated, run on every change

| Check | Result |
|---|---|
| `npm test` (`cargo test`) | 122 tests pass: 105 unit, 10 integration timeline, 7 privacy; plus 11 ignored (1 local-Ollama, 9 credential-gated connections, 1 Apple on-device) |
| `cargo clippy -- -D warnings` (in `npm run check`) | passes |
| `cargo fmt --check` | passes |
| `npm run build` (`vue-tsc` + Vite) | passes |
| `npm run test:ui` (vitest) | 28 tests pass (approvals and their plain-words summaries, refresh timing, drafts, collection search/filtering, assistant clip slots in the packs panel, history expansion, automatic task mode, and sanitized Markdown) |

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
  passes on Windows. Not checked: right-click opening the dashboard on the
  Assistant (written, not yet tried), an installed NSIS build, window riding while a window is dragged,
  occlusion with overlapping windows beyond casual use, multiple monitors,
  scaling other than 100%, fullscreen suspension, the task layer against real
  files, CPU and RAM against the budget, and any soak. There is no pet bubble
  on Windows, and folder drag-and-drop is macOS-only.
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
