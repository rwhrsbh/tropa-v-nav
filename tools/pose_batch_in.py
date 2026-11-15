"""Takes in a batch of poses painted by `tools/flow_pose_cmds.py --batch`.

    python tools/pose_batch_in.py [<n> ...]

Reads `_inbox/flow_up/batch/batch.txt` (tile number, foe, pose) and takes every pose tile in with
tools/pose_in.py, skipping uploads and the tile numbers given (the ones that came out wrong, to be
asked again). Writes target/batch.png, every tile with its label, to check by eye first.
"""
import os
import subprocess
import sys

from PIL import Image, ImageDraw

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
B = os.path.join(ROOT, "_inbox", "flow_up", "batch")


def main():
    skip = set(sys.argv[1:])
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
        if pose == "upload" or k in skip:
            continue
        subprocess.run([sys.executable, "-X", "utf8", os.path.join(ROOT, "tools", "pose_in.py"), os.path.join(B, f"{k}.png"), foe, f"{foe}_{pose}",
                        os.path.join(ROOT, "_inbox", "flow_up", f"rig_{foe}.png")], check=True)


if __name__ == "__main__":
    main()
