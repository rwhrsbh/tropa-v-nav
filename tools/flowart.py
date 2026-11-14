"""Turns pictures downloaded from Google Flow into card illustrations or relic icons.

    python tools/flowart.py <card id> [...]            # 1000x750 -> assets/cards/<id>-gemini.png
    python tools/flowart.py --relic <relic id> [...]   # 256x256   -> assets/relics/<id>-gemini.png
    python tools/flowart.py --story <page id> [...]    # 1920x1080 -> assets/story/<id>-gemini.png
    python tools/flowart.py --list                     # what is waiting, oldest first
    python tools/flowart.py --match [--relic] <id> [...] # pair waiting files with these ids by their titles
    python tools/flowart.py --packs                    # take in every tropa-pack-*.bin and tropa-<kind>-<id>.jpg

A pack is what the page helper `flowPack` saves: several full-size pictures in one file, each
with its kind and id, because the browser lets a page save only one file at a time.

Flow names a download `<Some_title>_<yyyymmddhhmmss>.jpg`; only such files, newer than a day,
are taken, in the order they arrived. `-` throws a picture away. Originals move to `_inbox/`.
"""
import glob
import os
import re
import shutil
import sys
import time

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
# where the browser saves: TROPA_DOWNLOADS if set, else the user's Downloads folder
DOWNLOADS = os.environ.get("TROPA_DOWNLOADS") or os.path.join(os.path.expanduser("~"), "Downloads")
# Flow ends a name with a time stamp, or (since 2026-11) with eight hex digits
STAMP = re.compile(r"_(\d{14}|[0-9a-f]{8})( \(\d+\))?\.(jpe?g|png)$", re.I)


def waiting():
    fresh = time.time() - 24 * 3600
    files = [f for f in glob.glob(os.path.join(DOWNLOADS, "*")) if STAMP.search(f) and os.path.getmtime(f) > fresh]
    return sorted(files, key=os.path.getmtime)


def place(im, name, kind):
    """Crops and scales a picture for `kind` ("card", "relic" or "story") and saves it."""
    w, h = im.size
    if kind == "relic":
        side = min(w, h)
        im = im.crop(((w - side) // 2, (h - side) // 2, (w - side) // 2 + side, (h - side) // 2 + side)).resize((256, 256), Image.LANCZOS)
        folder = "relics"
    elif kind == "story":
        # a page of the tale is 16:9, and what matters sits in its upper two thirds: crop
        # more from the foot than from the head
        nh = min(w * 9 // 16, h)
        top = (h - nh) // 3
        im = im.crop((0, top, w, top + nh)).resize((1920, 1080), Image.LANCZOS)
        folder = "story"
    else:
        if w * 3 > h * 4:
            nw = h * 4 // 3
            im = im.crop(((w - nw) // 2, 0, (w - nw) // 2 + nw, h))
        else:
            nh = w * 3 // 4
            im = im.crop((0, (h - nh) // 2, w, (h - nh) // 2 + nh))
        im = im.resize((1000, 750), Image.LANCZOS)
        folder = "cards"
    os.makedirs(os.path.join(ROOT, "assets", folder), exist_ok=True)
    im.save(os.path.join(ROOT, "assets", folder, f"{name}-gemini.png"))
    print(kind, name, (w, h))


def packs():
    import io
    import json
    import struct

    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    for f in sorted(glob.glob(os.path.join(DOWNLOADS, "tropa-pack-*.bin")), key=os.path.getmtime):
        data = open(f, "rb").read()
        n = struct.unpack(">I", data[:4])[0]
        head = json.loads(data[4:4 + n].decode("utf-8"))
        at = 4 + n
        for kind, name, size in head:
            raw = data[at:at + size]
            at += size
            open(os.path.join(inbox, f"{name}_f.jpg"), "wb").write(raw)
            place(Image.open(io.BytesIO(raw)).convert("RGB"), name, kind)
        os.remove(f)
    for f in sorted(glob.glob(os.path.join(DOWNLOADS, "tropa-card-*.jpg")) + glob.glob(os.path.join(DOWNLOADS, "tropa-relic-*.jpg"))):
        kind, name = os.path.basename(f)[len("tropa-"):-4].split("-", 1)
        place(Image.open(f).convert("RGB"), name, kind)
        shutil.move(f, os.path.join(inbox, f"{name}_f.jpg"))


# words too common in titles and prompts to say which picture is which
STOP = {"the", "and", "with", "slavic", "peasant", "old", "dark", "background", "over", "from", "into", "near", "woman", "man",
        "hand", "hands", "holding", "game", "icon", "painting", "digital", "oil", "item", "her", "his", "through", "for"}


def words(text):
    return {w[:5] for w in re.findall(r"[a-z]+", text.lower()) if len(w) > 2 and w not in STOP}


def match(ids, relic):
    """Pairs each waiting file with one of `ids` by the words its title shares with the prompt."""
    import json

    files = waiting()
    prompts = {}
    for f in glob.glob(os.path.join(ROOT, "docs", "prompts", "plots", "*.json")):
        for k, v in json.load(open(f, encoding="utf-8")).items():
            if isinstance(v, str):
                prompts[k.split(":", 1)[-1]] = v.split("Game item icon: ", 1)[-1].split(". A single object", 1)[0]
    left = list(ids)
    pairs = []
    for f in files:
        title = words(os.path.basename(f))
        best = max(left, key=lambda i: len(title & (words(i) | words(prompts.get(i, "")))), default=None)
        score = len(title & (words(best) | words(prompts.get(best, "")))) if best else 0
        if best is None or score == 0:
            pairs.append((f, None))
            continue
        left.remove(best)
        pairs.append((f, best))
    for f, i in pairs:
        print(f"{os.path.basename(f)[:44]:<44} -> {i}")
    if left:
        print("no picture for:", " ".join(left))
    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    for f, i in pairs:
        if i is None:
            continue
        place(Image.open(f).convert("RGB"), i, "relic" if relic else "card")
        shutil.move(f, os.path.join(inbox, f"{'relic_' if relic else ''}{i}_f.jpg"))


def main():
    args = sys.argv[1:]
    if args[:1] == ["--match"]:
        relic = args[1:2] == ["--relic"]
        match(args[2:] if relic else args[1:], relic)
        return
    if args[:1] == ["--packs"]:
        packs()
        return
    files = waiting()
    if args[:1] == ["--list"]:
        for f in files:
            print(os.path.basename(f))
        return
    kind = {"--relic": "relic", "--story": "story"}.get(args[0] if args else "", "card")
    names = args[1:] if kind != "card" else args
    if len(files) != len(names):
        raise SystemExit(f"{len(files)} downloads waiting but {len(names)} names given; nothing moved")
    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    for f, name in zip(files, names):
        if name == "-":
            os.remove(f)
            continue
        place(Image.open(f).convert("RGB"), name, kind)
        k = 1
        while os.path.exists(os.path.join(inbox, f"{name}_f{k}.jpg")):
            k += 1
        shutil.move(f, os.path.join(inbox, f"{name}_f{k}.jpg"))


if __name__ == "__main__":
    main()
