# Ledgeling

A little company for your desktop. A bird, a squirrel, a gecko or a cat lives on the
edges of your open windows, notices roughly how your session is going, and never
sends anything anywhere.

Vue 3 · TypeScript · Tailwind 4 · Tauri 2 · Rust.

## Run it

Requirements: Node.js 22.12+ (CI runs on Node 24), Rust stable, and the native
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS,
install the Xcode Command Line Tools.

```sh
npm install
npm run tauri dev
```

The companion window opens and a transparent overlay appears on each display.
On a first launch a squirrel drops onto the window you are looking at; open
**Pets** to bring out the budgie, the gecko or the cat as well.

- **Drag** a pet to pick it up; let go and it finds its way back. Fling it and it
  arcs through the air, bounces off the screen edge and lands a little dizzy.
- **Rub** the pointer back and forth over a pet to pet it.
- **Click** a pet to say hello.
- **Home** shows who is out, with pause, hide and a three-step pace control.
- **Pets** brings pets out and sends them home, filters them by status, movement,
  maker, source and tag. **Make your own pet** opens the Pet Studio: a pixel
  editor for every animation and block-style behaviour editing, saved as an
  ordinary pack. Drop a `.petpack` file or a pack folder onto the window to install it.
- **Assistant** takes a request; **History** lists past ones with status, date,
  pet, kind and provider filters.
- **Settings** holds appearance, sounds, awareness, break reminders, login and the
  plain-language privacy summary. Its **Advanced** section leads to the desktop
  map and awareness details (reminder timing, app categories).
- **Pet Studio** (Pets → Make your own pet) is where pets are authored: **Look**,
  **Behaviour**, and **Test** — live reload, a validator, scenario buttons and new pack folders.
- Cmd/Ctrl+Shift+H shows or hides the pets, Cmd/Ctrl+Shift+P pauses, Cmd/Ctrl+1
  reopens the companion window. Closing that window leaves the pets running;
  quit from the menu-bar icon.

## Build and check

```sh
npm run build      # type-check and build the frontend
npm test           # 149 Rust tests: 132 unit, 10 timeline, 7 privacy
npm run test:ui    # 62 UI vitest tests
npm run check      # frontend build plus Clippy with warnings denied
npm run tauri build
```

On macOS the bundle lands in `src-tauri/target/release/bundle/macos/Ledgeling.app`.
It is an unsigned local build, not a notarised release. On Windows,
`src-tauri/tauri.windows.conf.json` switches the bundle to a per-user NSIS
installer and bundles `node.exe` for the Claude bridge. With it, `npm run tauri
dev` works on Windows after `npm run prepare:sidecar`; a local installer build
has not been tried yet.

## What is built

Everything in milestones M0–M4 that is code:

- **The surface graph.** Every visible window edge — top, bottom, left, right —
  with z-order occlusion subtracted, plus monitor edges and the taskbar/dock line,
  connected as contiguous and jumpable edges and diffed between scans.
- **Four locomotion classes.** `flier` ignores the graph and flies; `climber` A\*s
  over contiguous and jumpable edges, rounds corners, leaps up to `max_gap` and
  falls when its surface goes; `walker` uses connected horizontal edges only;
  `hopper` is a walker with short jumps.
- **Pets ride windows.** A pet holds `(surface, t)`, so dragging the window it is
  standing on carries it along, and a fast drag startles it.
- **Recovery.** A closed window makes a flier take off and a climber fall to the
  first surface below. A pet is never left floating, off-screen, or on NaN.
- **Multiple monitors,** including negative coordinates and mixed DPI, with an
  overlay per display. Unplugging one does not strand anybody.
- **Suspend.** A fullscreen app, a locked session or a sleeping display suspends
  scanning and rendering.
- **Pet packs.** The full v1 manifest, a validator whose every message names the
  file, the JSON path, the problem and the expectation, a declarative behavior
  machine with a non-Turing-complete condition grammar, `.petpack` install and
  export, a pack browser, scaffolding, live reload, a debug overlay and scenario
  buttons.
- **Four species, no engine changes between them.** Budgie (flier, window tops),
  squirrel (climber, everything, leaps 180 px), cat (hopper, window tops and the
  floor, hops 140 px), gecko (climber, window sides only,
  two sprite strips and under forty lines of JSON — the format's own acceptance
  test).
- **Awareness.** Idle detection, process-name→category mapping with a 5 s
  debounce, energy and mood on a minutes-long timescale, session and break
  tracking, and a three-tier nudge ladder that is body language only.
- **Settings and lifecycle.** Persistence with corrupt-file recovery, a tray icon,
  opt-in launch at login, and a clean quit.

## What is not

- **Windows has only had a first run.** It has been compiled and run by hand in
  dev mode on one Windows 11 machine at 100% DPI; see `docs/verification.md`
  ("Windows — first run only") for the exact record of what was and was not checked.
  The NSIS installer, pet bubble on hardware, window riding while dragging,
  occlusion, multi-monitor, scaling ≠ 100%, fullscreen suspension, task layer
  against real files, and Windows folder handoff remain unverified on hardware.
- **No signing or notarisation.** No certificate is available, so bundles are
  unsigned. On Windows, SmartScreen will warn about an unknown publisher until the
  installer builds reputation or is signed; some antivirus tools flag unsigned
  Rust binaries that create always-on-top windows. Both are expected for an
  unsigned build; the fix is a certificate, not a code change.
- **The performance budget is not re-certified** for this build, and the
  eight-hour soak has not been run. `scripts/benchmark.py` and `scripts/soak.py`
  are there to do both.
- **Nobody outside the project has shipped a pack yet,** and the authoring guide
  has not been tested on an actual non-programmer. Both are M2/M4 exit criteria
  that need a second person.
- **No release has been published.**

`docs/verification.md` is the full account of what has and has not been run.

## How it is put together

Rust owns all truth. The webview draws a sprite at a position it was told, and
sends pointer events back; it never decides where a pet is.

| Path | What lives there |
|---|---|
| `src-tauri/src/world/` | Geometry, surface extraction with occlusion, the traversal graph, scan diffing |
| `src-tauri/src/pet/` | Pet runtime, per-class locomotion, A\* pathfinding, the behavior machine |
| `src-tauri/src/packs/` | Manifest reading, human-readable validation, the condition grammar, install and scaffolding |
| `src-tauri/src/awareness/` | Idle and session tracking, process→category mapping, the nudge ladder |
| `src-tauri/src/platform/` | Every OS call, behind one trait, with macOS, Windows and mock implementations |
| `src-tauri/src/engine.rs` | The headless core: no Tauri, no windows, fully testable |
| `src-tauri/src/app.rs` | Tauri wiring: windows, commands, menus, the update loop |
| `src/` | The companion UI and the overlay renderer |
| `packs/` | The four bundled first-party packs |
| `schema/` | The published JSON Schema for `pet.json` |

Coordinates are physical pixels in virtual-desktop space throughout, converted to
logical only at the window API boundary.

## Making a pet

Read [docs/pack-authoring.md](docs/pack-authoring.md). It is written for someone
who can draw and edit JSON but cannot program, and the whole format is designed
around that person finishing in under an hour.

`python3 scripts/make_packs.py` redraws the bundled sprite art (needs Pillow).

## Privacy

Nothing leaves your machine, and there is no code in the build that could send it.
Window titles are never read. See [docs/privacy.md](docs/privacy.md) — and note
that `src-tauri/tests/privacy.rs` fails the build if any of that stops being true.

## License

Ledgeling is source-available, not open source; see [LICENSE.txt](LICENSE.txt)
for what you may do with it.

Contributions are accepted under the [Contributor License Agreement](CLA.md).

## Environment variables

- `LEDGELING_DIAGNOSTICS=1` prints a scan/state summary to stderr every five seconds
  (`PERCH_DIAGNOSTICS=1` is supported for backwards compatibility).
