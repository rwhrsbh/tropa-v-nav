"""Takes hero sprites downloaded from Google Flow (green screen) into the game.

    python tools/heroart.py <stem> [<stem> ...]     # e.g. kuzma kuzma_strike kuzma_beast_cast
    python tools/heroart.py --portrait <hero>        # a head and shoulders, 512x512

The waiting downloads (see flowart.py) are taken oldest first, one per stem: keyed out with
cutout.py, fitted into 768x1024 (1024x1024 for a stem with `_beast`) with the feet on the
baseline, and saved as assets/heroes/<stem>-gemini.png. `-` throws a picture away. The
originals move to `_inbox/`.
"""
import os
import shutil
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cutout import fit, key_green  # noqa: E402
from flowart import ROOT, waiting  # noqa: E402


def main():
    args = sys.argv[1:]
    portrait = args[:1] == ["--portrait"]
    if portrait:
        args = args[1:]
    files = waiting()
    if len(files) < len(args):
        print(f"{len(files)} downloads waiting but {len(args)} names given; nothing moved")
        return 1
    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    for f, stem in zip(files, args):
        if stem != "-":
            im = Image.open(f).convert("RGB")
            if portrait:
                side = min(im.size)
                im = im.crop(((im.width - side) // 2, 0, (im.width - side) // 2 + side, side)).resize((512, 512), Image.LANCZOS)
                im.save(os.path.join(ROOT, "assets", "portraits", f"{stem}-gemini.png"))
            else:
                w = 1024 if stem.startswith("volkolak_beast") else 768
                fit(key_green(im), w, 1024, 0.957).save(os.path.join(ROOT, "assets", "heroes", f"{stem}-gemini.png"), optimize=True)
            print(stem, im.size)
        shutil.move(f, os.path.join(inbox, os.path.basename(f)))
    return 0


if __name__ == "__main__":
    sys.exit(main())


def portrait(hero):
    """The head and shoulders of a hero's standing sprite on the dark of the lobby, 512x512."""
    sprite = Image.open(os.path.join(ROOT, "assets", "heroes", f"{hero}-gemini.png")).convert("RGBA")
    box = sprite.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox()
    side = int((box[3] - box[1]) * 0.42)
    cx = (box[0] + box[2]) // 2
    crop = sprite.crop((cx - side // 2, box[1] - side // 12, cx + side - side // 2, box[1] - side // 12 + side))
    sheet = Image.new("RGBA", crop.size, (44, 36, 34, 255))
    sheet.alpha_composite(crop)
    sheet.convert("RGB").resize((512, 512), Image.LANCZOS).save(os.path.join(ROOT, "assets", "portraits", f"{hero}-gemini.png"))
