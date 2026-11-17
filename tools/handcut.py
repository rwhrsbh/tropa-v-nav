"""Turns a Flow picture of a hand on a green screen into a chest rock-paper-scissors hand.

    python tools/handcut.py <source.jpg> <hero> <rock|paper|scissors>

Keys out the green (as cutout.py does), trims to the arm, keeps it against the left edge the way
the older heroes' hands are, scales it to 384 wide and writes assets/hands/<hero>_<gesture>.png.
"""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cutout import key_green  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
WIDTH = 384


def main():
    if len(sys.argv) != 4 or sys.argv[3] not in ("rock", "paper", "scissors"):
        raise SystemExit(__doc__)
    src, hero, gesture = sys.argv[1:]
    im = key_green(Image.open(src))
    box = im.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox()
    if box is None:
        raise SystemExit("nothing left after keying: is the background really green?")
    im = im.crop(box)
    k = WIDTH / im.width
    im = im.resize((WIDTH, max(1, round(im.height * k))), Image.LANCZOS)
    out = os.path.join(ROOT, "assets", "hands", f"{hero}_{gesture}.png")
    im.save(out, optimize=True)
    print("wrote", out, im.size)


main()
