#!/usr/bin/env python3
"""Draw the bundled Meadow decor pack: placeholder pixel art, standard library only.

    python3 scripts/make_decor_meadow.py

Writes packs/decor-meadow/{decor.json,LICENSE.txt,preview.png,sprites/*.png}.
The art is deliberately simple; redraw any sprite and keep its size.
"""
import json
import math
import os
import struct
import zlib

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "packs", "decor-meadow")

BARK, BARK_DARK = (122, 86, 58, 255), (92, 62, 42, 255)
LEAF, LEAF_DARK, LEAF_LIGHT = (92, 150, 72, 255), (62, 112, 56, 255), (138, 186, 96, 255)
PINE, PINE_DARK = (54, 112, 84, 255), (36, 82, 64, 255)
STONE, STONE_DARK, STONE_LIGHT = (150, 150, 142, 255), (112, 112, 106, 255), (186, 186, 176, 255)
PETAL, PETAL_DARK, CENTRE = (236, 132, 150, 255), (204, 96, 122, 255), (246, 206, 92, 255)
CAP, CAP_DARK, SPOT, STALK = (204, 72, 64, 255), (160, 52, 48, 255), (250, 244, 232, 255), (236, 226, 204, 255)
GRASS, GRASS_DARK = (118, 170, 76, 255), (80, 130, 58, 255)


class Canvas:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.px = [[(0, 0, 0, 0)] * w for _ in range(h)]

    def put(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[y][x] = c

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1):
            for x in range(x0, x1):
                self.put(x, y, c)

    def disc(self, cx, cy, r, c, shade=None):
        for y in range(int(cy - r) - 1, int(cy + r) + 2):
            for x in range(int(cx - r) - 1, int(cx + r) + 2):
                d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
                if d <= r:
                    lower_right = (x - cx) + (y - cy) > r * 0.5
                    self.put(x, y, shade if shade and lower_right else c)

    def blit(self, other, ox, oy):
        for y in range(other.h):
            for x in range(other.w):
                if other.px[y][x][3]:
                    self.put(ox + x, oy + y, other.px[y][x])

    def png(self):
        raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in self.px)
        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        header = struct.pack(">IIBBBBB", self.w, self.h, 8, 6, 0, 0, 0)
        return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def oak():
    c = Canvas(48, 64)
    c.rect(21, 34, 27, 64, BARK)
    c.rect(25, 34, 27, 64, BARK_DARK)
    c.rect(14, 22, 34, 24, BARK)  # the branch a pet can sit on
    for cx, cy, r in [(24, 16, 14), (12, 22, 9), (36, 22, 9), (18, 30, 8), (31, 30, 8)]:
        c.disc(cx, cy, r, LEAF, LEAF_DARK)
    for x, y in [(18, 8), (28, 11), (10, 19), (34, 18), (22, 24)]:
        c.put(x, y, LEAF_LIGHT)
        c.put(x + 1, y, LEAF_LIGHT)
    return c


def pine():
    c = Canvas(32, 64)
    c.rect(14, 52, 18, 64, BARK_DARK)
    for top, half in [(2, 7), (14, 11), (28, 14)]:
        for y in range(top, top + 24):
            w = int(half * (y - top) / 24) + 1
            for x in range(16 - w, 16 + w):
                c.put(x, y, PINE_DARK if x >= 16 else PINE)
    return c


def bush():
    c = Canvas(32, 20)
    for cx, cy, r in [(9, 13, 7), (17, 10, 9), (25, 13, 7)]:
        c.disc(cx, cy, r, LEAF, LEAF_DARK)
    c.rect(2, 19, 30, 20, LEAF_DARK)
    for x, y in [(12, 5), (20, 6), (6, 10)]:
        c.put(x, y, LEAF_LIGHT)
    return c


def flower():
    c = Canvas(12, 16)
    c.rect(5, 7, 7, 16, GRASS_DARK)
    c.rect(7, 11, 9, 12, GRASS)
    for dx, dy in [(-2, 0), (2, 0), (0, -2), (0, 2)]:
        c.disc(6 + dx, 5 + dy, 2, PETAL, PETAL_DARK)
    c.disc(6, 5, 1.2, CENTRE)
    return c


def rock():
    c = Canvas(20, 12)
    c.disc(10, 11, 9, STONE, STONE_DARK)
    c.rect(0, 11, 20, 12, (0, 0, 0, 0))
    c.rect(1, 11, 19, 12, STONE_DARK)
    c.put(7, 5, STONE_LIGHT)
    c.put(8, 5, STONE_LIGHT)
    return c


def tuft():
    c = Canvas(16, 10)
    for x, lean, height in [(2, 1, 6), (5, -1, 9), (8, 0, 8), (11, 1, 7), (13, -1, 5)]:
        for i in range(height):
            c.put(x + (lean * i) // 4, 9 - i, GRASS if i % 3 else GRASS_DARK)
    return c


def mushroom():
    c = Canvas(12, 12)
    c.rect(5, 6, 8, 12, STALK)
    c.disc(6, 6, 5.5, CAP, CAP_DARK)
    c.rect(0, 7, 12, 12, (0, 0, 0, 0))
    c.rect(5, 7, 8, 12, STALK)
    c.put(4, 3, SPOT)
    c.put(8, 4, SPOT)
    return c


PROPS = {
    # Pets sit on top of the canopy and the bush; ledges narrower than a pet
    # are widened by the engine, so the pet overhangs a little.
    "oak": (oak, {"sway": "tall", "perch": {"y": 0.05, "from": 0.15, "to": 0.85}}),
    "pine": (pine, {"sway": "tall"}),
    "bush": (bush, {"sway": "soft", "perch": {"y": 0.15, "from": 0.1, "to": 0.9}}),
    "flower": (flower, {"sway": "soft"}),
    "rock": (rock, {"sway": "stiff"}),
    "tuft": (tuft, {"sway": "soft"}),
    "mushroom": (mushroom, {"sway": "stiff"}),
}


def main():
    os.makedirs(os.path.join(ROOT, "sprites"), exist_ok=True)
    manifest = {
        "format": 1,
        "id": "com.perch.decor-meadow",
        "name": "Meadow",
        "author": "Ledgeling",
        "version": "1.0.0",
        "license": "CC-BY-4.0",
        "props": {},
    }
    preview = Canvas(64, 64)
    x = 0
    for name, (draw, extra) in PROPS.items():
        canvas = draw()
        with open(os.path.join(ROOT, "sprites", f"{name}.png"), "wb") as f:
            f.write(canvas.png())
        manifest["props"][name] = {
            "file": f"sprites/{name}.png",
            "width": canvas.w,
            "height": canvas.h,
            **extra,
        }
        if name in ("oak", "bush", "flower", "rock"):
            preview.blit(canvas, x, 64 - canvas.h)
            x += canvas.w // 2 + 4
    with open(os.path.join(ROOT, "preview.png"), "wb") as f:
        f.write(preview.png())
    with open(os.path.join(ROOT, "decor.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    with open(os.path.join(ROOT, "LICENSE.txt"), "w") as f:
        f.write(
            "The sprite art, preview image and manifest in this pack are released under\n"
            "Creative Commons Attribution 4.0 International (CC-BY-4.0).\n\n"
            "You may copy, modify and redistribute them, including commercially, as long as\n"
            "you credit the Ledgeling project. Full text: https://creativecommons.org/licenses/by/4.0/\n"
        )


if __name__ == "__main__":
    main()
