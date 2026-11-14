"""Prints the Google Flow agent messages for the new wave of foes (docs/prompts/plots/wave.json).

    python tools/wave_prompts.py            # every batch, one message per line, ids to stderr
    python tools/wave_prompts.py --json     # [{"ids": [...], "text": "..."}] for pasting into the page

Batches never mix kinds, so each message carries one style tail: foes on green, icons on black,
cards as 4:3 scenes. Things that already have their picture are left out.
"""
import json
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
FOE_TAIL = (
    "Every picture: a single full-body creature, three-quarter view facing left, feet on the ground near the bottom edge (a flying one hovers low), "
    "the whole figure inside the frame with a small margin, nothing cropped. Flat solid pure green #00FF00 background, no ground shadow, no background elements, "
    "no green on the character. No particles, sparks, embers, snow, smoke, magic aura or glow effects around the character. "
    "Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of "
    "soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk tale, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). "
    "Grim, grounded, readable silhouette. No text, no letters, no watermark, no frame, no modern objects."
)
ICON_HEAD = (
    "Each is one carved and painted emblem of old wood, iron, bone and cloth, lit with a faint inner glow, centred on a flat pure black background, "
    "with a bold silhouette that stays readable when tiny. No text, no letters, no frame, no border, no background scenery."
)
CARD_TAIL = (
    "Every picture: one clear subject, dramatic composition, dark background fading to the edges. No magic glow, no sparks or particles. "
    "Dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, "
    "limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk horror in the manner of "
    "19th-century Russian painting (Vasnetsov, Vrubel). No text, no letters, no watermark, no frame, no modern objects."
)
DIR = {"N": "enemies", "E": "enemies", "B": "enemies", "S": "status", "C": "cards"}


def batches():
    j = json.load(open(os.path.join(ROOT, "docs", "prompts", "plots", "wave.json"), encoding="utf-8"))
    groups = {"N": [], "Q": [], "S": [], "C": []}
    for k, (kind, text) in ((k, v) for k, v in j.items() if not k.startswith("_")):
        if os.path.exists(os.path.join(ROOT, "assets", DIR[kind], k + "-gemini.png")):
            continue
        groups["Q" if kind in "EB" else kind].append((k, text.lstrip("@")))
    out = []
    for g, items in groups.items():
        for i in range(0, len(items), 6):
            part = items[i:i + 6]
            n = len(part)
            if g == "N":
                head = f"Create {n} separate images, one per numbered creature, in this order, each its own portrait 2:3 picture. Do not ask, just generate them one after another."
                tail = FOE_TAIL
            elif g == "Q":
                head = f"Create {n} separate images, one per numbered creature, in this order, each its own square 1:1 picture. Do not ask, just generate them one after another."
                tail = FOE_TAIL
            elif g == "S":
                head = f"Paint {n} separate square 1:1 emblem icons for a dark Slavic folk-horror card game, one image per numbered emblem, in this order. Do not ask, just generate them. " + ICON_HEAD
                tail = ""
            else:
                head = f"Create {n} separate images, one per numbered scene, in this order, each its own landscape 4:3 card illustration. Do not ask, just generate them."
                tail = CARD_TAIL
            body = " ".join(f"{x}) {t}" for x, (_, t) in enumerate(part, 1))
            out.append({"ids": [k for k, _ in part], "text": " ".join(s for s in (head, body, tail) if s)})
    return out


def main():
    b = batches()
    if "--json" in sys.argv:
        print(json.dumps(b, ensure_ascii=False))
        return
    for x in b:
        print(x["text"])
        print(" ".join(x["ids"]), file=sys.stderr)


main()
