"""Turns a generated character on a green screen into a game sprite.

    python tools/cutout.py <source.png> <assets-relative stem> <width> <height> [--foot 0.957]

Keys out the green, removes the green spill from edge pixels, trims to the
figure, scales it into width x height and plants the feet on the baseline
(y = 980 of 1024 for heroes, as the asset spec asks). Writes `<stem>-gemini.png`.
"""
import os
import sys

import numpy as np
from PIL import Image, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")


def key_green(img):
    a = np.asarray(img.convert("RGB"), np.float32)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    # how much greener a pixel is than its other channels
    green = g - np.maximum(r, b)
    alpha = 1.0 - np.clip((green - 8.0) / 25.0, 0, 1)
    # aggressive despill: cap green at the brighter of red and blue wherever green > 0
    spill = np.clip(green, 0, None)
    a[..., 1] = g - spill
    alpha_img = Image.fromarray((alpha * 255).astype(np.uint8))
    # shave 2 pixels so no green fringe survives, then soften
    alpha_img = alpha_img.filter(ImageFilter.MinFilter(5)).filter(ImageFilter.GaussianBlur(0.8))
    out = np.dstack([np.clip(a, 0, 255), np.asarray(alpha_img, np.float32)]).astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def fit(img, w, h, foot):
    box = img.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox()
    if box is None:
        raise SystemExit("nothing left after keying: is the background really green?")
    fig = img.crop(box)
    room_h = h * foot - h * 0.02
    k = min((w * 0.96) / fig.width, room_h / fig.height)
    fig = fig.resize((max(1, int(fig.width * k)), max(1, int(fig.height * k))), Image.LANCZOS)
    sheet = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    sheet.alpha_composite(fig, ((w - fig.width) // 2, int(h * foot) - fig.height))
    return sheet


def main():
    foot = 0.957
    raw_args = sys.argv[1:]
    args = []
    i = 0
    while i < len(raw_args):
        if raw_args[i] == "--foot" and i + 1 < len(raw_args):
            foot = float(raw_args[i + 1])
            i += 2
        elif raw_args[i].startswith("--foot="):
            foot = float(raw_args[i].split("=")[1])
            i += 1
        elif raw_args[i].startswith("--"):
            i += 1
        else:
            args.append(raw_args[i])
            i += 1
    if len(args) != 4:
        raise SystemExit(__doc__)
    src, stem, w, h = args[0], args[1], int(args[2]), int(args[3])
    out = fit(key_green(Image.open(src)), w, h, foot)
    # Final despill on out sheet to ensure zero lanczos edge bleeding
    arr = np.asarray(out).copy()
    r = arr[..., 0].astype(float)
    g = arr[..., 1].astype(float)
    b = arr[..., 2].astype(float)
    excess = g - np.maximum(r, b)
    spill = np.clip(excess, 0, None)
    arr[..., 1] = (g - spill).astype(np.uint8)
    out = Image.fromarray(arr, "RGBA")
    path = os.path.join(ROOT, f"{stem}-gemini.png")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    out.save(path, optimize=True)
    print("wrote", path, out.size)


if __name__ == "__main__":
    main()
