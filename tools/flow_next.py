"""Prints one message for the Google Flow agent asking for the next N unpainted cards.

    python tools/flow_next.py [N=4] [--relics]

Cards come from docs/prompts/plots/*.json in file order, skipping those that already have
assets/cards/<id>-gemini.png. The ids, in the order asked, go to stderr: the pictures come
back in that order, and that is the order to give tools/flowart.py.
"""
import glob
import json
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
LOOK = {
    "B": "a huge grey-bearded warrior in hacked mail and an iron face mask, with a tall black oak shield carved with a stern face and a six-flanged mace, ",
    "V": "a gaunt woman in a damp linen shift, braids threaded with small bird bones, milky white eyes, rag dolls hanging on her belt, soot-stained fingers, ",
    "O": "a tall old man in animal hides and a birch-bark mask, a long beard with woven charms, dark red rune marks painted on his arms, a staff of lightning-split oak, ",
    "W": "a wiry hunter with a wolf pelt on his shoulders and a silver chain around his neck ",
    "Z": "a stooped middle-aged woman in a dark headscarf with old burn scars on her face, bundles of dried herbs on her back, a sickle and two clay jugs at her belt, ",
    "E": "Yemelya, a plump clean-shaven grown peasant man with a round face and a blond bowl haircut, in a patched off-white linen shirt with a red collar and a red sash, grey trousers and bast shoes, ",
    "S": "a thin Slavic skomorokh jester in a patchwork coat sewn with small bells, bare-headed with a white grinning wooden mask on his face and three wooden masks on his belt, ",
}
STYLE = (
    "Every picture: one clear action, dramatic composition, dark background fading to the edges. No magic glow, no sparks or particles. "
    "Dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, "
    "limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk horror in the manner of "
    "19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, realistic anatomy. No text, no letters, no watermark, no signature, no frame, no modern objects."
)
RELIC_STYLE = (
    "Every picture: a single object centred and filling most of the frame, on a plain very dark brown background with a soft vignette, lit from the upper left. "
    "Old, worn, handmade. Dark painterly digital oil painting, thick visible brushstrokes, limited palette of soot black, bone white, rust red, swamp green, "
    "candle amber, cold blue. Slavic folk horror. No text, no letters, no frame, no border, no hands, no people."
)


def cards():
    out = []
    order = ["volhv3", "volkolak3", "znaharka3", "skomoroh3", "neutral3", "bogatyr4", "vorozheya4", "volhv4", "volkolak4", "znaharka4", "skomoroh4", "emelya4", "emelya5"]
    for name in order:
        j = json.load(open(os.path.join(ROOT, "docs", "prompts", "plots", name + ".json"), encoding="utf-8"))
        look = LOOK.get(j.get("_look", ""), "")
        for k, v in j.items():
            if k.startswith("_"):
                continue
            if os.path.exists(os.path.join(ROOT, "assets", "cards", k + "-gemini.png")):
                continue
            out.append((k, v[1:] if v.startswith("@") else look + v))
    return out


def relics():
    j = json.load(open(os.path.join(ROOT, "docs", "prompts", "plots", "relics.json"), encoding="utf-8"))
    out = []
    for k, v in j.items():
        if k.startswith("_"):
            continue
        rid = k.split(":", 1)[1]
        if os.path.exists(os.path.join(ROOT, "assets", "relics", rid + "-gemini.png")):
            continue
        subject = v.split("Game item icon: ", 1)[1].split(". A single object", 1)[0]
        out.append((rid, subject + "."))
    return out


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    n = int(args[0]) if args else 4
    skip = int(args[1]) if len(args) > 1 else 0
    want_relics = "--relics" in sys.argv
    todo = (relics() if want_relics else cards())[skip:skip + n]
    if not todo:
        print("NOTHING LEFT", file=sys.stderr)
        return
    kind = "square 1:1 game item icons" if want_relics else "landscape 4:3 card illustrations"
    lines = [f"Create {len(todo)} separate images, one image per numbered scene, in this order, each as its own {kind[:-1]}. Do not ask, just generate them one after another."]
    for i, (_, text) in enumerate(todo, 1):
        lines.append(f"{i}) {text}")
    lines.append(RELIC_STYLE if want_relics else STYLE)
    print(" ".join(lines))
    print(" ".join(k for k, _ in todo), file=sys.stderr)


main()
