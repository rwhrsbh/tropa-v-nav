"""Takes a pose painted in Flow on flat green into the game at the same scale as its resting picture.

    python tools/pose_in.py <flow picture> <rest stem> <out stem> [<scale source>]

<rest stem> is the resting picture in assets/enemies (e.g. `kot_bayun_ground`), <out stem> the
pose to write (e.g. `kot_bayun_ground_strike`). The figure is keyed by flooding the background in
from the edges (tools/key_flood.py), scaled as the resting picture's own source was (the Flow
picture it was cut from, <scale source>, by default `_inbox/flow_up/<rest stem>_src.png`), and
stood on the same foot line. A pose wider than the resting picture gets a wider sheet; the game
draws it at its own width (`rig::fit`).
"""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from key_flood import key_flood  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def figure(path):
    im = key_flood(Image.open(path), tol=90)
    return im.crop(im.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox())


def main():
    src, rest, out = sys.argv[1:4]
    scale_src = sys.argv[4] if len(sys.argv) > 4 else os.path.join(ROOT, "_inbox", "flow_up", f"{rest}_src.png")
    rest_pic = Image.open(os.path.join(ROOT, "assets", "enemies", f"{rest}-gemini.png")).convert("RGBA")
    rb = rest_pic.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox()
    # how much the resting picture's source was shrunk on its way in
    k = (rb[2] - rb[0]) / figure(scale_src).width
    fig = figure(src)
    fig = fig.resize((max(1, round(fig.width * k)), max(1, round(fig.height * k))), Image.LANCZOS)
    w, h = rest_pic.size
    w = max(w, fig.width + 40)
    sheet = Image.new("RGBA", (w, h))
    sheet.alpha_composite(fig, ((w - fig.width) // 2, rb[3] - fig.height))
    sheet.save(os.path.join(ROOT, "assets", "enemies", f"{out}-gemini.png"))
    print(out, sheet.size, "scale %.3f" % k)


if __name__ == "__main__":
    main()
