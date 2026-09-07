# Verification status

Updated after the M1–M4 implementation pass. This file says what has actually
been run, and — more importantly — what has not.

## Automated, run on every change

| Check | Result |
|---|---|
| `npm test` (`cargo test`) | 74 tests pass: 58 unit, 9 integration timeline, 7 privacy |
| `cargo clippy --all-targets -- -D warnings` | passes |
| `cargo fmt --check` | passes |
| `npm run build` (`vue-tsc` + Vite) | passes |

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
  screen-capture, key-state or network API, if a network or telemetry crate is in
  `Cargo.toml`, or if the Tauri capability list grants anything beyond local
  window events.

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

## Not verified

Stated plainly, because these are the difference between "built" and "shipped":

- **Windows.** The Win32 adapter is written to the APIs the architecture doc
  names — `EnumWindows`, `DWMWA_EXTENDED_FRAME_BOUNDS`, `DWMWA_CLOAKED`,
  `GetLastInputInfo`, `QueryFullProcessImageNameW` — but it has never been
  compiled or run, because this is a Mac. Treat it as unreviewed code.
- **Multi-monitor on real hardware.** Correct in the mock-platform timeline; this
  machine has one display.
- **The performance budget.** Scan cost is measured and inside budget. Sustained
  CPU and RAM for the current build are **not** re-measured since the M0 renderer
  comparison; `scripts/benchmark.py` exists to do it and has not been run for ten
  minutes on this build.
- **The eight-hour soak.** `scripts/soak.py` exists and has not been run.
- **Signing and notarisation.** No certificate is available. Bundles are unsigned.
- **A non-programmer following the authoring guide.** `docs/pack-authoring.md` is
  written for that test and has not been given to an actual non-programmer.
- **Public release.** Nothing has been published anywhere.
