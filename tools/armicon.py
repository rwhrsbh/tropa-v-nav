"""Cuts each hero's arms out of their rig parts into small square pictures for the hollow's
"lost an arm" box.

    python tools/armicon.py

assets/heroes/<hero>_arm-gemini.png (the weapon arm) and <hero>_shield-gemini.png (the other arm)
are the size of the whole figure; the arm is cropped to its own box, set in the middle of a square
and written as assets/hands/<hero>_arm_weapon-gemini.png and <hero>_arm_shield-gemini.png.
"""
import glob
import os

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
SIZE = 256
PAD = 0.06

for path in sorted(glob.glob(os.path.join(ROOT, "assets", "heroes", "*_arm-gemini.png"))):
    hero = os.path.basename(path)[: -len("_arm-gemini.png")]
    if hero.endswith("_beast"):
        continue
    for part, word in (("arm", "weapon"), ("shield", "shield")):
        src = os.path.join(ROOT, "assets", "heroes", f"{hero}_{part}-gemini.png")
        if not os.path.exists(src):
            continue
        im = Image.open(src).convert("RGBA")
        box = im.getchannel("A").point(lambda a: 255 if a > 25 else 0).getbbox()
        if not box:
            continue
        im = im.crop(box)
        side = int(max(im.size) * (1 + 2 * PAD))
        sq = Image.new("RGBA", (side, side))
        sq.alpha_composite(im, ((side - im.width) // 2, (side - im.height) // 2))
        out = os.path.join(ROOT, "assets", "hands", f"{hero}_arm_{word}-gemini.png")
        sq.resize((SIZE, SIZE), Image.LANCZOS).save(out)
        print(out)
