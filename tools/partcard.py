"""Makes the cards a hero carries when a head or an arm is cut off (own_head_/own_weapon_/
own_shield_<hero>) from the hero's own cut-out parts, the way the first heroes' were made.

    python tools/partcard.py <hero> [<hero> ...]

Each part (assets/heroes/<hero>_head, _arm and _shield, cut by tools/rigparts.py) is taken as it
is painted, fitted into the middle of the card (at most 630 tall and 560 wide), and laid with a
soft dark shadow on the dark backdrop all these cards share (docs/prompts/rig/part_bg.png, the
pixel median of the first heroes' cards). Writes assets/cards/own_<kind>_<hero>-gemini.png.
"""
import os
import sys

import numpy as np
from PIL import Image, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
KINDS = {"head": "head", "weapon": "arm", "shield": "shield"}
CX, CY = 500, 372


def card(hero, kind, part):
    bg = Image.open(os.path.join(ROOT, "docs", "prompts", "rig", "part_bg.png")).convert("RGB")
    p = Image.open(os.path.join(ROOT, "assets", "heroes", f"{hero}_{part}-gemini.png")).convert("RGBA")
    box = p.getchannel("A").point(lambda v: 255 if v > 60 else 0).getbbox()
    p = p.crop(box)
    k = min(630 / p.height, 560 / p.width)
    p = p.resize((round(p.width * k), round(p.height * k)), Image.LANCZOS)
    at = (CX - p.width // 2, CY - p.height // 2)
    a = Image.new("L", bg.size, 0)
    a.paste(p.getchannel("A"), at)
    shadow = np.asarray(a.filter(ImageFilter.MaxFilter(9)).filter(ImageFilter.GaussianBlur(14))).astype(float) / 255
    out = np.asarray(bg).astype(float) * (1 - 0.55 * shadow[..., None])
    out = Image.fromarray(out.clip(0, 255).astype(np.uint8)).convert("RGBA")
    out.alpha_composite(p, at)
    dst = os.path.join(ROOT, "assets", "cards", f"own_{kind}_{hero}-gemini.png")
    out.convert("RGB").save(dst, optimize=True)
    print(dst)


for hero in sys.argv[1:]:
    for kind, part in KINDS.items():
        card(hero, kind, part)
