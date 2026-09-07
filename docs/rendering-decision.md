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

## Screens that cannot be read

A locked session or a sleeping display makes macOS report **no screens at all**.
The engine treats that as a suspend trigger and keeps the last known monitor
geometry, rather than concluding the desktop is empty and stranding every pet.
This was found by running the app while the machine was locked.
