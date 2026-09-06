# Perch — User Stories & Backlog

**Roles**
- **Owner** — the person running Perch on their desktop
- **Author** — someone making a pet pack
- **Maintainer** — you, building and shipping it

Stories are grouped by milestone. **Milestones are hard gates:** nothing from a later milestone gets built until the current one ships. This is the main defence against the project never finishing.

---

## M0 — Technical Spike (target: 1–2 weeks)

**Goal:** prove the illusion is achievable and performant before building anything real. If M0 fails, the architecture changes — better to learn that now than after the behavior engine exists.

**Exit criteria:** a bird sprite flies around your desktop and lands on real window edges, at acceptable CPU cost. Nothing else.

---

**M0-1 — Transparent overlay window**
> As the **Maintainer**, I need a borderless, transparent, always-on-top Tauri window, so a sprite can appear to float on the desktop.

*Acceptance:*
- Window shows a PNG with no background, border, titlebar, or taskbar entry
- Stays above normal windows
- Survives display resolution changes without crashing

---

**M0-2 — Click-through with alpha hit-testing**
> As an **Owner**, I want to click through the empty space around the pet, so it never blocks my work.

*Acceptance:*
- Clicks on transparent pixels reach the app underneath
- Clicks on the pet itself register on the pet
- Verified against the chosen rendering model (per-pet window vs full overlay)

---

**M0-3 — Window enumeration and edge detection**
> As the **Maintainer**, I need the visible edges of open windows, so pets have somewhere real to stand.

*Acceptance:*
- Enumerates visible top-level windows with accurate bounds (`DWMWA_EXTENDED_FRAME_BOUNDS`, not `GetWindowRect`)
- Filters cloaked UWP windows, tool windows, zero-size windows
- Z-order occlusion subtraction works: an edge behind another window is **not** reported as available
- Debug overlay draws detected surfaces
- Scan completes in < 8 ms with 20+ windows open

---

**M0-4 — Performance spike: per-frame repositioning** ⚠️ **RISKIEST ITEM IN THE PROJECT**
> As the **Maintainer**, I need to know whether moving a small window at 60 Hz is viable, so I can pick the rendering architecture.

*Acceptance:*
- Measured CPU for a 128×128 window repositioned at 60 Hz for 10 minutes
- Assessed for visible jitter/tearing
- Compared against the full-overlay approach
- **Decision recorded in the architecture doc**

---

**M0-5 — Sprite renders and moves**
> As an **Owner**, I want to see an animated bird fly across my desktop, so the concept is real.

*Acceptance:*
- Animated sprite strip plays at a set fps
- Bird moves between two hardcoded points
- Sustains > 30 fps

---

## M1 — Core Engine (target: 4–6 weeks)

**Goal:** the surface graph, both locomotion classes, and two genuinely different-feeling pets.

**Exit criteria:** a budgie and a squirrel behave visibly differently on the same desktop.

---

**M1-1 — Surface graph**
> As the **Maintainer**, I need surfaces connected into a traversable graph, so pets can path between them.

*Acceptance:*
- Segments become graph nodes; contiguous and jumpable connections identified
- Diffing emits `SurfaceAdded`/`Lost`/`Moved`
- Unit-tested against synthetic layouts: full occlusion, partial overlap, nested, off-screen, multi-monitor

---

**M1-2 — Flier locomotion**
> As an **Owner**, I want the budgie to fly between window tops, so it moves like a bird.

*Acceptance:*
- Direct air pathing ignoring graph edges
- `flap_arc` produces a bouncing rather than straight path
- Lands with a `land` clip, takes off with `takeoff`
- Only settles on surfaces in its allowed list

---

**M1-3 — Climber locomotion**
> As an **Owner**, I want the squirrel to climb along window borders, so it feels physically grounded.

*Acceptance:*
- A* pathing over contiguous + jumpable graph edges only
- Rotates to align with vertical surfaces
- Rounds corners between adjacent edges
- Jumps gaps up to `max_gap`, arcs plausibly
- **Cannot** cross a gap wider than `max_gap` — reroutes or stays put
- Falls under gravity when its surface disappears, lands on the first surface below

---

**M1-4 — Pets ride moving windows**
> As an **Owner**, I want the pet to stay on a window when I drag it, so it feels attached to my desktop.

*Acceptance:*
- Pet holds its `(surface, t)` position as the window moves
- Fast drags optionally trigger `startled`
- Window resize keeps the pet on the surface, or triggers relocation if it no longer fits

---

**M1-5 — Surface loss recovery**
> As an **Owner**, I want the pet to react when I close the window it's on, so it never floats in dead space.

*Acceptance:*
- Flier takes off toward another surface
- Climber/walker falls and lands
- Falling pet that reaches the screen bottom lands on the taskbar or monitor edge
- No surfaces at all → moves to a monitor edge or suspends
- Pet is **never** left floating in mid-air or off-screen

---

**M1-6 — Multi-monitor**
> As an **Owner** with two monitors, I want the pet to use both, so it isn't trapped on one screen.

*Acceptance:*
- Virtual-desktop coordinates handled, including negative
- Mixed DPI handled correctly
- Monitor connect/disconnect doesn't strand or crash the pet

---

**M1-7 — Fullscreen suspend**
> As an **Owner**, I want the pet to get out of the way in games and films, so it never interferes.

*Acceptance:*
- Fullscreen exclusive app detected
- Pet hides and rendering/scanning suspends
- CPU drops to ~0%
- Restores on exit

---

## M2 — Pet Pack System (target: 3–4 weeks)

**Goal:** the extensibility layer. This is the growth engine — it has to be good.

**Exit criteria:** someone who isn't you ships a working pack.

---

**M2-1 — Manifest loading**
> As an **Author**, I want to define a pet in JSON, so I can add one without writing code.

*Acceptance:*
- Full v1 manifest parsed per spec
- Minimal pack (2 animations, one locomotion line) loads and works
- Animation fallback chain works — missing `climb` falls back to `move`
- Unknown fields ignored with an INFO note

---

**M2-2 — Validator with human errors**
> As an **Author**, I want to be told exactly what's wrong with my pack, so I'm not guessing.

*Acceptance:*
- Every error names file, JSON path, problem, and expectation
- Error/Warning/Info tiers implemented
- Malformed JSON, missing sprites, bad frame counts, unparseable conditions all covered
- **No error message ever consists only of a stack trace or a raw serde error**

---

**M2-3 — Declarative behavior FSM**
> As an **Author**, I want to describe my pet's behavior in data, so it has personality without scripting.

*Acceptance:*
- States, durations, weighted transitions, built-in actions all work
- Condition grammar evaluated safely — no code execution path exists
- Omitting `behavior` yields a sensible per-class default
- Invalid conditions are validation errors, not silent no-ops

---

**M2-4 — Pack install and management**
> As an **Owner**, I want to install pet packs easily, so I can get new animals.

*Acceptance:*
- `.petpack` installs by double-click or drag
- Pack browser lists installed packs with previews
- Spawn, despawn, uninstall
- Duplicate `id` rejected with a clear message

---

**M2-5 — Pet Lab live reload**
> As an **Author**, I want changes to appear instantly, so iterating is fast.

*Acceptance:*
- Pack directory watched; changes respawn within ~200 ms
- Validation errors appear live without restarting the app
- Debug overlay shows surface graph, current path, state name, active signals
- Scenario buttons force-fire awareness events

---

**M2-6 — Pack scaffolding**
> As an **Author**, I want a starting template, so I'm not staring at an empty folder.

*Acceptance:*
- "New Pack" generates a folder per locomotion class with placeholder sprites and commented manifest
- Generated pack runs immediately without edits
- JSON Schema published for editor autocomplete

---

**M2-7 — Two shipped species proving the abstraction**
> As an **Owner**, I want pets that feel genuinely different, so the app isn't one animation reskinned.

*Acceptance:*
- Budgie: flier, top edges, flits and perches, preens, sings
- Squirrel: climber, all edges, climbs and leaps, falls when a window closes
- **Neither required an engine change beyond its locomotion class**
- A third species (gecko: vertical edges only, no jump) can be made in under an hour with no code — this is the acceptance test for the whole format

---

## M3 — Awareness Layer (target: 3–4 weeks)

**Goal:** the actual differentiator.

**Exit criteria:** the pet's behavior visibly reflects your session without you telling it anything.

---

**M3-1 — Idle detection**
> As an **Owner**, I want the pet to settle down when I step away, so it feels attentive.

*Acceptance:*
- Idle seconds polled at 1 Hz, tick wraparound handled
- Four idle states derived at documented thresholds
- Pet dozes when away, greets on return

---

**M3-2 — App category detection (privacy-constrained)**
> As an **Owner**, I want the pet to respond to what kind of work I'm doing, without it reading my screen.

*Acceptance:*
- Process name → category mapping works
- **Window titles are never read** — verified by code review and a test asserting no title API is called
- Category changes debounced 5s
- Mapping table user-editable

---

**M3-3 — Energy and mood**
> As an **Owner**, I want the pet to have continuity of state, so it feels alive rather than scripted.

*Acceptance:*
- Both scalars update on a minutes-long timescale
- Exposed to packs as read-only signals
- Visible behavioral difference between a fresh and a tired pet

---

**M3-4 — Session tracking**
> As an **Owner**, I want the pet to notice long unbroken stretches, so it can nudge me gently.

*Acceptance:*
- Unbroken stretch tracked; breaks ≥ 5m reset it
- Micro-breaks noted without resetting
- All in-memory; nothing written to disk beyond aggregates

---

**M3-5 — Nudge ladder**
> As an **Owner**, I want a gentle prompt to take breaks, without being nagged.

*Acceptance:*
- Three tiers implemented at documented thresholds
- **No popups, no toasts, no text, no focus steal** — body language only
- Cooldowns enforced; escalation caps
- Suppressed in `game`/`media` categories
- Thresholds user-editable; whole system disableable

---

**M3-6 — Privacy screen**
> As an **Owner**, I want to know exactly what's being observed, so I can trust it.

*Acceptance:*
- Plain-language settings page listing collected and not-collected data
- One master toggle disables all awareness while keeping the pet
- No network calls exist in the v1 build (verifiable with a network monitor)

---

## M4 — Ship (target: 2–3 weeks)

**Goal:** it's a real product other people can install.

**Exit criteria:** publicly downloadable and someone you don't know is using it.

---

**M4-1 — Settings and persistence** — pets/positions/preferences restored on launch; corrupt config recovers to defaults rather than crashing
**M4-2 — Tray icon and lifecycle** — pause/resume, launch-on-startup (opt-in, off by default), clean quit
**M4-3 — Installer and signing** — MSI/NSIS; signed if affordable; SmartScreen behavior documented; antivirus false-positive guidance in the README
**M4-4 — Stability soak** — 8-hour run with no memory growth or handle leaks; survives sleep/wake, lock/unlock, monitor changes, RDP
**M4-5 — Pack authoring guide** — a non-programmer follows it end-to-end and ships a pet in under an hour. *Test it on an actual non-programmer.*
**M4-6 — Public release** — itch.io page, GitHub release, demo GIF, devlog post

---

## Post-v1 — Deliberately Deferred

Listed to keep them **out** of v1. Nothing here is built until M4 ships publicly.

| Item | Milestone |
|---|---|
| macOS support | v1.1 |
| Linux support | v1.2 |
| Pets noticing each other | v1.2 |
| More species (cat, gecko, moth, bat, frog) | v1.1+ |
| Sound design pass | v1.1 |
| AI companion layer (BYOK first) | v2.0 |
| Pets performing actions (needs a permissions model) | v2.1+ |
| Meeting detection via camera/mic state | v1.1 |
| Notification interception | Probably never — OS-policy blocked |
| Pack marketplace | Only if community packs materialise |

---

## Suggested build order

The first thing to build is not the first thing on this list.

1. **M0-4 first.** The performance spike answers the question the entire architecture depends on. Do it before writing anything you'd have to throw away.
2. Then M0 in order — get a bird on real window edges. This is the motivating milestone; everything after is easier once you've seen it work.
3. M1-3 (climber) before M1-2 (flier), counterintuitively. The climber is the harder case, and building it first proves the graph abstraction genuinely handles both. If the graph is only ever exercised by a flier that ignores it, you'll discover its flaws late.
4. M2 before M3. The pack system changes engine interfaces; doing it after the awareness layer means rewriting the awareness integration.
5. **Ship M4 before touching anything in the deferred list.** A shipped small thing beats an unshipped ambitious one, both as a product and as portfolio evidence.
