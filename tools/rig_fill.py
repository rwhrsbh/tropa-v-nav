"""Gives back to a foe's rig parts what fell between them when it was cut.

    python tools/rig_fill.py [<id> ...] [--dry]

A head and a body cut from Flow's colour mask and beheaded picture often miss a rim of the resting
picture: the tip of a snout the mask did not paint, a flame, a strand of hair, the outline Flow ate
away. In the game those pixels are simply gone (Kot Bayun's muzzle came out cut flat). Every pixel
of the resting picture that no part holds goes to the nearer of the head and the body (arm and
shield parts count as holding theirs), copied from the resting picture. With no ids, every foe
with a `_head` and a `_body`. `--dry` only writes target/rig_fill.png: each foe before (missing in
red) and after, to look at first.
"""
import glob
import os
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
EN = os.path.join(ROOT, "assets", "enemies")


def rgba(p):
    return np.array(Image.open(p).convert("RGBA"))


def fill(foe, dry):
    rest, body, head = (rgba(os.path.join(EN, f"{foe}{s}-gemini.png")) for s in ("", "_body", "_head"))
    if not rest.shape == body.shape == head.shape:
        return None
    held = (body[..., 3] > 24) | (head[..., 3] > 24)
    for part in ("arm", "shield"):
        p = os.path.join(EN, f"{foe}_{part}-gemini.png")
        if os.path.exists(p):
            held |= rgba(p)[..., 3] > 24
    miss = (rest[..., 3] > 24) & ~held
    if miss.sum() == 0:
        return None
    to_head = ndimage.distance_transform_edt(head[..., 3] <= 24) < ndimage.distance_transform_edt(body[..., 3] <= 24)
    before = Image.new("RGBA", rest.shape[1::-1], (90, 90, 90, 255))
    before.alpha_composite(Image.fromarray(rest))
    b = np.array(before)
    b[miss] = [255, 0, 0, 255]
    for part, pick in ((head, miss & to_head), (body, miss & ~to_head)):
        part[pick] = rest[pick]
    if not dry:
        Image.fromarray(body).save(os.path.join(EN, f"{foe}_body-gemini.png"))
        Image.fromarray(head).save(os.path.join(EN, f"{foe}_head-gemini.png"))
    after = Image.new("RGBA", rest.shape[1::-1], (90, 90, 90, 255))
    after.alpha_composite(Image.fromarray(body))
    after.alpha_composite(Image.fromarray(head))
    return Image.fromarray(b), after, int(miss.sum())


def main():
    dry = "--dry" in sys.argv
    ids = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not ids:
        ids = sorted(os.path.basename(f)[: -len("_head-gemini.png")] for f in glob.glob(os.path.join(EN, "*_head-gemini.png"))
                     if os.path.exists(f.replace("_head-", "_body-")) and os.path.exists(f.replace("_head-", "-")))
    tiles = []
    for foe in ids:
        r = fill(foe, dry)
        if r:
            before, after, n = r
            print(foe, n)
            for t in (before, after):
                t.thumbnail((220, 220))
                tiles.append(t.convert("RGB"))
    if tiles:
        cols = 8
        sheet = Image.new("RGB", (cols * 220, ((len(tiles) + cols - 1) // cols) * 220), (40, 40, 40))
        for i, t in enumerate(tiles):
            sheet.paste(t, ((i % cols) * 220, (i // cols) * 220))
        sheet.save(os.path.join(ROOT, "target", "rig_fill.png"))


if __name__ == "__main__":
    main()
