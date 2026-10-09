#!/usr/bin/env python3
"""Draws the newpub app icon (an original design): a teal tile with a newsletter page on it.

Writes assets/icon/newpub.png (1024 px), newpub-256.png (window icon) and newpub.ico (Windows).
The macOS .icns is made from newpub.png on the packaging runner (sips + iconutil).
"""
import os
from PIL import Image, ImageDraw

S = 1024
OUT = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "icon")


def draw(size: int) -> Image.Image:
    k = size / S
    im = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    r = lambda *v: [round(x * k) for x in v]
    # Tile with a vertical teal gradient.
    tile = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    td = ImageDraw.Draw(tile)
    for y in range(size):
        t = y / size
        c = (round(18 + 10 * t), round(122 - 30 * t), round(130 - 20 * t), 255)
        td.line([(0, y), (size, y)], fill=c)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(r(64, 64, 960, 960), radius=round(200 * k), fill=255)
    im.paste(tile, (0, 0), mask)
    # Page with a folded corner and a soft shadow.
    d.rounded_rectangle(r(262, 214, 782, 854), radius=round(24 * k), fill=(0, 0, 0, 60))
    d.polygon([tuple(r(246, 190)), tuple(r(666, 190)), tuple(r(766, 290)), tuple(r(766, 830)), tuple(r(246, 830))],
              fill=(250, 250, 246, 255))
    d.polygon([tuple(r(666, 190)), tuple(r(666, 290)), tuple(r(766, 290))], fill=(214, 220, 216, 255))
    # Masthead bar, two text columns and a picture block.
    d.rounded_rectangle(r(306, 262, 610, 322), radius=round(10 * k), fill=(232, 96, 60, 255))
    for i in range(6):
        y = 384 + i * 62
        d.rounded_rectangle(r(306, y, 486, y + 26), radius=round(8 * k), fill=(70, 84, 88, 255))
    d.rounded_rectangle(r(526, 384, 706, 570), radius=round(12 * k), fill=(18, 122, 130, 255))
    for i in range(3):
        y = 632 + i * 62
        d.rounded_rectangle(r(526, y, 706, y + 26), radius=round(8 * k), fill=(70, 84, 88, 255))
    return im


big = draw(S)
os.makedirs(OUT, exist_ok=True)
big.save(os.path.join(OUT, "newpub.png"))
draw(256).save(os.path.join(OUT, "newpub-256.png"))
big.save(os.path.join(OUT, "newpub.ico"), sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("icons written to", os.path.abspath(OUT))
