"""Takes a foe's four Flow edits into the game: its head and headless body for the rig, and its
two corpses.

    python tools/rig_in.py <id> <mask> <body> <dead> <dead_headless>

<mask> is the resting picture with the whole head painted flat cyan, <body> the same picture with
the head cut off and a neck stump, <dead> the foe lying dead, <dead_headless> that corpse with its
head gone. The first two go through tools/rigparts.py (head and body, laid over the resting
picture, which is never changed); it prints the line for `rig::FIGURES`. The corpses are keyed by
flooding the background from the edges (tools/key_flood.py) and laid on a wide sheet as the older
foes' are: 3:2, the body as long as 0.88 of a standing foe's sheet is tall, on the same foot line;
the headless one at the corpse's own scale. A preview sheet lands in target/rig_<id>.png.
"""
import os
import subprocess
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from key_flood import key_flood  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ENEMIES = os.path.join(ROOT, "assets", "enemies")
LONG = 0.88
FOOT = 0.985


def figure(path):
    im = key_flood(Image.open(path), tol=90)
    return im.crop(im.getchannel("A").point(lambda v: 255 if v > 24 else 0).getbbox())


def main():
    foe, mask, body, dead, headless = sys.argv[1:6]
    env = dict(os.environ, PARTS="head")
    preview = os.path.join(ROOT, "target", f"rig_{foe}.png")
    subprocess.run([sys.executable, "-X", "utf8", os.path.join(ROOT, "tools", "rigparts.py"), os.path.join(ENEMIES, f"{foe}-gemini.png"), body, mask, ENEMIES, foe, preview], env=env, check=True)
    h = Image.open(os.path.join(ENEMIES, f"{foe}-gemini.png")).size[1]
    w = h * 3 // 2
    d = figure(dead)
    k = LONG * h / max(d.width, d.height * 1.4)
    out = []
    for name, fig in ((f"{foe}_dead", d), (f"{foe}_dead_headless", figure(headless))):
        fig = fig.resize((max(1, round(fig.width * k)), max(1, round(fig.height * k))), Image.LANCZOS)
        sheet = Image.new("RGBA", (w, h))
        sheet.alpha_composite(fig, ((w - fig.width) // 2, int(h * FOOT) - fig.height))
        sheet.save(os.path.join(ENEMIES, f"{name}-gemini.png"))
        out.append(sheet)
    # the corpses under the rig preview, on grey, to look at
    rp = Image.open(preview).convert("RGBA")
    sheet = Image.new("RGBA", (max(rp.width, w), rp.height + h // 2), (90, 90, 90, 255))
    sheet.alpha_composite(rp, (0, 0))
    for i, s in enumerate(out):
        s = s.resize((w // 2, h // 2))
        sheet.alpha_composite(s, (i * (w // 2), rp.height))
    sheet.convert("RGB").save(preview)
    print("corpses", w, h, "scale %.3f" % k)


if __name__ == "__main__":
    main()
