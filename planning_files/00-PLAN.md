# Perch — Plan & MVP Status

One file, replacing the original seven planning documents. Each section keeps the
original document's intent and records **what was actually built to get the MVP
running on macOS**. The honest, always-current account of what has and has not
been verified lives in [`docs/verification.md`](../docs/verification.md) — this
file summarizes; that one is the record.

---

## 1. Product brief

**The idea.** A desktop companion that lives on the edges of your real windows,
moves like the animal it actually is, and quietly notices how your session is
going. Not a productivity tracker, not a mascot skin engine. Never reads window
titles, screen contents, or keystrokes; nothing leaves the machine in v1.

**v1 scope (as planned):** real window-edge awareness, two shipped species
proving two locomotion classes, a data-only pet pack format, local-only session
awareness. Windows first, architecture kept portable.

**Done in the MVP.** Everything in the v1 scope exists as code and runs, on
macOS rather than Windows: three bundled species (budgie, squirrel, gecko) that
differ only in data; the full pack format; the awareness layer; settings, tray,
and a companion window. Monetization (itch.io first, paid packs later) and
success metrics (500 downloads, three community packs, 2h median session)
remain entirely future work — nothing has shipped.

**Still open:** ship it. See §6.

---

## 2. Technical architecture

**The plan.** Tauri 2 + Rust backend owning all truth (world model, pet state,
behavior); webview as a dumb renderer. All OS calls behind a `Platform` trait
so macOS/Linux can be added later. Physical pixels in virtual-desktop space,
converted to logical only at the Tauri boundary. Rendering model to be decided
by the M0 spike: per-pet window vs full overlay.

**Done in the MVP.**

- **Rendering decision made and recorded** ([`docs/rendering-decision.md`](../docs/rendering-decision.md),
  including the M1 addendum): **one transparent overlay per monitor**. The
  per-pet-window spike cost too much native CPU; the `PERCH_RENDERER=small`
  variant was later removed entirely. Pets are drawn as DOM sprites in the
  overlay; click-through is toggled per-monitor by alpha hit-testing against
  the pack's real alpha masks.
- **`Platform` trait** (`src-tauri/src/platform/mod.rs`) with a working
  **geometry-only macOS adapter** (`macos.rs`) — CoreGraphics window bounds,
  layers, alpha, owner PIDs via `proc_name`, input idle, cursor, mouse-down.
  A **Win32 adapter** (`windows.rs`) is written to the planned APIs
  (`EnumWindows`, `DWMWA_EXTENDED_FRAME_BOUNDS`, `DWMWA_CLOAKED`,
  `GetLastInputInfo`, `QueryFullProcessImageNameW`) but **has never been
  compiled or run** — this is a Mac. Treat it as unreviewed.
- **`engine.rs` is the headless core**: world scan, pets, awareness, packs with
  no Tauri. Timeline tests drive it through a mock `Platform`, which is how
  "the user dragged a window while the squirrel was climbing it" is repeatable.
- **Surface graph** (`world/`): all four window edges with front-to-back
  z-order occlusion subtraction, permanent monitor/taskbar surfaces,
  `CONTIGUOUS`/`JUMPABLE`/`AIR` edges, minimum-segment rejection, and a
  tracker diffing `Added`/`Lost`/`Moved` (fast moves raise `startle`).
  Pets hold `(SurfaceId, t)` and ride windows for free.
- **Scan cadence**: 2 Hz baseline, 10 Hz for 2 s after any change — as planned.
  Measured 1.2–2.8 ms with 9–11 windows on this Mac (budget < 3 ms).
- **Deviation from plan:** macOS, not Windows, is the only verified platform;
  mixed-DPI is exercised only through the mock (single display here). The
  Windows-first roadmap claim is currently inverted — decide which way to
  resolve before release (see §6).

**Performance budget** (a ceiling, not a goal): < 0.5% CPU idle, < 2% moving,
~0% suspended, < 80 MB RAM per pet, scan < 3 ms. Scan cost is measured and
inside budget; the rest was certified at M0 and **has not been re-measured on
this build** (`scripts/benchmark.py` exists for exactly this).

---

## 3. Pet pack spec

**The plan.** Data, never code. A person who can draw and edit JSON but cannot
program ships a working pet in under an hour. Minimal pack = two sprite strips
and ~20 lines of JSON; optional animations fall back down a chain
(`climb → move → idle`); errors name file, JSON path, problem, and expectation —
never a bare stack trace. Four locomotion classes (`flier`, `climber`, `walker`,
`hopper`); one graph, different traversal rulesets.

**Done in the MVP.** The spec is implemented essentially as written and is
tested at every point it cares about:

- Manifest v1 parsing with the full fallback chain; unknown fields load with an
  INFO note; missing required fields, broken JSON, quoted frame counts, unknown
  classes/surfaces/actions/events, bad conditions and undeclared states each
  produce their own specific message — a property test asserts **no diagnostic
  is ever a raw serde error**.
- The condition grammar (`signal OP value` joined by `&&`/`||` over a closed
  signal namespace) is validated and evaluated with no code-execution path.
- Declarative FSM: states, durations, weighted transitions, built-in actions,
  reactions by priority with cooldowns, personality scalars.
- `.petpack` install by drag-drop, pack browser with previews, spawn/despawn,
  scaffolded "New Pack" per locomotion class, JSON Schema at
  `schema/pet-pack-v1.schema.json`, Pet Lab with live reload and the debug
  overlay (surface graph, paths, labels).
- **Three species ship as pure data** — budgie (flier), squirrel (climber),
  gecko (vertical-only climber, no jump) — the gecko being the spec's own
  acceptance test: zero engine changes.
- One addition beyond the original spec: sprite strips commonly export with
  transparent rows under the feet, so the loader now records each clip's
  ground line from the alpha channel and anchors to the visible feet rather
  than the texture edge — otherwise every pack floats a few pixels above its
  surface. Authors get this for free; no manifest change.

**Still open:** the acceptance test's human half — no actual non-programmer
has followed [`docs/pack-authoring.md`](../docs/pack-authoring.md) yet, and no
outside author has shipped a pack.

---

## 4. Awareness & behavior

**The plan.** Observe, don't interrogate: idle time, foreground app *category*,
unbroken-session length feed two slow scalars (energy, mood) that bias behavior.
Nudges are body language only — no popups, toasts, text, or focus steals;
three tiers, cooldowns, escalation caps, suppressed in `game`/`media`,
fully disableable. Titles are never read.

**Done in the MVP.** All of it, and the privacy constraint is load-bearing in
the test suite:

- Idle seconds polled via the platform layer; the four idle states, breaks
  (≥ 5 m) and micro-breaks derived from them.
- Foreground category from the **process name only** (`proc_name` on macOS →
  `AppCategory`), debounced, with a user-editable category map in settings.
  The **privacy tests fail the build** if any source file references a
  window-title, screen-capture, key-state or network API, if a network crate
  enters `Cargo.toml`, or if Tauri capabilities grow beyond local window events.
- Energy and mood on a minutes timescale, exposed to packs as read-only signals;
  nudge ladder with the documented thresholds and hard rules, suppressed in
  game/media, disableable wholesale; a plain-language privacy screen in the
  companion window ([`docs/privacy.md`](../docs/privacy.md)).
- Reactions pipeline with the priority bands (survival > direct > social >
  nudge > ambient > default), plus sounds per state when enabled.

**Still open:** nothing structural. The open questions from the original doc
(remembering across days, multi-monitor attention, meeting detection,
first-run pacing) remain deliberately unanswered.

---

## 5. User stories / backlog

**The plan.** M0 spike → M1 core engine → M2 packs → M3 awareness → M4 ship,
as hard gates. Deferred list stays out until M4 ships.

**Done in the MVP.** M0–M3 fully, including their acceptance criteria; M4 the
parts code can do:

| Milestone | Status |
|---|---|
| M0 — spike, overlay, enumeration, hit-testing | ✅ Done; decision recorded |
| M1 — surface graph, all locomotion classes, riding, recovery, multi-monitor, fullscreen suspend | ✅ Done (multi-monitor mock-verified only) |
| M2 — manifest, validator, FSM, install, Pet Lab, scaffold, schema, species | ✅ Done; the gecko proved the abstraction |
| M3 — idle, categories, energy/mood, sessions, nudge ladder, privacy screen | ✅ Done |
| M4-1 settings/persistence, M4-2 tray/lifecycle/autostart | ✅ Done |
| M4-3 installer & signing · M4-4 soak · M4-5 authoring-guide test · M4-6 release | ❌ Not done |

77 tests cover it (61 unit, 7 privacy, 9 timeline integration), clippy and fmt
clean on every change; `docs/verification.md` is the item-by-item account.

---

## 6. What's left before release

In the order the plan itself suggests:

1. **Stability soak (M4-4)** — `scripts/soak.py` exists, never run. Eight
   hours: memory drift, sleep/wake, lock/unlock, monitor changes.
2. **Performance re-certification** — ten minutes of
   `scripts/benchmark.py` on this build for CPU/RAM, not just scan cost.
3. **Multi-monitor on real hardware** — the timeline tests pass; a second
   display has never been plugged in.
4. **Authoring guide test (M4-5, also the M2 exit criterion)** — hand
   `docs/pack-authoring.md` to an actual non-programmer; they ship a pet in
   under an hour or the format needs work, not them.
5. **Platform decision** — the brief said Windows-first, reality is
   macOS-only. Either verify `platform/windows.rs` on a Windows machine or
   update the plan to ship macOS v1 (notarised `.dmg` instead of MSI/NSIS).
   Resolving this changes what "installer & signing" means.
6. **Installer & signing (M4-3)** — unsigned bundles today.
7. **Public release (M4-6)** — itch.io page, GitHub release, demo GIF,
   devlog. Exit criterion: someone you don't know is using it.

## 7. Deliberately deferred (unchanged)

Nothing here enters the repo until M4 ships publicly: macOS/Linux parity
beyond what exists, pets noticing each other, more species (cat, moth, bat,
frog), sound design pass, AI layer, pet actions, meeting detection,
notification interception, pack marketplace.