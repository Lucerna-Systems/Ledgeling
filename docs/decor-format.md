# Decor packs

A decor pack is a set of props (trees, flowers, rocks) that people place on the
top edge of an app's window to make a **biome**. Like a pet pack, it is data:
PNG sprites and one JSON file, never code.

You can also draw one in **Studio → Decor**: name the pack, add props, pick a
size, a sway and an optional ledge, draw each one, and save. Studio writes the
same folder as below and checks it with the same loader.

## The folder

```
decor-meadow/
  decor.json
  preview.png      optional, shown in pickers
  LICENSE.txt      say what others may do with the art
  sprites/
    oak.png
    bush.png
```

It goes in the same packs folder as the pets. Ledgeling tells it apart by its
`decor.json`. The bundled **Meadow** pack (`packs/decor-meadow`) is a worked
example; `scripts/make_decor_meadow.py` draws its placeholder art.

## `decor.json`

```json
{
  "format": 1,
  "id": "com.yourname.meadow",
  "name": "Meadow",
  "author": "Your Name",
  "version": "1.0.0",
  "license": "CC-BY-4.0",
  "props": {
    "oak":  { "file": "sprites/oak.png", "width": 48, "height": 64, "sway": "tall",
              "perch": { "y": 0.05, "from": 0.15, "to": 0.85 } },
    "tuft": { "file": "sprites/tuft.png", "width": 16, "height": 10 }
  }
}
```

| Field | Required | What it means |
|---|---|---|
| `file` | yes | The sprite, relative to the pack. One frame, or a strip of frames side by side. |
| `width`, `height` | yes | Size in logical pixels, 1 to 128 each. |
| `anchorX` | no, 0.5 | Where along its width the prop stands on the edge, 0 to 1. |
| `frames`, `fps` | no, 1 and 0 | For an animated strip: how many frames, and how fast it loops. |
| `sway` | no, `soft` | How it moves when the window moves. See below. |
| `perch` | no | A ledge a pet can stand on. See below. |

A pack holds at most 32 props.

### Sway

`stiff` barely moves (rocks, mushrooms). `soft` wobbles quickly and settles
(grass, flowers, bushes). `tall` leans slowly and swings for a while (trees).
For finer control give numbers instead:

```json
"sway": { "stiffness": 140, "damping": 0.25, "softness": 1 }
```

`stiffness` (10 to 600) is how quickly it springs back, `damping` (0.05 to 1.5)
how soon it stops, and `softness` (0 to 2) how far the window's motion swings it.

When the user switches off **Squish & wobble**, or their system asks for reduced
motion, props stand still and only follow their window.

### Perch

`perch` marks a horizontal ledge across the prop: `y` is its height as a fraction
from the top of the prop (0) to its base (1), and `from` and `to` are where it
starts and ends across the width. A ledge narrower than a pet is widened so the
pet fits; it overhangs a little, which reads as sitting on it. Any pet that may
stand on a window's top edge may stand on a ledge.

## When something is wrong

Every problem names `decor.json`, the JSON path (for example `props.oak.sway`),
what was wrong and what was expected. A missing sprite names the sprite file.
