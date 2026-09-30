# Making a pet

The quickest way is inside Ledgeling: **Pets → Make your own pet** opens the Pet Studio.
Draw each animation frame by frame, set the behaviours with plain-language blocks
("When you click it, go to say hi"), and press **Save & bring out**. The Studio
writes an ordinary pack, the same format this guide describes, and the same
validator checks it.

This guide is the by-hand route. You need two things: a program that can draw with
transparency, and a text editor. You do not need to write any code — a pack is
data, and there is nowhere in the format to put a program even if you wanted to.

Give this an hour. Most of that hour is drawing.

---

## 1. Make the folder (2 minutes)

Open Ledgeling → **Settings → Advanced → Pet Lab** → under **New pack folder**, type a
name, pick how it moves, press **New pack**.

You now have a folder with placeholder art and a `pet.json` that already works.
Ledgeling tells you where it is. Open that folder.

```
my-new-pet/
├── pet.json                  the settings
├── preview.png               the picture in the pack list
├── pet-pack-v1.schema.json   makes your editor autocomplete pet.json
├── README.md
├── LICENSE.txt
└── sprites/
    ├── idle.png
    └── move.png
```

Press **Bring out** on your new pack. A placeholder blob appears on your windows.
It works. Everything from here is replacing parts of something that already runs.

---

## 2. Draw the animal (most of the hour)

Each PNG is one animation, drawn as a single row of equal-width frames:

```
idle.png →  [frame 1][frame 2][frame 3][frame 4]
            ← 48 px →
```

- Transparency matters: Ledgeling uses it so clicks pass through the empty corners.
- The frame size is up to you. Whatever you choose, write it in `pet.json` under
  `sprite.width` and `sprite.height`.
- Draw the animal **facing right**. Ledgeling mirrors it when it walks the other way.
- Its feet should touch the bottom edge of the frame. That bottom-centre point is
  where it meets the window edge, which is what `anchor` means.

Save over `sprites/idle.png` and `sprites/move.png`. The pet on screen changes as
soon as you save — no restart, nothing to press.

**No single animation is required — one is enough.** `idle` and `move` stand in
for each other, and if any other is missing Ledgeling uses one you did draw:

```
climb → move → idle        land  → idle        sleep → idle
jump  → move → idle        happy → idle        alert → idle
fall  → move → idle        takeoff → move → idle
busy  → preen → idle       attention → alert → happy → idle
dizzy → alert → idle       petted → happy → idle
stretch → idle
```

So a two-picture pack is a complete pet. Adding more art only ever makes it better.

`busy` and `attention` are the two assistant clips. A pet plays `busy` while it is
working on a task you gave it, and `attention` while that task is waiting for your
answer — a file change to approve, a command to allow. Both are optional like the
rest; **Pet packs → Details** tells you, per pack, which clip each slot is actually
playing, so a missing one is visible rather than silent.

---

## 3. Say what kind of animal it is (10 minutes)

Open `pet.json`. Change `name`, `author`, and `id` (the id must be unique — use
`com.yourname.something`). Then the part that matters:

```json
"locomotion": {
  "class": "climber",
  "surfaces": ["window_any", "monitor_any", "taskbar_top"],
  "speed": { "surface": 110, "climb": 85 },
  "jump": { "enabled": true, "max_gap": 160 },
  "gravity": 1.0
}
```

**`class`** is how it gets around:

| class | what it does |
|---|---|
| `flier` | flies straight to wherever it wants. Never falls. Birds, moths. |
| `climber` | walks edges, climbs sides, jumps gaps, falls when its window closes. Squirrels, geckos. |
| `walker` | horizontal edges only, and they must connect. Cats, ducks. |
| `hopper` | a walker that can hop short gaps. Frogs, sparrows on foot. |

**`surfaces`** is where it is allowed to be, and it does an enormous amount of
character work on its own:

```
window_top     window_bottom    window_left    window_right
monitor_top    monitor_bottom   monitor_left   monitor_right    taskbar_top
```
Shorthands: `window_any`, `window_vertical`, `monitor_any`, `all`.

A bird takes `window_top` and nothing else. A gecko takes `window_vertical` and
lives on the sides of your windows, and nowhere else. Same engine, different animal.

Add `"surface_orientation": "align"` in the `sprite` block if your animal should
lie flat against a vertical edge. Leave it `"upright"` for anything that would
look silly sideways — a bird perches on a window side, it does not lie on it.

Try things and watch. This is the fun part.

---

## 4. Give it a personality (optional, 15 minutes)

Leave `behavior` out entirely and your pet gets a sensible default for its class.
When you want more, describe what it does as a list of states:

```json
"behavior": {
  "initial": "perch",
  "states": {
    "perch": {
      "clip": "idle",
      "duration": [6, 20],
      "transitions": [
        { "to": "preen", "weight": 3 },
        { "to": "relocate", "weight": 2 },
        { "to": "doze", "weight": 6, "when": "idle_seconds > 240" }
      ]
    },
    "preen": { "clip": "preen", "duration": [3, 6], "transitions": [{ "to": "perch" }] },
    "relocate": { "clip": "move", "action": "move_to_random_surface",
                  "transitions": [{ "to": "perch" }] },
    "doze": { "clip": "sleep", "duration": [30, 120],
              "transitions": [{ "to": "perch", "when": "idle_seconds < 10" }] }
  }
}
```

- `duration` is seconds, or `[shortest, longest]` for a bit of variety.
- `weight` is how likely a transition is compared to the others in the same list.
- `action` is one of the movements Ledgeling already knows how to do:
  `move_to_random_surface`, `move_to_nearest_surface`, `move_toward_cursor`,
  `flee_from_cursor`, `patrol_current_surface`, `return_to_home_surface`,
  `move_toward_pet` (walk or hop to stand beside the nearest other pet; it does
  nothing when the pet is alone or cannot reach it).
- `when` is an optional condition. If it is false, that transition cannot happen.
- **Rare idles.** A tiny `weight` makes a transition rare. Against the 11 total in
  the example, `{ "to": "fluff", "weight": 0.2 }` fires about once in fifty
  changes of state. Give it its own clip (any name you like under `animations`)
  and a short `duration`; if the clip is missing the pet just idles. The bundled
  budgie (`fluff`, `dance`) and squirrel (`nibble`, `flip`) each have two.

Conditions are deliberately tiny — a signal, a comparison, a value, joined with
`&&` and `||`:

```
idle_seconds > 300
session_minutes > 50 && app_category == code
cursor_distance < 120 && energy > 0.4
hour >= 22 || hour < 6
```

The signals you may use: `idle_seconds`, `session_minutes`, `app_category`,
`hour`, `energy`, `mood`, `surface_count`, `cursor_distance`, `pet_distance`
(pixels to the nearest other pet; infinite when there is none),
`affection` (0 to 1, grows when the pet is petted and clicked, cools a little
during long absences), `days_together`, `is_dragging`.
`hour` is the local hour, 0–23. The engine also tires every pet late at night
(23:00–05:00) and perks it up in the morning (06:00–10:00), so `energy < 0.25`
rules already make pets sleepy at night; add `hour` rules for more.
Anything else is an error that tells you so, with the alternatives listed.

**Reactions** let events interrupt whatever the pet was doing:

```json
"reactions": [
  { "on": "surface_lost", "goto": "startled", "priority": 100 },
  { "on": "user_returned", "goto": "greet", "priority": 60, "cooldown": 60 },
  { "on": "long_session", "goto": "restless", "priority": 50, "cooldown": 900 }
]
```

`cooldown` is seconds, and it is how you keep a pet from becoming annoying. Use it
generously on anything to do with nudging.

**Picking up, throwing and petting.** Three events come from the pointer:

- `thrown` — the user let go of the pet while moving the pointer fast, so it flies.
  A pack with no `thrown` reaction gets plain `released` instead, as it always did.
- `landed_hard` — a thrown pet came down fast. Pair it with a `dizzy` clip.
- `petted` — the pointer was rubbed back and forth over the pet. Pair it with a
  `petted` clip (eyes closed, a heart) and a short state:

```json
"states": {
  "dizzy": { "clip": "dizzy", "duration": [2, 3], "transitions": [{ "to": "perch" }] },
  "enjoy": { "clip": "petted", "duration": [2, 4], "transitions": [{ "to": "perch" }] }
},
"reactions": [
  { "on": "thrown", "goto": "dizzy", "priority": 90 },
  { "on": "landed_hard", "goto": "dizzy", "priority": 90 },
  { "on": "petted", "goto": "enjoy", "priority": 60, "cooldown": 3 }
]
```

`pet_nearby` fires once when another pet comes within about 140 px, and again
only after they have parted by more than 200 px; use a long `cooldown` so a
greeting stays special.

`new_day` fires once when the user's first day with the pet rolls over to the
next calendar day it is on screen; a `greet` reaction with a long `cooldown`
makes a good morning hello.

Packs without a `behavior` block get these states and reactions for free; both
clips are optional and fall back to `alert`/`happy`, then `idle`.

---

## 5. When something is wrong

Open **Pet Lab** and press *Check* on your pack. Every problem names the file, the
exact place in it, what is wrong, and what was expected:

```
ERROR  pet.json → animations.climb.frames
       Expected a positive integer, found "6 " (string).
       Frame counts must be numbers, not quoted strings.
```

Pet Lab also has **scenario buttons**, which fire an event immediately so you can
see your `long_session` reaction without working for 45 minutes first, and a
**debug overlay** that draws the surfaces, the jumps your pet can and cannot make,
and its current path over your real windows. If a climber refuses to cross a gap,
that overlay is where you find out why.

---

## 6. Share it

Press **Export** on your pack. You get a `.petpack` file — anyone can drag it onto
their Ledgeling window to install it. Put a real licence in `LICENSE.txt` first so
people know what they may do with your drawing.

---

## Things worth knowing

- **A pack can never harm anyone.** There is no scripting and no way to add one.
  The worst a broken pack can do is look wrong.
- **Unknown settings are ignored**, with a note. A pack made for a newer Ledgeling
  still loads on an older one.
- **Pixel art scales best.** Ledgeling draws sprites without smoothing.
- **Big animals need wide windows.** Use `min_surface_length` to stop a cat from
  trying to stand on a 60-pixel-wide window edge.
