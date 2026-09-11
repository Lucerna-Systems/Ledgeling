# M0 rendering decision — macOS MVP

Decision date: 2026-09-06. Choose the **primary-display overlay** as the MVP default. Keep a `PetSurface` boundary and retain the small-window implementation behind `PERCH_RENDERER=small`.

The initial 128×128 window test moves a transparent, borderless, always-on-top Tauri window on a fixed path at 60 Hz. Both variants receive identical 60 Hz events and render the same four-frame sprite at 12 fps. In the overlay variant, native position stays fixed and the sprite position changes inside the webview.

Both corrected runs completed with the release frontend embedded. An earlier direct Cargo build without `tauri/custom-protocol` was stopped and discarded.

| Model | Duration | Mean native CPU | Median native CPU | Peak native RSS |
|---|---:|---:|---:|---:|
| small | 600 s | 12.77% | 12.90% | 115.3 MiB |
| overlay | 600 s | 3.48% | 4.10% | 108.2 MiB |

The overlay reduced measured native CPU by approximately 73%. It is the better model for this machine. **The complete product performance budget is not yet proven**: the overlay mean exceeds the 2% target, some samples exceed the 4% ceiling, and helper-process costs were not included. This is a working MVP, not a performance-qualified release. The optimized production loop should be measured separately before M0 performance acceptance is signed off.

The bird's PNG strip was visually inspected in the native release window. The companion UI and its native connection were also inspected. Full overlay capture is unreliable in the available UI automation tool; that limits claims about visually assessed movement smoothness and click-through on other apps.

## Boundaries

- This benchmark measures the native Perch process, not its WebKit helpers or WindowServer. It is a rendering-model comparison, not proof of the full product CPU/RAM ceiling.
- This is a ten-minute run for each approach on one development Mac while other development work runs. No Windows, mixed-DPI multi-monitor, eight-hour stability, or battery claim is made.
- The production renderer emits only changed frames. It ticks at 60 Hz during flight/drag and 30 Hz while perched. Scans run at 2 Hz, briefly increasing to 10 Hz after geometry changes. Paused/hidden state skips scanning and uses a 200 ms control wakeup.
- The initial Mac MVP intentionally stays on the primary display. CoreGraphics point coordinates are multiplied by that display's scale to produce Rust physical pixels; conversion back to logical coordinates happens in `windows_mgr.rs`. Secondary-display traversal is an M1 gate.
- The platform adapter uses only geometry dictionary keys. CoreGraphics may return less geometry under OS restrictions; enumeration errors are surfaced, with a permanent desktop-edge fallback.
- Debug lines use the same transparent overlay. All transparent sprite pixels pass through; interaction is enabled only where the current sprite's pixel alpha is nonzero, using the actual generated alpha masks.

## Sources checked during implementation

- [Tauri Vite configuration](https://v2.tauri.app/start/frontend/vite/)
- [Tauri window builder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)
- [Apple window geometry API](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:))

---

## M1 addendum — one overlay per monitor

> **Superseded by the single-screen addendum below** — kept as the record of
> how the multi-monitor renderer was built and why it was replaced.

The M0 comparison chose the overlay. M1 needed the pet to use more than the
primary display, and the overlay had to change shape to allow it: instead of a
single window over the primary display, Perch now creates **one transparent
overlay per monitor**, each covering that monitor and drawing in that monitor's
own logical coordinates.

Why this rather than one window spanning the whole virtual desktop:

- Mixed DPI stays correct without any per-pixel fudging. Each overlay converts
  physical to logical using its own scale factor, which is the boundary the
  architecture doc already asked for.
- Click-through is decided per monitor, so a pet on the left screen does not make
  the right screen interactive.
- A monitor being unplugged closes one window rather than resizing everything.

What did not change: no window is repositioned per frame, so the measurement that
drove the M0 decision still holds. Pets moving between monitors are simply drawn
by a different overlay; the Rust side does not care, because it works in
virtual-desktop physical pixels throughout.

The `PERCH_RENDERER=small` variant has been **removed**. It existed to answer the
M0 question, that question is answered, and keeping an unused renderer that could
not handle multiple monitors was a liability rather than an option. `PetSurface`
remains the boundary, so a different renderer can still be dropped in.

`scripts/benchmark.py` was rewritten accordingly: it no longer selects a renderer,
and it now measures the whole process tree by default rather than only the Rust
process, because the budget is about what the machine pays.

## Single-screen addendum — one overlay, on the designated pet screen

Decision date: 2026-09-09, after the first real multi-display session on this
Mac (1080p external at scale 1 as main, MacBook built-in at scale 2). That
session found the per-monitor model broken in practice, and the product answer
is smaller, not bigger: **pets live on exactly one user-chosen screen** — the
"pet workstation" — and there is exactly one overlay, on that screen.

What the multi-display session actually exposed:

- **A coordinate-space mismatch on mixed DPI.** tao reports each monitor's
  geometry as CoreGraphics points scaled by *that monitor's* scale, while the
  macOS adapter scaled window and cursor coordinates by the *primary* monitor's
  scale. With a scale-1 primary and a scale-2 secondary, every window on the
  secondary landed at half the coordinates the monitor list claimed. Window
  edges on the secondary fell outside its work area and were clipped away
  (pets there could walk screen edges but never perch on a window), and the
  "gap" between claimed and real geometry was a region where pets were drawn
  outside any overlay — invisible.
- **A rebuild storm.** Any change to the monitor list — including transient
  states while a display negotiates its connection, and index reshuffles that
  change nothing — tore down and recreated every overlay. Closing a webview is
  asynchronous, so during a storm several generations of overlay coexisted,
  each still showing its last painted scene: pets appeared **duplicated across
  screens**, the screen **flashed** while it settled, and frozen ghost overlays
  sat under real windows. The engine's monitor ids are also list indices, so
  an index reshuffle registered as a world change and made it worse.

The single-screen model removes both failure modes structurally rather than
patching them:

- The engine and the platform adapter receive **only the chosen monitor**, with
  its id canonicalised to 0 (stable across reshuffles). Surface extraction
  already clips window edges to a monitor's work area, so every other screen's
  geometry is excluded before the graph exists; `Pet::clamp` keeps pets inside
  the chosen screen's bounds; and because there is only one monitor in engine
  space, the one scale factor the adapter applies is the right one for every
  window it reports.
- There is **one overlay window with a constant label**, created once. When the
  pet screen changes — a different pick, a resolution change, a replug — the
  window is **moved and resized** (`set_position` then `set_size`, logical
  coordinates pre-converted against the target screen's scale, because tao
  converts `set_size` with the window's *current* screen scale). No teardown,
  no reload, no flash, nothing to duplicate. Pets relocate through the existing
  `SurfaceLost` recovery, which the timeline tests already cover; a pet whose
  position the new screen does not cover is rehomed onto it like a fresh
  spawn, because falling alone can leave it parked at the clamp margin with
  nothing under it and no way back.

The user picks the screen in the companion app's Desktop panel (stored in
settings as the display's hardware name, so it survives list reshuffles); the
default is the main display. When a pick's display is not connected, the
fallback becomes the pick: the setting is rewritten to the screen pets
actually fell back to, and the old display does not reclaim them when it
returns — pick it again to move pets back. Decided 2026-09-10, after the
first live unplug: the earlier sticky-until-return design left the picker
naming a display that was not there, which read as the pets being in limbo
rather than moved.

The engine itself remains multi-monitor-capable — the timeline tests still
prove pets can use two monitors and survive one being pulled — so multi-screen
pets stay a product decision, not an engine rewrite, if it is ever revisited.
What is *not* solved, and does not need to be while pets are single-screen, is
mixed-DPI multi-monitor coordinate space in the engine.

## Screens that cannot be read

A locked session or a sleeping display makes macOS report **no screens at all**.
The engine treats that as a suspend trigger and keeps the last known monitor
geometry, rather than concluding the desktop is empty and stranding every pet.
This was found by running the app while the machine was locked.
