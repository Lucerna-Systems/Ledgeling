# Perch — Pet Pack Specification v1

**The design goal, stated as a test:**
> A person who can draw and edit a JSON file, but cannot program, should be able to ship a working climbing pet in under an hour.

If a proposed feature breaks that test, it doesn't go in the format.

---

## 1. Core principles

1. **Data, never code.** No scripting language. Behavior is declarative. A pack can look wrong but can never do harm.
2. **Minimum viable pack is tiny.** Two sprite strips and ~20 lines of JSON gets you a working pet. Everything else is optional with sensible fallbacks.
3. **Graceful degradation.** Missing a `climb` animation? It falls back to `move`, then to `idle`. A pack never fails to load because something optional is absent.
4. **The engine owns physics, the pack owns character.** Pack authors describe *what kind of animal this is* and *what it looks like*. They never implement pathfinding.
5. **Errors are for humans.** Every validation failure names the file, the JSON path, what was wrong, and what was expected.

---

## 2. Pack structure

```
budgie-classic/
├── pet.json              # required — the manifest
├── preview.png           # required — 256×256 pack browser thumbnail
├── sprites/
│   ├── idle.png          # required
│   ├── move.png          # required
│   ├── land.png          # optional
│   ├── takeoff.png
│   ├── preen.png
│   ├── sleep.png
│   ├── alert.png
│   └── happy.png
├── sounds/               # optional
│   └── chirp.ogg
├── LICENSE.txt           # required for distribution
└── README.md             # optional
```

Distributed as a `.petpack` file — a zip with the extension renamed. Double-click to install; drag a folder into the packs directory during development.

### Sprite format

Horizontal strips: all frames of one animation in a single row, equal width.

```
idle.png  →  [frame0][frame1][frame2][frame3]
             ← frame_width →
```

Frame width is derived as `image_width / frame_count`. PNG with alpha. No packed atlas required — the engine builds one internally at load time.

---

## 3. Manifest reference

### 3.1 Minimal valid pack

```json
{
  "format": 1,
  "id": "com.yourname.simplebird",
  "name": "Simple Bird",
  "author": "Your Name",
  "version": "1.0.0",
  "sprite": { "width": 48, "height": 48, "anchor": { "x": 0.5, "y": 1.0 } },
  "locomotion": { "class": "flier" },
  "animations": {
    "idle": { "file": "sprites/idle.png", "frames": 4, "fps": 6 },
    "move": { "file": "sprites/move.png", "frames": 6, "fps": 14 }
  }
}
```

That's a complete, working pet. Everything below is optional refinement.

### 3.2 Identity block

| Field | Type | Required | Notes |
|---|---|---|---|
| `format` | int | ✅ | Spec version. Currently `1`. |
| `id` | string | ✅ | Reverse-DNS, globally unique. Collisions are rejected at install. |
| `name` | string | ✅ | Display name |
| `author` | string | ✅ | |
| `version` | semver | ✅ | |
| `description` | string | | Shown in the pack browser |
| `license` | SPDX string | | e.g. `"CC-BY-4.0"` |
| `tags` | string[] | | e.g. `["bird","small","cheerful"]` |

### 3.3 Sprite block

```json
"sprite": {
  "width": 48,
  "height": 48,
  "anchor": { "x": 0.5, "y": 1.0 },
  "default_facing": "right",
  "scale": 1.0,
  "surface_orientation": "align"
}
```

| Field | Meaning |
|---|---|
| `anchor` | Contact point in 0–1 sprite space. `{0.5, 1.0}` = bottom-centre = feet. A bat hanging underneath would use `{0.5, 0.0}`. |
| `default_facing` | Which way the art faces natively. The engine mirrors automatically for the other direction. |
| `surface_orientation` | `"align"` rotates the sprite to match the surface (squirrel rotates 90° on a vertical window edge). `"upright"` never rotates (a bird stays upright even when perched on a vertical edge — it just can't). |

### 3.4 Locomotion block — the heart of the format

This is what makes a budgie a budgie and a squirrel a squirrel.

```json
"locomotion": {
  "class": "flier",
  "surfaces": ["window_top", "monitor_top", "taskbar_top"],
  "speed": { "surface": 45, "air": 220, "climb": 0 },
  "jump": { "enabled": false },
  "gravity": 0.0,
  "air_style": "flap_arc",
  "min_surface_length": 40
}
```

#### `class` — the traversal ruleset (required)

| Class | Moves through air? | Uses graph edges? | Falls? | Example |
|---|---|---|---|---|
| `flier` | Freely, any-to-any | No — ignores the graph | No | budgie, moth, dragonfly |
| `climber` | Only via jumps | Contiguous + jumpable | Yes | squirrel, gecko, spider |
| `walker` | No | Contiguous only, horizontal | Yes | cat, hedgehog, duck |
| `hopper` | Short arcs | Contiguous + short jumps | Yes | frog, sparrow-on-foot, rabbit |

Adding a class is an engine change, not a pack change. Four covers a very wide range of animals.

#### `surfaces` — where this pet is allowed to be

```
window_top      window_bottom     window_left      window_right
monitor_top     monitor_bottom    monitor_left     monitor_right
taskbar_top
```

Shorthand groups: `"window_any"`, `"window_vertical"`, `"monitor_any"`, `"all"`.

A budgie takes only horizontal top edges. A squirrel takes everything. A gecko might take only vertical edges. This single list does an enormous amount of characterisation work.

#### `speed` (px/sec), `gravity` (0 = weightless, 1 = normal fall), `air_style`

`air_style` is cosmetic pathing flavour: `"straight"`, `"flap_arc"` (bouncing sine, birds), `"flutter"` (erratic, insects), `"glide"` (long shallow descents).

### 3.5 Worked example — flier (budgie)

```json
"locomotion": {
  "class": "flier",
  "surfaces": ["window_top", "monitor_top", "taskbar_top"],
  "speed": { "surface": 45, "air": 220 },
  "gravity": 0.0,
  "air_style": "flap_arc",
  "min_surface_length": 40
}
```
*Result:* lands only on horizontal top edges, flies directly between them in bouncing arcs, never falls, shuffles slowly along a perch.

### 3.6 Worked example — climber (squirrel)

```json
"locomotion": {
  "class": "climber",
  "surfaces": ["window_any", "monitor_any", "taskbar_top"],
  "speed": { "surface": 110, "climb": 85, "air": 0 },
  "jump": { "enabled": true, "max_gap": 180, "arc_height": 70 },
  "gravity": 1.0,
  "min_surface_length": 60
}
```
*Result:* runs along window tops, climbs up and down window sides, rounds corners, leaps gaps up to 180px, and falls if its surface vanishes. Cannot cross a wide gap with no intermediate window — it has to find a route, which produces genuinely fun emergent behavior as you open and close windows.

**Same engine. Same graph. Completely different creature.** That's the whole architectural bet.

### 3.7 Animations block

```json
"animations": {
  "idle":    { "file": "sprites/idle.png",    "frames": 4, "fps": 6,  "loop": true },
  "move":    { "file": "sprites/move.png",    "frames": 6, "fps": 14, "loop": true },
  "climb":   { "file": "sprites/climb.png",   "frames": 6, "fps": 12, "loop": true },
  "jump":    { "file": "sprites/jump.png",    "frames": 4, "fps": 12, "loop": false },
  "fall":    { "file": "sprites/fall.png",    "frames": 2, "fps": 8,  "loop": true },
  "land":    { "file": "sprites/land.png",    "frames": 3, "fps": 16, "loop": false },
  "sleep":   { "file": "sprites/sleep.png",   "frames": 2, "fps": 2,  "loop": true },
  "alert":   { "file": "sprites/alert.png",   "frames": 3, "fps": 10, "loop": false },
  "happy":   { "file": "sprites/happy.png",   "frames": 6, "fps": 12, "loop": false },
  "grabbed": { "file": "sprites/grabbed.png", "frames": 2, "fps": 8,  "loop": true }
}
```

**Required:** `idle`, `move`. **Everything else falls back:**

```
climb   → move → idle
jump    → move → idle
fall    → move → idle
land    → idle
takeoff → move → idle
sleep   → idle
alert   → idle
happy   → idle
grabbed → idle
```

So a two-animation pack works everywhere. Adding art only ever improves it — it's never required to avoid breakage.

Optional per-clip extras: `"sound"` (plays on clip start), `"offset"` (nudge anchor for this clip), `"pingpong"` (bounce instead of loop).

### 3.8 Behavior block — declarative FSM

If omitted entirely, the engine applies a sensible default profile per locomotion class. Authors only write this to add character.

```json
"behavior": {
  "initial": "perch",
  "idle_wander_chance": 0.3,
  "states": {
    "perch": {
      "clip": "idle",
      "duration": [6, 20],
      "transitions": [
        { "to": "preen",    "weight": 3 },
        { "to": "relocate", "weight": 2 },
        { "to": "sing",     "weight": 1 },
        { "to": "doze",     "weight": 6, "when": "idle_seconds > 240" }
      ]
    },
    "preen": {
      "clip": "preen", "duration": [3, 6],
      "transitions": [{ "to": "perch", "weight": 1 }]
    },
    "relocate": {
      "action": "move_to_random_surface",
      "transitions": [{ "to": "perch", "weight": 1 }]
    },
    "doze": {
      "clip": "sleep", "duration": [30, 120],
      "transitions": [{ "to": "perch", "weight": 1, "when": "idle_seconds < 10" }]
    },
    "sing": {
      "clip": "happy", "duration": [2, 4], "sound": "sounds/chirp.ogg",
      "transitions": [{ "to": "perch", "weight": 1 }]
    }
  }
}
```

**State fields:** `clip`, `duration` (fixed or `[min,max]` random range), `action`, `sound`, `transitions`.

**Built-in actions** (engine-provided, authors just name them):
```
move_to_random_surface     move_to_nearest_surface
move_toward_cursor         flee_from_cursor
patrol_current_surface     return_to_home_surface
```

**Transitions:** `to` (target state), `weight` (relative probability), `when` (optional condition — if false, weight is zero).

### 3.9 Condition expression grammar

Deliberately tiny and non-Turing-complete. No function calls, no loops, no property access beyond the fixed signal namespace.

```
expr    := comparison (("&&" | "||") comparison)*
comparison := signal OP value
OP      := ">" | "<" | ">=" | "<=" | "==" | "!="
```

**Available signals:**

| Signal | Type | Meaning |
|---|---|---|
| `idle_seconds` | number | Seconds since last keyboard/mouse input |
| `session_minutes` | number | Length of current unbroken work stretch |
| `app_category` | enum | `code`/`browser`/`comms`/`media`/`creative`/`game`/`other` |
| `hour` | 0–23 | Local hour |
| `energy` | 0–1 | Pet's internal energy |
| `mood` | 0–1 | Pet's internal mood |
| `surface_count` | number | Available surfaces right now |
| `cursor_distance` | number | Pixels from pet to cursor |
| `is_dragging` | bool | User is dragging this pet |

Examples:
```
"idle_seconds > 300"
"session_minutes > 50 && app_category == code"
"cursor_distance < 120 && energy > 0.4"
"hour >= 22 || hour < 6"
```

Anything unparseable is a **validation error**, not a silent failure.

### 3.10 Reactions block — responding to awareness events

Reactions interrupt the normal FSM by priority. This is where the "aware of your work" character lives.

```json
"reactions": [
  { "on": "surface_lost",           "goto": "startled", "priority": 100 },
  { "on": "grabbed",                "goto": "held",     "priority": 90 },
  { "on": "long_session",           "goto": "nudge",    "priority": 50, "cooldown": 900 },
  { "on": "break_taken",            "goto": "sing",     "priority": 40, "cooldown": 300 },
  { "on": "user_idle_long",         "goto": "doze",     "priority": 30 },
  { "on": "user_returned",          "goto": "greet",    "priority": 60, "cooldown": 60 },
  { "on": "cursor_nearby",          "goto": "alert",    "priority": 20, "cooldown": 15 },
  { "on": "app_category_changed",   "goto": "lookup",   "priority": 10, "cooldown": 30 }
]
```

**Event catalogue:** `surface_lost`, `surface_moved_fast`, `grabbed`, `released`, `clicked`, `cursor_nearby`, `user_idle_long`, `user_returned`, `long_session`, `break_taken`, `app_category_changed`, `hour_changed`, `startup`, `low_energy`.

`cooldown` (seconds) prevents a reaction firing repeatedly and becoming annoying — an important tuning lever for anything nudge-related.

### 3.11 Personality block (optional)

Scalars that bias engine defaults without writing a full FSM. Good first knob for casual authors.

```json
"personality": {
  "activity": 0.7,      // how often it moves at all
  "boldness": 0.4,      // approaches vs avoids the cursor
  "sociability": 0.8,   // interest in other pets on screen
  "sleepiness": 0.3,    // how fast energy drains
  "curiosity": 0.6      // reaction to windows opening/closing
}
```

---

## 4. Authoring workflow — Pet Lab

The format only works if the dev loop is fast. Pet Lab is a built-in mode in the settings window.

1. **New Pack** — pick a locomotion class, get a scaffolded folder with placeholder sprites and a commented `pet.json`
2. **Live reload** — the packs directory is watched; saving `pet.json` or any PNG respawns the pet within ~200 ms
3. **Validator panel** — errors listed with file, JSON path, problem, and expected value. Warnings for things that are legal but probably wrong (`fps: 60` on a 2-frame clip; `max_gap: 2000`)
4. **Debug overlay** — draw the surface graph, the pet's current path, its state name and active signals. Essential for understanding why a climber refuses to cross a gap.
5. **Scenario buttons** — force-fire events (`long_session`, `surface_lost`, `user_returned`) without waiting an hour for real conditions
6. **Export** — zips to `.petpack` and runs a final validation pass

### Validation tiers

| Tier | Meaning | Behavior |
|---|---|---|
| **Error** | Pack cannot load | Blocks install, explains exactly what to fix |
| **Warning** | Loads but likely wrong | Shown in Pet Lab and pack browser |
| **Info** | Suggestion | Pet Lab only |

Example error output:
```
ERROR  pet.json → animations.climb.frames
       Expected a positive integer, found "6 " (string).
       Frame counts must be numbers, not quoted strings.

WARN   pet.json → locomotion.jump.max_gap
       180 exceeds typical range for a 48px sprite (~120).
       Jumps may look physically implausible.

INFO   sprites/idle.png
       Image is 192×64 but sprite.height is 48.
       12 px of vertical space is unused.
```

---

## 5. Versioning and compatibility

- `format: 1` is frozen once v1.0 ships. New optional fields may be added within format 1; anything breaking requires `format: 2`.
- Unknown fields are ignored with an INFO note — forward-compatible so newer packs degrade rather than fail on older builds.
- The engine ships a JSON Schema (`schema/pet-pack-v1.schema.json`) so authors get autocomplete and inline errors in VS Code.

---

## 6. Extending to new species — worked scenarios

The format was designed against these cases; each should require **zero engine changes**.

| Species | Class | Key config | Emergent result |
|---|---|---|---|
| **Squirrel** | climber | all surfaces, jump 180px, gravity 1.0 | Climbs window sides, leaps between windows, falls when one closes |
| **Budgie** | flier | top edges only, no gravity | Flits between window tops, ignores layout complexity |
| **Gecko** | climber | `window_vertical` only, no jump, `surface_orientation: align` | Only ever on window sides, rotated flat to the glass |
| **Cat** | walker | `window_top` + `taskbar_top`, min length 120 | Only settles on wide surfaces; ignores narrow windows |
| **Moth** | flier | `air_style: flutter`, `monitor_any` | Erratic drifting, rarely settles |
| **Bat** | climber | `window_bottom` only, anchor `{0.5, 0.0}` | Hangs upside-down from window undersides |
| **Frog** | hopper | jump 90px, gravity 1.0 | Hops between nearby edges, falls to the taskbar |

That table is the actual proof the abstraction is right: seven distinctly different-feeling animals, all pure data.

---

## 7. Open questions for v1.1+

- **Multi-pet interaction.** Should pets notice each other? Needs an interaction event vocabulary. Deferred — significant scope.
- **Pet-carried props.** A squirrel holding an acorn. Probably an attachment-point system on the sprite block.
- **Sound design.** Currently one-shot clips per state. Ambient loops and volume-ducking on foreground app change may be worth it.
- **Procedural tinting.** Palette-swap variants from one sprite set — cheap variety, needs an indexed-colour convention.
- **Larger-than-window pets.** A cat wider than a narrow window looks broken. Currently handled with `min_surface_length`; may need overhang tolerance.
