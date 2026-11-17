"""Takes Gorynych into the game as one body with three heads on their own necks.

    python tools/gorynych_in.py [<dir>]

Reads from `<dir>` (default `_inbox/flow_up`) the Flow pictures `gor3.png` (the whole dragon),
`gor3_body.png` (the same dragon, all three necks cut halfway up), `gor3_mask.png` (the same
dragon with the fire head magenta, the venom head cyan, the thunder head yellow), `gor3_dead.png`
and `gor3_deadhl.png` (lying dead, with and without heads). Writes into assets/enemies, all on
the same 1024 sheet so they lie exactly over each other:

- `gorynych-gemini.png` the whole dragon, `gorynych_body-gemini.png` the body with three stumps;
- `gorynych_<fire|poison|thunder>_head-gemini.png`: each head with the part of its neck the body
  lacks (whatever of the whole dragon the body does not hold goes to the nearest mask colour);
- `gorynych_dead-gemini.png`, `gorynych_dead_headless-gemini.png` (1536x1024).

It prints each head's neck joint (where it meets its stump, as fractions of the sheet) for
`crates/game/src/battle/rig.rs`, and writes target/gorynych.png to look at.
"""
import os
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from key_flood import key_flood  # noqa: E402
from rig_in import FOOT, LONG, despeckle, figure  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
EN = os.path.join(ROOT, "assets", "enemies")
SIDE = 1024
HEADS = (("fire", (255, 0, 255)), ("poison", (0, 255, 255)), ("thunder", (255, 255, 0)))


def main():
    src = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "_inbox", "flow_up")
    pic = lambda n: os.path.join(src, n)
    rest = key_flood(Image.open(pic("gor3.png")), tol=90)
    body = key_flood(Image.open(pic("gor3_body.png")).resize(rest.size, Image.LANCZOS), tol=90)
    mask = np.array(Image.open(pic("gor3_mask.png")).convert("RGB").resize(rest.size, Image.NEAREST)).astype(int)
    # one placing for all: the whole dragon's box, fitted to the sheet, feet near the bottom
    box = rest.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox()
    k = min(0.94 * SIDE / (box[2] - box[0]), 0.92 * SIDE / (box[3] - box[1]))
    at = ((SIDE - round((box[2] - box[0]) * k)) // 2, round(SIDE * 0.957) - round((box[3] - box[1]) * k))

    def place(im):
        im = im.crop(box)
        im = im.resize((max(1, round(im.width * k)), max(1, round(im.height * k))), Image.LANCZOS)
        sheet = Image.new("RGBA", (SIDE, SIDE))
        sheet.alpha_composite(im, at)
        return sheet

    R, B = np.array(place(rest)), np.array(place(body))
    M = np.array(place(Image.fromarray(np.dstack([mask, np.full(mask.shape[:2], 255)]).astype(np.uint8), "RGBA")))[..., :3].astype(int)
    whole, held = R[..., 3] > 24, B[..., 3] > 24
    # each head takes the whole dragon's paint from its mask down over its bloody stump on the
    # body, so a living head hides its stump and a cut one leaves it
    red = (B[..., 0].astype(int) > 80) & (B[..., 0].astype(int) > 1.6 * B[..., 1]) & held
    marks, dist = [], []
    for _, col in HEADS:
        near = (np.abs(M - np.array(col)).sum(axis=2) < 150) & whole
        marks.append(near)
        dist.append(ndimage.distance_transform_edt(~near) if near.any() else np.full(whole.shape, 1e9))
    owner = np.argmin(np.stack(dist), axis=0)
    out = {"gorynych": Image.fromarray(R), "gorynych_body": Image.fromarray(B)}
    joints = []
    for i, (name, _) in enumerate(HEADS):
        part = np.zeros_like(R)
        stump = red & (owner == i) & (dist[i] < 260)
        reach = dist[i][stump].max() + 8 if stump.any() else 60
        sel = whole & (owner == i) & (dist[i] <= reach)
        part[sel] = R[sel]
        out[f"gorynych_{name}_head"] = Image.fromarray(part)
        # the neck joint: where this piece touches the body
        seam = sel & ndimage.binary_dilation(held, iterations=3)
        ys, xs = np.nonzero(seam if seam.any() else sel)
        joints.append((name, xs.mean() / SIDE, ys.mean() / SIDE))
    for name, im in out.items():
        path = os.path.join(EN, f"{name}-gemini.png")
        im.save(path)
        if name != "gorynych":
            despeckle(path)
    w = SIDE * 3 // 2
    d = figure(pic("gor3_dead.png"))
    kd = LONG * SIDE / max(d.width, d.height * 1.4)
    corpses = []
    for name, fig in (("gorynych_dead", d), ("gorynych_dead_headless", figure(pic("gor3_deadhl.png")))):
        fig = fig.resize((max(1, round(fig.width * kd)), max(1, round(fig.height * kd))), Image.LANCZOS)
        sheet = Image.new("RGBA", (w, SIDE))
        sheet.alpha_composite(fig, ((w - fig.width) // 2, int(SIDE * FOOT) - fig.height))
        sheet.save(os.path.join(EN, f"{name}-gemini.png"))
        despeckle(os.path.join(EN, f"{name}-gemini.png"))
        corpses.append(sheet)
    # to look at: body alone, each head alone in its place, the heads over the body, the corpses
    tiles = [out["gorynych_body"]] + [out[f"gorynych_{n}_head"] for n, _ in HEADS]
    over = out["gorynych_body"].copy()
    for n, _ in HEADS:
        over.alpha_composite(out[f"gorynych_{n}_head"])
    tiles.append(over)
    sheet = Image.new("RGBA", (5 * 320, 320 + 214), (90, 90, 90, 255))
    for i, t in enumerate(tiles):
        sheet.alpha_composite(t.resize((320, 320)), (i * 320, 0))
    for i, c in enumerate(corpses):
        sheet.alpha_composite(c.resize((320, 214)), (i * 320, 320))
    sheet.convert("RGB").save(os.path.join(ROOT, "target", "gorynych.png"))
    for name, x, y in joints:
        print(f'    ("enemies/gorynych_{name}_head", joint {x:.3f}, {y:.3f})')


if __name__ == "__main__":
    main()
