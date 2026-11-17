"""Takes a map token drawn by Flow on a plain light grey background into the game.

    python tools/maptoken.py <downloaded picture> <hero id>

The grey is flood-filled away from the picture's edges (so grey inside the figure stays), the
figure is trimmed, scaled to fit 160x160 with a little room above, its bottom on the bottom
edge, and saved as assets/map/hero_<id>-gemini.png. The original moves to _inbox/.
"""
import os
import shutil
import sys
from collections import deque

import numpy as np
from PIL import Image, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def key_grey(img, tol=28):
    a = np.asarray(img.convert("RGB")).astype(int)
    h, w, _ = a.shape
    bg = np.median(np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]]), axis=0)
    near = np.abs(a - bg).max(axis=2) < tol
    out = np.zeros((h, w), bool)
    q = deque((y, x) for y in range(h) for x in (0, w - 1) if near[y, x])
    q.extend((y, x) for x in range(w) for y in (0, h - 1) if near[y, x])
    for y, x in q:
        out[y, x] = True
    while q:
        y, x = q.popleft()
        for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            ny, nx = y + dy, x + dx
            if 0 <= ny < h and 0 <= nx < w and near[ny, nx] and not out[ny, nx]:
                out[ny, nx] = True
                q.append((ny, nx))
    alpha = Image.fromarray(np.where(out, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.2))
    rgba = img.convert("RGBA")
    rgba.putalpha(alpha)
    return rgba


def main():
    src, hero = sys.argv[1], sys.argv[2]
    img = key_grey(Image.open(src))
    box = img.getchannel("A").point(lambda v: 255 if v > 20 else 0).getbbox()
    fig = img.crop(box)
    k = min(160 / fig.width, 148 / fig.height)
    fig = fig.resize((max(1, round(fig.width * k)), max(1, round(fig.height * k))), Image.LANCZOS)
    out = Image.new("RGBA", (160, 160), (0, 0, 0, 0))
    out.alpha_composite(fig, ((160 - fig.width) // 2, 160 - fig.height))
    dst = os.path.join(ROOT, "assets", "map", f"hero_{hero}-gemini.png")
    out.save(dst, optimize=True)
    os.makedirs(os.path.join(ROOT, "_inbox"), exist_ok=True)
    shutil.move(src, os.path.join(ROOT, "_inbox", os.path.basename(src)))
    print(dst)


if __name__ == "__main__":
    main()
