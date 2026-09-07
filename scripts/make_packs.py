"""Draw the bundled pet packs' sprite strips. Original pixel art, no assets.

Each frame is drawn on a 24x24 grid and doubled to the 48x48 the manifests
declare, which keeps the pixels crisp at the size the engine renders."""
from PIL import Image, ImageDraw
from pathlib import Path
import math

ROOT = Path(__file__).resolve().parents[1]
CELL = 24
SCALE = 2


def strip(frames, draw_frame):
    im = Image.new('RGBA', (CELL * frames, CELL))
    for i in range(frames):
        cell = Image.new('RGBA', (CELL, CELL))
        draw_frame(ImageDraw.Draw(cell), i)
        im.paste(cell, (i * CELL, 0))
    return im.resize((im.width * SCALE, im.height * SCALE), Image.NEAREST)


def write(pack, name, image):
    path = ROOT / 'packs' / pack / 'sprites' / f'{name}.png'
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path)


# --------------------------------------------------------------- budgie
BUD = dict(ink='#294d42', body='#84b887', light='#b7d69b', dark='#4b8467',
           head='#f2d77a', beak='#d89953', eye='#12241f')


def budgie_body(d, bob=0, wing='folded', head=0, eye_open=True, tuck=False):
    c = BUD
    o = lambda pts: [(x, y + bob) for x, y in pts]
    d.polygon(o([(8, 16), (6, 21), (9, 20), (12, 15)]), fill=c['dark'])
    d.polygon(o([(8, 9), (14, 8), (17, 11), (17, 16), (14, 19), (10, 19), (7, 15)]), fill=c['ink'])
    d.polygon(o([(9, 10), (14, 9), (16, 12), (16, 16), (13, 18), (10, 18), (8, 15)]), fill=c['body'])
    d.polygon(o([(12, 12), (16, 13), (16, 16), (13, 18), (11, 16)]), fill=c['light'])
    hy = head
    d.polygon(o([(11, 5 + hy), (15, 4 + hy), (18, 7 + hy), (17, 11 + hy), (13, 12 + hy), (10, 9 + hy)]), fill=c['ink'])
    d.polygon(o([(12, 5 + hy), (15, 5 + hy), (17, 8 + hy), (16, 11 + hy), (13, 11 + hy), (11, 8 + hy)]), fill=c['head'])
    d.polygon(o([(17, 8 + hy), (20, 9 + hy), (17, 10 + hy)]), fill=c['beak'])
    if eye_open:
        d.rectangle(o([(14, 7 + hy), (15, 8 + hy)]), fill=c['eye'])
        d.point((15, 7 + hy + bob), fill='#ffffff')
    else:
        d.line(o([(14, 8 + hy), (15, 8 + hy)]), fill=c['eye'])
    if not tuck:
        d.rectangle(o([(11, 19), (11, 21)]), fill=c['beak'])
        d.rectangle(o([(14, 19), (14, 21)]), fill=c['beak'])
    if wing == 'folded':
        d.polygon(o([(9, 11), (13, 12), (14, 16), (11, 17), (9, 15)]), fill=c['dark'])
        d.line(o([(10, 13), (12, 15)]), fill=c['light'])


def budgie_idle(d, i):
    budgie_body(d, bob=(0, 0, -1, 0)[i], eye_open=i != 3)


def budgie_move(d, i):
    wings = [[(11, 13), (3, 2), (1, 6), (4, 12), (9, 16)],
             [(11, 13), (2, 7), (1, 12), (8, 17)],
             [(11, 13), (4, 15), (6, 20), (10, 17)],
             [(11, 13), (5, 18), (8, 21), (11, 17)],
             [(11, 13), (3, 12), (2, 16), (9, 18)],
             [(11, 13), (2, 4), (1, 9), (7, 15)]]
    budgie_body(d, bob=(0, -1, 0, 1, 0, -1)[i], wing='out', tuck=True)
    d.polygon(wings[i], fill=BUD['dark'])
    d.polygon([(x + 1, y + 1) for x, y in wings[i][:-1]], fill=BUD['body'])


def budgie_land(d, i):
    budgie_body(d, bob=(-2, 0, 0)[i], wing='out' if i == 0 else 'folded')
    if i == 0:
        d.polygon([(11, 13), (4, 16), (7, 21), (11, 17)], fill=BUD['dark'])


def budgie_takeoff(d, i):
    budgie_body(d, bob=(0, -1, -3)[i], wing='out', tuck=i > 0)
    d.polygon([(11, 13), (3, 3 + i * 3), (1, 8 + i * 3), (8, 15)], fill=BUD['dark'])


def budgie_preen(d, i):
    budgie_body(d, head=(0, 3, 4, 2)[i], eye_open=i == 0)


def budgie_sleep(d, i):
    budgie_body(d, bob=i, head=2, eye_open=False, tuck=True)
    if i == 1:
        d.text((17, 2), 'z', fill=BUD['dark'])


def budgie_happy(d, i):
    budgie_body(d, bob=(0, -2, -3, -2, 0, -1)[i], head=(0, -1, -1, -1, 0, 0)[i])
    if i in (2, 3):
        d.point((20, 5), fill='#e58b7a')
        d.line((19, 6, 21, 6), fill='#e58b7a')


def budgie_alert(d, i):
    budgie_body(d, head=(0, -1, -1)[i])
    if i == 2:
        d.line((18, 2, 18, 4), fill=BUD['ink'])


# ------------------------------------------------------------- squirrel
SQ = dict(ink='#3a2118', body='#c1622d', light='#e2884a', belly='#f0d4b0',
          tail='#a8501f', eye='#20140f')


def squirrel_body(d, legs=0, tail=0, curl=0, eye_open=True):
    c = SQ
    d.polygon([(3, 16 - tail), (0, 9 - tail), (3, 3), (9, 3), (10, 8), (7, 13), (8, 17)], fill=c['ink'])
    d.polygon([(4, 15 - tail), (2, 9 - tail), (4, 4), (8, 4), (9, 9), (7, 13), (8, 16)], fill=c['tail'])
    d.polygon([(6, 12), (10, 9), (16, 10), (19, 13), (18, 18), (9, 18)], fill=c['ink'])
    d.polygon([(7, 12), (11, 10), (16, 11), (18, 14), (17, 17), (9, 17)], fill=c['body'])
    d.polygon([(10, 15), (16, 15), (17, 17), (10, 17)], fill=c['belly'])
    d.polygon([(15, 5), (19, 4), (22, 7), (21, 11), (17, 12), (14, 9)], fill=c['ink'])
    d.polygon([(16, 6), (19, 5), (21, 8), (20, 11), (17, 11), (15, 9)], fill=c['body'])
    d.polygon([(16, 5), (15, 2), (18, 3)], fill=c['ink'])
    d.polygon([(20, 4), (22, 2), (22, 5)], fill=c['ink'])
    d.polygon([(20, 9), (23, 10), (20, 11)], fill=c['light'])
    if eye_open:
        d.rectangle((18, 7, 19, 8), fill=c['eye'])
        d.point((19, 7), fill='#ffffff')
    else:
        d.line((18, 8, 19, 8), fill=c['eye'])
    for x in (10, 16):
        d.rectangle((x, 18, x + 1, max(18, 20 + legs)), fill=c['ink'])


def squirrel_idle(d, i):
    squirrel_body(d, tail=(0, 1, 1, 0)[i], eye_open=i != 3)


def squirrel_move(d, i):
    d_legs = (0, -1, 0, 1, 0, -1)[i]
    squirrel_body(d, legs=d_legs, tail=(1, 2, 1, 0, 1, 2)[i])


def squirrel_climb(d, i):
    squirrel_body(d, legs=(0, 1, 2, 1, 0, 1)[i], tail=(2, 1, 0, 1, 2, 1)[i])
    d.line((9, 19, 7, 21 - (i % 3)), fill=SQ['ink'])


def squirrel_jump(d, i):
    squirrel_body(d, legs=(2, 1, 0, 1)[i], tail=(0, 2, 3, 2)[i])


def squirrel_fall(d, i):
    squirrel_body(d, legs=2, tail=(3, 2)[i])


def squirrel_land(d, i):
    squirrel_body(d, legs=(-1, 0, 0)[i], tail=(0, 1, 0)[i])


def squirrel_sleep(d, i):
    squirrel_body(d, tail=-2 + i, eye_open=False)
    if i == 1:
        d.text((19, 1), 'z', fill=SQ['tail'])


def squirrel_alert(d, i):
    squirrel_body(d, tail=(1, 2, 2)[i])
    if i == 2:
        d.line((22, 1, 22, 3), fill=SQ['ink'])


def squirrel_happy(d, i):
    squirrel_body(d, legs=(0, -2, -3, -2, 0, 0)[i], tail=(1, 2, 3, 2, 1, 1)[i])


# ---------------------------------------------------------------- gecko
GK = dict(ink='#1f3a2b', body='#5aa06a', light='#8fc98a', spot='#3d7a52', eye='#f4e3a1')


def gecko_body(d, sway=0, foot=0, eye_open=True):
    c = GK
    d.polygon([(2, 13 + sway), (6, 11), (12, 11), (18, 10), (21, 12), (20, 15), (13, 16), (6, 16), (2, 15 + sway)], fill=c['ink'])
    d.polygon([(4, 13 + sway), (7, 12), (12, 12), (17, 11), (20, 13), (19, 14), (13, 15), (7, 15), (4, 14 + sway)], fill=c['body'])
    d.polygon([(17, 11), (21, 11), (22, 13), (20, 15), (17, 14)], fill=c['ink'])
    d.polygon([(18, 12), (20, 12), (21, 13), (19, 14), (18, 13)], fill=c['light'])
    for x in (9, 13, 17):
        d.point((x, 13), fill=c['spot'])
    if eye_open:
        d.point((20, 12), fill=c['eye'])
    for x, dy in ((7, 0), (15, 0)):
        d.line((x, 15, x - 2, 18 + foot), fill=c['ink'])
        d.line((x + 1, 12, x - 1, 9 - foot), fill=c['ink'])


def gecko_idle(d, i):
    gecko_body(d, sway=(0, 0, 1, 0)[i], eye_open=i != 2)


def gecko_move(d, i):
    gecko_body(d, sway=(0, 1, 0, -1)[i], foot=(0, 1, 0, 1)[i])


# ---------------------------------------------------------------- preview
def preview(pack, draw_frame, background):
    im = Image.new('RGBA', (128, 128), background)
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((10, 74, 118, 118), 6, fill='#ffffff40', outline='#ffffff70')
    cell = Image.new('RGBA', (CELL, CELL))
    draw_frame(ImageDraw.Draw(cell), 0)
    im.paste(cell.resize((CELL * 3, CELL * 3), Image.NEAREST), (28, 4), cell.resize((CELL * 3, CELL * 3), Image.NEAREST))
    out = im.resize((256, 256), Image.NEAREST)
    path = ROOT / 'packs' / pack / 'preview.png'
    path.parent.mkdir(parents=True, exist_ok=True)
    out.save(path)


CLIPS = {
    'budgie-classic': dict(idle=(4, budgie_idle), move=(6, budgie_move), land=(3, budgie_land),
                           takeoff=(3, budgie_takeoff), preen=(4, budgie_preen), sleep=(2, budgie_sleep),
                           happy=(6, budgie_happy), alert=(3, budgie_alert)),
    'squirrel-red': dict(idle=(4, squirrel_idle), move=(6, squirrel_move), climb=(6, squirrel_climb),
                         jump=(4, squirrel_jump), fall=(2, squirrel_fall), land=(3, squirrel_land),
                         sleep=(2, squirrel_sleep), alert=(3, squirrel_alert), happy=(6, squirrel_happy)),
    'gecko-glass': dict(idle=(4, gecko_idle), move=(4, gecko_move)),
}
BACKDROP = {'budgie-classic': '#e9eedd', 'squirrel-red': '#f0e6da', 'gecko-glass': '#dee9e2'}

if __name__ == '__main__':
    for pack, clips in CLIPS.items():
        for name, (frames, fn) in clips.items():
            write(pack, name, strip(frames, fn))
        preview(pack, clips['idle'][1], BACKDROP[pack])
        print(f'{pack}: {len(clips)} clips')
