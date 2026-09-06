# Perch — Technical Architecture

**Stack:** Tauri 2 · Rust backend · TypeScript + Canvas/PixiJS frontend
**Primary target:** Windows 10/11 · **Portable to:** macOS, Linux

---

## 1. Why Tauri

| Requirement | Tauri answer |
|---|---|
| Transparent window | `transparent: true` — first-class config |
| No titlebar/border | `decorations: false` |
| Always on top | `alwaysOnTop: true` |
| Click-through | `set_ignore_cursor_events()` — togglable at runtime |
| OS-level queries (idle, foreground window, window rects) | Rust backend + `windows-rs`, Microsoft's own Win32 bindings |
| Small binary | Uses system webview, ~5–10 MB vs Electron's ~100 MB+ |
| Cross-platform later | Same codebase, OS specifics isolated behind a trait |

The alternative (GameMaker) requires paid third-party extensions for transparency and click-through, and custom DLL work for window enumeration. Tauri gets all of it natively.

---

## 2. High-level structure

```
┌─────────────────────────────────────────────────────────────┐
│                     RUST BACKEND (core)                     │
│                                                             │
│  ┌───────────────┐  ┌────────────────┐  ┌───────────────┐   │
│  │ Platform      │  │ World Model    │  │ Awareness     │   │
│  │ Layer (trait) │→ │ (surface graph)│  │ Engine        │   │
│  │ win/mac/linux │  │                │  │ idle/app/time │   │
│  └───────────────┘  └────────────────┘  └───────────────┘   │
│           ↓                  ↓                   ↓          │
│  ┌────────────────────────────────────────────────────┐     │
│  │              Pet Runtime (per pet)                 │     │
│  │  pathfinding · behavior FSM · physics · state      │     │
│  └────────────────────────────────────────────────────┘     │
│           ↓                                                 │
│  ┌───────────────┐  ┌────────────────┐                      │
│  │ Pack Loader   │  │ Window Manager │                      │
│  │ + validator   │  │ one per pet    │                      │
│  └───────────────┘  └────────────────┘                      │
└─────────────────────────────────────────────────────────────┘
                            ↕ IPC (events + commands)
┌─────────────────────────────────────────────────────────────┐
│                   WEBVIEW FRONTEND                          │
│  Pet Window(s): canvas sprite renderer, alpha hit-test      │
│  Settings Window: pack manager, preferences, Pet Lab        │
└─────────────────────────────────────────────────────────────┘
```

**Division of responsibility:** Rust owns all truth — where the pet is, what state it's in, what the world looks like. The webview is a dumb renderer that draws a sprite frame at a position. This keeps behavior deterministic, testable without a GUI, and avoids the webview becoming a performance bottleneck.

---

## 3. Rendering approach — the critical decision

Two viable models. **This must be spiked in M0 before anything else is built**, because the whole product depends on it feeling smooth.

### Option A — One small window per pet (recommended, default)

Each pet gets its own borderless transparent window, sized to the sprite bounds plus margin, repositioned as the pet moves.

- ✅ Cheap to composite — tiny surfaces
- ✅ Handles per-monitor DPI naturally
- ✅ Click-through mostly solves itself: the window only covers the pet, so everything else is clickable by default
- ⚠️ Repositioning at 60 Hz via `SetWindowPos` may jitter or cost more than expected — **this is the thing to measure**
- ⚠️ "Pet above taskbar" needs care with window layering

### Option B — Single full-virtual-desktop overlay

One transparent window covering all monitors; pets are drawn inside it.

- ✅ No per-frame window moves; movement is pure canvas rendering
- ✅ Cross-monitor movement is trivial
- ⚠️ A permanently always-on-top full-screen layered window is heavier and can interfere with fullscreen apps
- ⚠️ Must dynamically toggle click-through based on alpha hit-testing under the cursor
- ⚠️ Mixed-DPI multi-monitor setups get awkward

### Decision rule

Build the M0 spike with Option A. If sustained repositioning of a 128×128 window at 60 Hz costs more than ~2% CPU or visibly jitters, switch to Option B. Keep the renderer behind a `PetSurface` trait so the swap is contained.

### Movement smoothing (either option)

Reposition only when the pet's integer position actually changes; run animation frames inside the window independently. Cap to 30 fps while perched/idle, 60 fps only during active movement. Pause rendering entirely when the pet is off-screen or suspended.

---

## 4. Platform layer

All OS-specific calls sit behind one trait so macOS/Linux can be added without touching pet logic.

```rust
pub trait Platform: Send + Sync {
    /// Visible top-level windows, front-to-back in z-order.
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>>;

    /// Seconds since last keyboard/mouse input.
    fn idle_seconds(&self) -> Result<f32>;

    /// Currently focused window — process name only, never the title.
    fn foreground_process(&self) -> Result<Option<ProcessInfo>>;

    /// Monitor geometry, work areas, scale factors.
    fn monitors(&self) -> Result<Vec<MonitorInfo>>;

    /// Is a fullscreen/exclusive app running? (suspend trigger)
    fn fullscreen_app_active(&self) -> Result<bool>;
}

pub struct WindowInfo {
    pub id: u64,
    pub rect: PhysicalRect,     // virtual-desktop coordinates
    pub z_index: usize,         // 0 = frontmost
    pub is_minimized: bool,
    pub monitor_id: u64,
    pub app_category: AppCategory,  // derived; raw title NEVER stored
}
```

### Windows implementation notes

| Need | API | Gotcha |
|---|---|---|
| Enumerate windows | `EnumWindows` | Filter aggressively: skip invisible, zero-size, `WS_EX_TOOLWINDOW`, and cloaked UWP shells |
| Detect cloaked windows | `DwmGetWindowAttribute(DWMWA_CLOAKED)` | Without this you get phantom invisible UWP windows |
| Accurate bounds | `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` | `GetWindowRect` includes invisible resize borders — pets would perch several px off the visual edge |
| Idle time | `GetLastInputInfo` | Returns tick count; handle 49-day wraparound |
| Foreground app | `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW` | **Take the process name only.** Never read the window title. |
| Taskbar position | `SHAppBarMessage(ABM_GETTASKBARPOS)` | Users move it; it can auto-hide |
| Work area per monitor | `GetMonitorInfoW` | Use work area, not full bounds, when respecting the taskbar |
| DPI awareness | Manifest as Per-Monitor V2 | Otherwise coordinates silently break on mixed-DPI setups |
| Efficient change detection | `SetWinEventHook` on `EVENT_OBJECT_LOCATIONCHANGE` | Very noisy — debounce hard, or just poll instead |

### Coordinate space

Everything internally is **physical pixels in virtual-desktop space** (origin at the primary monitor's top-left; negative coordinates are legal for monitors placed left/above). Convert to logical only at the Tauri window API boundary.

---

## 5. World model — the surface graph

This is the piece that lets a budgie and a squirrel share one engine.

### Build pipeline (runs each scan)

```
1. Enumerate visible windows, front-to-back
2. For window i, for each edge (top/bottom/left/right):
     segment = full edge line
     for each window j where j is in front of i (j < i):
         subtract j.rect's projection from segment
     → yields visible segments only
3. Add permanent surfaces: monitor edges, taskbar top edge
4. Discard segments shorter than min_segment_length (default 48px)
5. Build graph edges between segments:
     - CONTIGUOUS: segments touching at a corner (climber can walk around)
     - JUMPABLE:   gap ≤ pet.max_gap and roughly reachable by arc
     - AIR:        any-to-any (fliers only, ignores the graph entirely)
6. Diff vs previous scan → emit SurfaceAdded / SurfaceLost / SurfaceMoved
```

**Step 2 is the important one.** Without z-order subtraction, pets perch on window edges that are hidden behind other windows, and the illusion dies instantly.

### Surface types

```rust
enum SurfaceKind {
    WindowTop, WindowBottom, WindowLeft, WindowRight,
    MonitorTop, MonitorBottom, MonitorLeft, MonitorRight,
    TaskbarTop,
}

struct Surface {
    id: SurfaceId,
    kind: SurfaceKind,
    owner_window: Option<u64>,
    start: PhysicalPoint,
    end: PhysicalPoint,
    orientation: Orientation,  // Horizontal | Vertical
    normal: Vec2,              // which way is "up" for a pet standing here
}
```

A pet occupies `(SurfaceId, t)` where `t ∈ [0,1]` along the segment. When the owning window moves, the pet's world position updates automatically — it rides the window. That's free realism.

### Scan cadence

- Baseline poll: **2 Hz** (windows don't move that often)
- Bump to 10 Hz for 2 seconds after any detected window move/close, then decay
- Skip entirely while suspended (fullscreen app, screen locked, pet asleep)

### Required event handling

| Event | Pet must |
|---|---|
| `SurfaceLost` (window closed/minimized) | Immediately transition to a recovery state — flier takes off, climber scrambles/falls |
| `SurfaceMoved` (window dragged) | Ride it; optionally play a "startled" clip if velocity is high |
| Surface shrinks below pet width | Relocate |
| All surfaces gone (fullscreen) | Fall back to monitor edge, or suspend |

---

## 6. Pet runtime

Each pet instance runs an independent update loop:

```
tick(dt):
  1. Pull latest signals from Awareness Engine
  2. Update mood/energy scalars
  3. Behavior FSM: evaluate transitions, apply reactions by priority
  4. If state implies movement → request path from World Model
  5. Step physics/locomotion along path
  6. Emit render frame {sprite_clip, frame_index, position, flip, rotation}
```

### Pathfinding by locomotion class

| Class | Pathing |
|---|---|
| **Flier** | Ignore graph edges. Direct air path to target surface, with a wander/arc offset so flight isn't a straight line. |
| **Climber** | A* over the surface graph using CONTIGUOUS + JUMPABLE edges only. Cannot cross open air except via jumps. |
| **Walker** | A* over CONTIGUOUS edges on horizontal surfaces only. Falls if the surface disappears. |
| **Hopper** | Walker + JUMPABLE edges, shorter max gap. |

The graph is shared; only the traversal ruleset differs per class. Adding a new class = adding a traversal ruleset, not a new engine.

---

## 7. IPC contract

Kept deliberately thin — high-frequency render data goes one way, user actions come back.

**Rust → Frontend (events)**
```ts
"pet:frame"    → { petId, clip, frameIndex, x, y, flipX, rotation, opacity }
"pet:spawn"    → { petId, packId, spriteAtlas }
"pet:despawn"  → { petId }
"world:suspend"/"world:resume"
```

**Frontend → Rust (commands)**
```ts
invoke("list_packs")
invoke("install_pack", { path })
invoke("validate_pack", { path })       // Pet Lab
invoke("spawn_pet", { packId })
invoke("despawn_pet", { petId })
invoke("pet_interact", { petId, kind })  // click, drag start/move/end
invoke("get_settings") / invoke("set_settings", { ... })
```

Interaction note: dragging a pet is handled by sending pointer events to Rust, which updates authoritative position — not by letting the frontend move the window. Keeps one source of truth.

---

## 8. Repository layout

```
perch/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── platform/
│   │   │   ├── mod.rs          # Platform trait
│   │   │   ├── windows.rs
│   │   │   ├── macos.rs        # stub in v1
│   │   │   └── linux.rs        # stub in v1
│   │   ├── world/
│   │   │   ├── surfaces.rs     # segment extraction + z-order subtraction
│   │   │   ├── graph.rs        # traversal graph
│   │   │   └── scanner.rs      # polling + diffing
│   │   ├── awareness/
│   │   │   ├── signals.rs
│   │   │   ├── categories.rs   # process name → AppCategory
│   │   │   └── session.rs      # unbroken-stretch tracking
│   │   ├── pet/
│   │   │   ├── runtime.rs
│   │   │   ├── fsm.rs          # declarative state machine interpreter
│   │   │   ├── locomotion.rs   # per-class traversal rules
│   │   │   └── pathfind.rs
│   │   ├── packs/
│   │   │   ├── manifest.rs     # serde types for pet.json
│   │   │   ├── loader.rs
│   │   │   ├── validator.rs
│   │   │   └── expr.rs         # tiny safe condition evaluator
│   │   └── windows_mgr.rs      # Tauri window lifecycle per pet
│   └── tauri.conf.json
├── src/                        # frontend
│   ├── pet/                    # canvas renderer
│   ├── settings/               # settings + pack manager UI
│   └── petlab/                 # pack dev tool
├── packs/                      # bundled first-party packs
│   ├── budgie-classic/
│   └── squirrel-red/
├── schema/
│   └── pet-pack-v1.schema.json
└── docs/
```

---

## 9. Performance budget

Non-negotiable — a companion app that eats battery gets uninstalled.

| Metric | Target | Ceiling |
|---|---|---|
| CPU, 1 pet idle/perched | < 0.5% | 1% |
| CPU, 1 pet actively moving | < 2% | 4% |
| CPU, suspended | ~0% | 0.2% |
| RAM, 1 pet | < 80 MB | 120 MB |
| RAM, 5 pets | < 150 MB | 220 MB |
| Window scan cost | < 3 ms | 8 ms |
| Cold start to first pet visible | < 1.5 s | 3 s |

**Suspend triggers (drop to near-zero work):** fullscreen app detected, session locked, monitor asleep, battery below user-set threshold, user-toggled pause.

---

## 10. Security posture

Three properties that must hold, stated up front because they constrain later design:

1. **Pet packs are data, never code.** No JS, no Lua, no eval. Behavior is declarative JSON evaluated by a tiny fixed-grammar expression evaluator. A malicious pack must not be able to do anything worse than look wrong.
2. **No window titles, ever.** Foreground detection reads process names only, mapped to a category. Window titles routinely contain email subjects, document names, and URLs — the app never touches them.
3. **Nothing leaves the machine in v1.** No telemetry, no crash reporting to a server, no update pings beyond a version check. When the AI layer arrives (phase 3) it ships off by default with an explicit opt-in that states exactly what is transmitted.

Tauri's capability system should be configured to the minimum: no `shell`, no arbitrary `fs` access beyond the packs directory and config file.

---

## 11. Testing strategy

The architecture is deliberately shaped so the interesting logic is testable without a GUI.

- **Unit:** surface segment extraction against fixed synthetic window layouts (including full occlusion, partial overlap, nested windows, off-screen windows)
- **Unit:** pathfinding — climber cannot cross a gap wider than `max_gap`; flier can; walker falls when surface vanishes
- **Unit:** manifest validation — every malformed-pack case produces a specific, human-readable error
- **Property:** a pet's position is always on a valid surface or in a legal air/fall state; never NaN, never off virtual desktop
- **Integration:** mock `Platform` replaying recorded window-layout timelines, so "user dragged a window while the squirrel was climbing it" is a repeatable test
- **Manual soak:** 8-hour run, measure memory drift and handle count (leak detection)

---

## M0 implementation decision — 2026-09-06

The local macOS MVP uses **Option B**, a transparent overlay on the primary display. The 128×128 moving-window spike was implemented first and compared against an overlay using an identical 60 Hz path. The small-window native CPU cost exceeded the proposed budget. Raw results, measurement boundaries, and the decision are in [docs/rendering-decision.md](../docs/rendering-decision.md). `PetSurface` retains both implementations; `PERCH_RENDERER=small` selects the alternative.

This MVP adds a geometry-only macOS adapter to run on the available development machine. It does not change the Windows-first product roadmap or claim M1–M4 completion. Rust geometry remains physical pixels, using the primary monitor scale at the CoreGraphics and Tauri boundaries. Mixed-DPI secondary-display traversal remains deferred to M1.
