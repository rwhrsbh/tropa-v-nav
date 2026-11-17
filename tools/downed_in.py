"""Takes a maimed hero's picture on the Edge, painted in Flow by tools/flow_downed_cmds.py, into the game.

    python tools/downed_in.py <flow picture> <hero><mark>

The Flow picture is an edit of `_inbox/flow_up/down_<hero>.png`, the hero's downed picture on a
green square; it is scaled back to that square, cut back to the downed picture's own sheet, keyed
(tools/key_flood.py) and saved as `assets/heroes/<hero><mark>_downed-gemini.png`, so it lies exactly
where the whole downed picture lies.
"""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from key_flood import key_flood  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
MARKS = ["_headless_noweapon", "_headless_noshield", "_headless_noarms", "_headless", "_noweapon", "_noshield", "_noarms"]


def main():
    src, stem = sys.argv[1:3]
    mark = next(m for m in MARKS if stem.endswith(m))
    hero = stem[: -len(mark)]
    whole = Image.open(os.path.join(ROOT, "assets", "heroes", f"{hero}_downed-gemini.png"))
    side = max(whole.size)
    pic = Image.open(src).convert("RGB").resize((side, side), Image.LANCZOS)
    x, y = (side - whole.width) // 2, (side - whole.height) // 2
    pic = pic.crop((x, y, x + whole.width, y + whole.height))
    keyed = key_flood(pic, tol=90)
    keyed.save(os.path.join(ROOT, "assets", "heroes", f"{stem}_downed-gemini.png"))
    print(stem + "_downed", keyed.size)


if __name__ == "__main__":
    main()
