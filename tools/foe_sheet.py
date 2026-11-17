"""Sheets a foe's every picture to look at: is it all there and does it all fit together?

    python tools/foe_sheet.py <id> [<id> ...]        # target/foe_<id>.png each
    python tools/foe_sheet.py --all <file>           # the ids listed in a file, one per line

One row per foe: the resting picture; head over body with the rig's head joint marked (red dot,
read from crates/game/src/battle/rig.rs) and anything the parts miss of the resting picture in
magenta; the head alone; both corpses; every pose (`_strike`, `_hurt`, `_guard`, `_cast` and any
`_<move id>`). On grey, so a black figure shows and a green fringe stands out.
"""
import glob
import os
import re
import sys

import numpy as np
from PIL import Image, ImageDraw

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
EN = os.path.join(ROOT, "assets", "enemies")
T = 220


def joints():
    text = open(os.path.join(ROOT, "crates", "game", "src", "battle", "rig.rs"), encoding="utf-8").read()
    return {m.group(1): (float(m.group(2)), float(m.group(3))) for m in re.finditer(r'\("enemies/([a-z_]+)", &\[\(Part::Head, Vec2::new\(([0-9.]+), ([0-9.]+)\)', text)}


def tile(im, label, mark=None):
    bg = Image.new("RGBA", im.size, (90, 90, 90, 255))
    bg.alpha_composite(im.convert("RGBA"))
    t = bg.convert("RGB")
    if mark:
        d = ImageDraw.Draw(t)
        x, y = mark[0] * t.width, mark[1] * t.height
        r = max(6, t.width // 60)
        d.ellipse((x - r, y - r, x + r, y + r), fill=(255, 0, 0))
    t.thumbnail((T, T))
    out = Image.new("RGB", (T, T + 16), (40, 40, 40))
    out.paste(t, ((T - t.width) // 2, 16 + (T - t.height) // 2))
    ImageDraw.Draw(out).text((3, 2), label, fill=(255, 230, 120))
    return out


def sheet(foe, J):
    path = lambda s: os.path.join(EN, f"{foe}{s}-gemini.png")
    if not os.path.exists(path("")):
        return None
    tiles = [tile(Image.open(path("")), foe)]
    if os.path.exists(path("_body")) and os.path.exists(path("_head")):
        rest = np.array(Image.open(path("")).convert("RGBA"))
        body, head = Image.open(path("_body")).convert("RGBA"), Image.open(path("_head")).convert("RGBA")
        both = body.copy()
        both.alpha_composite(head)
        held = np.array(both)[..., 3] > 24
        for part in ("_arm", "_shield"):
            if os.path.exists(path(part)):
                held |= np.array(Image.open(path(part)).convert("RGBA"))[..., 3] > 24
        a = np.array(both)
        if a.shape == rest.shape:
            miss = (rest[..., 3] > 24) & ~held
            a[miss] = [255, 0, 255, 255]
        tiles.append(tile(Image.fromarray(a), "head+body", J.get(foe)))
        tiles.append(tile(head, "head"))
    for s in ("_dead", "_dead_headless"):
        if os.path.exists(path(s)):
            tiles.append(tile(Image.open(path(s)), s[1:]))
    for p in sorted(glob.glob(path("_*"))):
        s = os.path.basename(p)[len(foe):-len("-gemini.png")]
        if s in ("_body", "_head", "_dead", "_dead_headless", "_arm", "_shield") or s.startswith("_ground") and foe != "kot_bayun":
            continue
        tiles.append(tile(Image.open(p), s[1:]))
    out = Image.new("RGB", (T * len(tiles), T + 16), (40, 40, 40))
    for i, t in enumerate(tiles):
        out.paste(t, (i * T, 0))
    return out


def main():
    ids = open(sys.argv[2]).read().split() if sys.argv[1] == "--all" else sys.argv[1:]
    J = joints()
    for foe in ids:
        s = sheet(foe, J)
        if s:
            s.save(os.path.join(ROOT, "target", f"foe_{foe}.png"))
    print(len(ids))


if __name__ == "__main__":
    main()
