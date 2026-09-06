# Perch — Design Documentation

A desktop companion that lives on the edges of your real windows, moves like the animal it actually is, and quietly notices how your work session is going.

---

## Documents

| Doc | Covers | Read it when |
|---|---|---|
| **[01 — Product Brief](01-product-brief.md)** | Vision, competitive gap, scope boundaries, monetization, risks | Deciding *what* to build, or when scope creep starts |
| **[02 — Technical Architecture](02-technical-architecture.md)** | Tauri structure, rendering models, OS integration, surface graph, performance budget, security | Writing code |
| **[03 — Pet Pack Spec](03-pet-pack-spec.md)** | The extensibility format — manifest reference, locomotion classes, animations, behavior FSM, authoring workflow | Adding species, or designing anything that touches pets |
| **[04 — Awareness & Behavior](04-awareness-and-behavior.md)** | Signal collection, energy/mood model, the nudge ladder, privacy constraints | Building the differentiator |
| **[05 — User Stories](05-user-stories.md)** | Milestones M0–M4 with acceptance criteria, plus deferred list | Every working session — this is the backlog |

---

## The three ideas that matter

**1. The desktop is terrain, not backdrop.**
Open windows are parsed into a *surface graph* — visible edge segments with z-order occlusion subtracted, connected by contiguity and jumpability. Pets occupy `(surface, t)` positions, so they ride windows when you drag them and have to cope when you close them.

**2. Species differ by traversal rules, not by animation.**
One graph, four rulesets. A `flier` ignores graph edges and moves through air. A `climber` must follow connected edges and can only cross gaps by jumping. Same engine, completely different creature. Seven distinct animals are specified in doc 03 as pure data with zero engine changes.

**3. Awareness is observed, not reported.**
Idle time, foreground app *category*, unbroken session length — feeding two slow scalars (energy, mood) that bias behavior. Never window titles, never screen contents, never anything off the machine.

---

## Adding a new pet — the short version

```
mypet/
├── pet.json
├── preview.png
└── sprites/
    ├── idle.png
    └── move.png
```

```json
{
  "format": 1,
  "id": "com.you.gecko",
  "name": "Gecko",
  "author": "You",
  "version": "1.0.0",
  "sprite": { "width": 40, "height": 24, "anchor": { "x": 0.5, "y": 1.0 },
              "surface_orientation": "align" },
  "locomotion": {
    "class": "climber",
    "surfaces": ["window_vertical", "monitor_left", "monitor_right"],
    "speed": { "surface": 70, "climb": 70 },
    "jump": { "enabled": false },
    "gravity": 1.0
  },
  "animations": {
    "idle": { "file": "sprites/idle.png", "frames": 4, "fps": 5 },
    "move": { "file": "sprites/move.png", "frames": 6, "fps": 12 }
  }
}
```

That's a complete gecko: lives only on vertical window edges, rotated flat against them, can't jump, falls if its window closes. No code.

---

## Where to start building

**Do M0-4 first** — the spike measuring whether a small window can be repositioned at 60 Hz without jitter or CPU cost. The entire rendering architecture depends on the answer, and finding out late means throwing away work.

Then get a bird onto real window edges (rest of M0). Seeing that work is the thing that carries the project through the less glamorous middle stretch.

Full reasoning in [doc 05, "Suggested build order"](05-user-stories.md#suggested-build-order).

---

## The two rules that protect this project

1. **Milestones are gates.** Nothing from M3 gets built during M1. Nothing from the deferred list gets built before M4 ships. The most likely failure mode for this project isn't technical difficulty — it's an unshipped, half-built, over-ambitious version of something that would have been great small.

2. **The pack format's acceptance test is a person, not a spec.** If someone who can draw but can't program cannot ship a working pet in under an hour, the format has failed regardless of how elegant it looks.
