"""Build Perch's original, four-frame pixel budgie strips (no external assets)."""
from PIL import Image, ImageDraw
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]

def bird(frame, flying=False):
    im = Image.new('RGBA', (32, 32))
    d = ImageDraw.Draw(im)
    ink, green, light, dark, yellow = '#294d42', '#84b887', '#b7d69b', '#4b8467', '#f2d77a'
    bob = (0, 0, -1, 0)[frame] if not flying else 0
    def poly(points, fill): d.polygon([(x,y+bob) for x,y in points], fill=fill)
    poly([(10,21),(7,27),(11,26),(15,20)], dark)
    poly([(11,12),(18,10),(23,14),(23,21),(20,25),(14,25),(10,21),(9,16)], ink)
    poly([(12,12),(18,12),(22,15),(22,21),(19,24),(14,24),(11,21),(10,16)], green)
    poly([(17,15),(21,15),(22,20),(20,23),(16,23),(15,20)], light)
    poly([(15,7),(20,6),(24,9),(24,14),(21,17),(16,15),(14,11)], ink)
    poly([(16,7),(20,7),(23,9),(23,14),(21,16),(17,14),(15,11)], yellow)
    poly([(23,11),(27,13),(23,14)], '#d89953')
    d.rectangle((20,9+bob,21,10+bob), fill=ink)
    if frame == 3 and not flying:
        d.rectangle((20,9+bob,21,9+bob), fill=yellow)
    d.point((21,9+bob), fill='#ffffff')
    d.rectangle((20,14+bob,21,15+bob), fill='#628e99')
    if flying:
        wings = [ [(14,17),(5,5),(3,8),(5,15),(12,21)], [(14,17),(2,12),(2,17),(11,22)], [(14,16),(5,23),(7,28),(13,23)], [(14,17),(5,12),(3,16),(12,22)] ]
        poly(wings[frame], dark)
        poly([(x+1,y) for x,y in wings[frame][:-1]], green)
    else:
        poly([(11,14),(15,15),(17,19),(14,22),(11,20)], dark)
        d.line((11,16+bob,14,19+bob), fill=light)
        d.line((11,18+bob,13,20+bob), fill=light)
    d.line((16,25+bob,16,27), fill='#b07d58')
    d.line((20,25+bob,20,27), fill='#b07d58')
    d.line((15,27,18,27), fill='#b07d58')
    d.line((19,27,22,27), fill='#b07d58')
    return im

for name in ['idle', 'move']:
    strip = Image.new('RGBA',(128,32))
    for i in range(4): strip.paste(bird(i,name=='move'),(32*i,0))
    strip.resize((512,128),Image.Resampling.NEAREST).save(ROOT/'public'/f'{name}.png')
icon = Image.new('RGBA',(256,256),'#f1f0e6')
icon.alpha_composite(bird(0).resize((224,224),Image.Resampling.NEAREST),(16,4))
icon.save(ROOT/'src-tauri/icons/icon.png')
icon.save(ROOT/'src-tauri/icons/icon.icns')
masks=[]
for flying in [False,True]:
    for frame in range(4):
        im=bird(frame,flying)
        masks.append([sum(1<<x for x in range(32) if im.getpixel((x,y))[3]>0) for y in range(32)])
(ROOT/'src-tauri/src/alpha_masks.in').write_text(str(masks)+'\n')
