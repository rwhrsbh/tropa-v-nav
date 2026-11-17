"""Takes in a batch of poses painted by `tools/flow_pose_cmds.py --batch`.

    python tools/pose_batch_in.py [<n> ...]

Reads `_inbox/flow_up/batch/titles.txt` (the tiles' own titles, which say the pose; the fetch
prints them) or else `batch.txt` (tile number, foe, pose) and takes every pose tile in with
tools/pose_in.py, skipping uploads and the tile numbers given (the ones that came out wrong, to be
asked again). Writes target/batch.png, every tile with its label, to check by eye first.
"""
import os
import subprocess
import sys

from PIL import Image, ImageDraw

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
B = os.path.join(ROOT, "_inbox", "flow_up", "batch")


# words in Flow's own title for a tile, and the pose they name
WORDS = (("reel", "hurt"), ("recoil", "hurt"), ("flinch", "hurt"), ("pain", "hurt"), ("struck", "hurt"),
         ("defens", "guard"), ("crouch", "guard"), ("guard", "guard"), ("brac", "guard"),
         ("cast", "cast"), ("curse", "cast"), ("hiss", "cast"), ("screech", "cast"), ("scream", "cast"), ("chant", "cast"), ("howl", "cast"),
         ("attack", "strike"), ("lung", "strike"), ("strik", "strike"), ("swing", "strike"), ("blow", "strike"))


def from_titles():
    """Rows read off `titles.txt` (the tiles' titles, newest first, as the fetch printed them):
    an upload `rig_<id>.png` names the foe of the poses above it, each title the pose."""
    text = open(os.path.join(B, "titles.txt"), encoding="utf-8").read()
    titles = [t.strip() for t in (text.split(" | ") if " | " in text else text.splitlines()) if t.strip()]
    rows, foe = [], None
    for k in range(len(titles) - 1, -1, -1):
        t = titles[k]
        if t.startswith("rig_") and t.endswith(".png"):
            foe = t[4:-4]
            rows.append((str(k), foe, "upload"))
            continue
        low = t.lower()
        pose = next((p for w, p in WORDS if w in low), None)
        rows.append((str(k), foe or "?", pose or "?"))
    return rows


def main():
    skip = set(sys.argv[1:])
    if os.path.exists(os.path.join(B, "titles.txt")):
        rows = [list(r) for r in from_titles()]
    else:
        rows = [l.split() for l in open(os.path.join(B, "batch.txt"), encoding="utf-8") if l.strip()]
    rows.sort(key=lambda r: int(r[0]))
    sheet = Image.new("RGB", (len(rows) * 240, 270), (40, 40, 40))
    d = ImageDraw.Draw(sheet)
    for i, (k, foe, pose) in enumerate(rows):
        f = os.path.join(B, f"{k}.png")
        if os.path.exists(f):
            sheet.paste(Image.open(f).convert("RGB").resize((240, 240)), (i * 240, 30))
        d.text((i * 240 + 4, 6), f"{k} {foe} {pose}", fill=(255, 255, 0))
    sheet.save(os.path.join(ROOT, "target", "batch.png"))
    if "--look" in skip:
        return
    for k, foe, pose in rows:
        if pose in ("upload", "?") or foe == "?" or k in skip:
            continue
        subprocess.run([sys.executable, "-X", "utf8", os.path.join(ROOT, "tools", "pose_in.py"), os.path.join(B, f"{k}.png"), foe, f"{foe}_{pose}",
                        os.path.join(ROOT, "_inbox", "flow_up", f"rig_{foe}.png")], check=True)


if __name__ == "__main__":
    main()
