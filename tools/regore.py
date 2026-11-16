"""Takes a hero sprite repainted in Flow (on its plain light grey background) back into the game
in place of the one it was made from: assets/heroes/<stem>-gemini.png.

    python tools/regore.py <downloaded picture> <stem> [<frame stem>]   # e.g. ... kuzma_headless_cast

A sprite that does not exist yet (a new maimed pose) is laid over the frame of <frame stem>, the
pose it was made from.

Flow hands the picture back in its own frame. The grey is flood-filled away from the picture's
edges, the figure is laid over the old sprite's frame by matching the figures' boxes (sides and
feet: a new bloody stump may make the top differ), and the old file is kept in _inbox/regore/.
"""
import os
import shutil
import sys
from collections import deque

import numpy as np
from PIL import Image, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def key_grey(img, tol=26):
    """Clears the plain grey: every patch of it, open to the edge or shut in between the legs, and
    the pale grey floor shadow Flow likes to lay under the feet where it touches the grey."""
    from scipy import ndimage
    a = np.asarray(img.convert("RGB")).astype(int)
    bg = np.median(np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]]), axis=0)
    near = np.abs(a - bg).max(axis=2) < tol
    lab, n = ndimage.label(near)
    sizes = ndimage.sum(near, lab, range(1, n + 1))
    edge = set(np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]]))) - {0}
    keep = [i + 1 for i in range(n) if sizes[i] > 150 or (i + 1) in edge]
    out = np.isin(lab, keep)
    # a soft shadow: pale and without colour, joined to the grey already cleared
    pale = (a.min(axis=2) > 120) & (a.max(axis=2) - a.min(axis=2) < 20)
    plab, pn = ndimage.label(pale)
    touching = np.unique(plab[ndimage.binary_dilation(out, iterations=2) & pale])
    out |= np.isin(plab, touching[touching > 0])
    alpha = Image.fromarray(np.where(out, 0, 255).astype(np.uint8)).filter(ImageFilter.MinFilter(3)).filter(ImageFilter.GaussianBlur(0.8))
    rgba = img.convert("RGBA")
    rgba.putalpha(alpha)
    return rgba


def main():
    src, stem = sys.argv[1], sys.argv[2]
    dst = os.path.join(ROOT, "assets", "heroes", f"{stem}-gemini.png")
    frame = dst if os.path.exists(dst) else os.path.join(ROOT, "assets", "heroes", f"{sys.argv[3]}-gemini.png")
    old = Image.open(frame).convert("RGBA")
    W, H = old.size
    ob = old.getchannel("A").point(lambda v: 255 if v > 60 else 0).getbbox()
    new = key_grey(Image.open(src))
    if not os.path.exists(dst) and abs(new.width / new.height - W / H) < 0.02:
        # a new pose made from the sprite itself: Flow keeps the sprite's frame, so it fits as it is
        out = new.resize((W, H), Image.LANCZOS)
        out.save(dst, optimize=True)
        keep = os.path.join(ROOT, "_inbox", "regore")
        os.makedirs(keep, exist_ok=True)
        shutil.move(src, os.path.join(keep, os.path.basename(src)))
        print(dst, "fitted to the frame")
        return
    nb = new.getchannel("A").point(lambda v: 255 if v > 60 else 0).getbbox()
    # scale by the height (feet to neck) and lay the feet where they stood, centred as before
    k = (ob[3] - ob[1]) / max(nb[3] - nb[1], 1)
    fig = new.crop(nb)
    fig = fig.resize((max(1, round(fig.width * k)), max(1, round(fig.height * k))), Image.LANCZOS)
    out = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    out.alpha_composite(fig, (round((ob[0] + ob[2]) / 2 - fig.width / 2), max(0, ob[3] - fig.height)))
    keep = os.path.join(ROOT, "_inbox", "regore")
    os.makedirs(keep, exist_ok=True)
    if os.path.exists(dst) and not os.path.exists(os.path.join(keep, f"{stem}-gemini.png")):
        shutil.copy(dst, os.path.join(keep, f"{stem}-gemini.png"))
    out.save(dst, optimize=True)
    shutil.move(src, os.path.join(keep, os.path.basename(src)))
    print(dst, "scale %.3f" % k)


if __name__ == "__main__":
    main()
