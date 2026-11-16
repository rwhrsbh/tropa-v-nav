"""Prints the Browser MCP command block that paints a foe's missing fight poses in Google Flow.

    python tools/flow_pose_cmds.py <id> [strike|hurt|guard|cast ...]

With no poses named, it paints every pose the foe lacks for its moves: `_hurt` always, `_strike`
for attacks, `_guard` for defending, `_cast` for buffs, curses, summons and specials. The foe is
described by the words of its rig prompts (`_inbox/flow_up/cmd_<id>.txt`) or, failing that, by its
line in docs/prompts/01_enemies.md. The resting picture on a green square,
`_inbox/flow_up/rig_<id>.png` (made if missing), is attached as the look only and each pose is
asked as a new picture; they land as `_inbox/flow_up/<id>_<pose>.png`, to be taken in with
`python tools/pose_in.py _inbox/flow_up/<id>_<pose>.png <id> <id>_<pose> _inbox/flow_up/rig_<id>.png`
(the green square is the scale source: it holds the resting picture at its own size). Paste the output into one
`browser` call (about a minute per pose).
"""
import os
import re
import sys

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
UP = "D:/grok-test/slaythespireclone/_inbox/flow_up"
ADD = "eval \"() => { const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Добавить в запрос'); if(b){b.click(); return 'added'} return 'direct' }\""

POSES = {
    # every pose keeps what the figure holds in the same hand as at rest: a pose that frees a busy
    # hand makes the model paint a third arm to hold the weapon, a weapon held "crosswise" comes
    # out with two shafts
    "strike": "{He} ATTACKS in mid-blow: a big step forward to the LEFT, body thrown forward, swinging whatever {he_l} holds in the SAME hand as in the attached picture far out to the left at full reach (with no weapon, claws or fists). Clearly a different pose from the attached picture.",
    "hurt": "{He} has just been struck hard and REELS BACK to the right, away from the blow, head thrown back, face twisted in pain, still gripping whatever {he_l} holds in the SAME hand as in the attached picture, a little dark blood on {his} shoulder.",
    "guard": "{He} pulls back into a DEFENSIVE crouch, body turned half away to the right, head ducked, holding whatever {he_l} holds in the SAME hands as in the attached picture upright close in front of {his} body to catch a blow (with no weapon, both forearms raised before the face).",
    "cast": "{He} CASTS a curse: standing tall, mouth open in a chant, still holding whatever {he_l} holds in the SAME hand as in the attached picture; only a hand that is empty in the attached picture is raised high with the fingers spread (if both hands are busy, both lift what they hold up high). No glow, no sparks, no magic effects at all, the pose alone.",
}
# four-legged beasts: "casting" or "reeling" with lifted paws gets them painted on two legs or with
# five; they keep all four paws on the ground
BEAST_POSES = {
    "strike": "{He} LUNGES to the LEFT to bite and claw: body stretched low and long, jaws wide open, one front paw reaching forward, hind legs pushing off. Clearly a different pose from the attached picture.",
    "hurt": "{He} FLINCHES away to the right from a blow: body crouched low and twisted, head turned back with a yelp, ears flat, all four paws still on the ground, a little dark blood on {his} shoulder.",
    "guard": "{He} crouches low and DEFENSIVE, body pressed down and turned half away, head lowered, teeth bared, fur bristling, all four paws on the ground.",
    "cast": "{He} HOWLS: standing on all four paws, head thrown up high with the muzzle pointing at the sky, mouth open, fur bristling along the back. No glow, no sparks, no magic effects.",
}
# by id: words in a description mislead ("a wolf-pelt hood", "toad-like" on a foe who stands upright)
BEASTS = {"biryuk", "nav_hound", "grey_wolf", "kot_bayun", "kot_bayun_ground", "shatun", "death_hare"}
NEG = ("Exactly as many arms, hands, legs and heads as in the attached picture; every held thing is one whole object.")
TAIL = (NEG + " " +
        "Whole figure inside the frame with a margin on every side, the feet near the bottom, the same size as in the attached picture. "
        "Flat solid pure green #00FF00 background, no ground texture, no shadow, no green on the figure.")


# foes painted before their rig prompts were written down and missing from 01_enemies.md
LOOKS = {
    "korochun": ("gaunt frost-rimed skeletal old man in a hooded fur cloak with a rune-carved ice scythe", "his"),
    "solovey": ("hunched ragged robber with wild grey hair, two fingers at his lips ready to whistle", "his"),
    "vodyanoy_tsar": ("bloated toad-faced water king with a crown, a weedy beard, a red mantle and a trident", "his"),
    "nav_hound": ("gaunt black hound of the dead with an iron collar", "its"),
    "miller": ("huge green-skinned drowned miller carrying a millstone on his back", "his"),
    "obderikha": ("hunched hairy bathhouse hag holding a birch broom and a bloody rag", "her"),
}


def who_of(foe):
    if foe in LOOKS:
        return LOOKS[foe]
    cmd =os.path.join(ROOT, "_inbox", "flow_up", f"cmd_{foe}.txt")
    if os.path.exists(cmd):
        m = re.search(r"Keep the SAME (.*?), (his|her|its) pose", open(cmd, encoding="utf-8").read())
        if m:
            return m.group(1), m.group(2)
    for line in open(os.path.join(ROOT, "docs", "prompts", "01_enemies.md"), encoding="utf-8"):
        if line.startswith(f"| `{foe}` |"):
            text = line.split("|")[3].strip().rstrip(".")
            its = "her" if re.search(r"\b(woman|witch|crone|maiden|she|her|girl|hag|goddess|bird-woman)\b", text, re.I) else "its"
            return text.split(":")[0].strip(), its
    raise SystemExit(f"no description for {foe}")


def missing(foe):
    t = "".join(open(os.path.join(ROOT, "assets", "data", "enemies", f), encoding="utf-8").read() for f in os.listdir(os.path.join(ROOT, "assets", "data", "enemies")))
    m = re.search(r'id:\s*"%s",\s*hp.*?\n    \),' % re.escape(foe), t, re.S)
    intents = set(re.findall(r"intent:\s*(\w+)", m.group(0))) if m else set()
    need = ["hurt"]
    if intents & {"Attack", "AttackAll"}:
        need.append("strike")
    # the guard pose shows whenever the foe gains block, whatever its move is called
    if "Defend" in intents or "Block(" in m.group(0):
        need.append("guard")
    if intents & {"Buff", "Debuff", "Special", "Summon", "Unknown"}:
        need.append("cast")
    return [p for p in need if not os.path.exists(os.path.join(ROOT, "assets", "enemies", f"{foe}_{p}-gemini.png"))]


def green_square(foe):
    out = os.path.join(ROOT, "_inbox", "flow_up", f"rig_{foe}.png")
    if not os.path.exists(out):
        im = Image.open(os.path.join(ROOT, "assets", "enemies", f"{foe}-gemini.png")).convert("RGBA")
        s = max(im.size)
        bg = Image.new("RGBA", (s, s), (0, 255, 0, 255))
        bg.alpha_composite(im, ((s - im.width) // 2, (s - im.height) // 2))
        bg.convert("RGB").save(out)


def jobs_of(foe, poses):
    """(foe, pose, prompt) for each pose to paint."""
    who, its = who_of(foe)
    he = {"his": "He", "her": "She", "its": "It"}[its]
    green_square(foe)
    out = []
    for pose in poses:
        table = BEAST_POSES if foe in BEASTS else POSES
        what = table[pose].format(He=he, he_l=he.lower(), his=its)
        prompt = (f"Use the attached picture only for what the {who} looks like (same body, clothes, colours, painted style, lighting and size). "
                  f"Paint {'him' if its == 'his' else 'her' if its == 'her' else 'it'} in a NEW pose, square 1:1, one single image, no text: {what} {TAIL}")
        out.append((foe, pose, prompt))
    return out


def ask(foe, prompt, upload):
    out = ["click getByRole('button', { name: 'Добавить ингредиенты в окно запроса' })", "wait 1200"]
    if upload:
        out += [f"upload getByRole('button', {{ name: 'Загрузить' }}).last() {UP}/rig_{foe}.png", "wait 9000"]
    out += [f'click "text=rig_{foe}.png >> visible=true >> nth=0"', "wait 1000", ADD, "wait 800"]
    # after an upload the ingredients panel's backdrop sometimes stays over the prompt box
    if upload:
        out += ["press Escape", "wait 500"]
    out += ["click getByRole('textbox', { name: 'Что вы хотите создать?' })", f'type "{prompt}"',
            "click getByRole('button', { name: 'Начать генерацию' })"]
    return out


def main():
    # --batch <id> <id> ...: every missing pose of these foes asked one after another without
    # waiting (Flow paints them side by side), then one wait and every tile saved. The newest tile
    # comes first, so the files are numbered back to front: `_inbox/flow_up/batch/<n>.png` with
    # the list in batch.txt; check them by eye before taking them in.
    if sys.argv[1] == "--batch":
        jobs = [j for foe in sys.argv[2:] for j in jobs_of(foe, missing(foe))]
        out, seen, tiles = [], set(), []
        for foe, pose, prompt in jobs:
            # an uploaded picture takes a tile of its own, before the poses asked from it
            if foe not in seen:
                tiles.append((foe, "upload"))
            out += ask(foe, prompt, foe not in seen) + ["wait 3000"]
            tiles.append((foe, pose))
            seen.add(foe)
        out += ["wait 70000"]
        n = len(tiles)
        os.makedirs(os.path.join(ROOT, "_inbox", "flow_up", "batch"), exist_ok=True)
        with open(os.path.join(ROOT, "_inbox", "flow_up", "batch", "batch.txt"), "w", encoding="utf-8") as f:
            for k, (foe, pose) in enumerate(tiles):
                f.write(f"{n - 1 - k} {foe} {pose}\n")
        # the tiles' order is the order they finished in, not the order they were asked: the
        # fetch first prints every tile's own title (`titles.txt`, for tools/pose_batch_in.py)
        titles = "() => [...document.querySelectorAll('[role=group]')].slice(0,%d).map(g=>g.getAttribute('aria-label')).join(' | ')" % n
        fetch = [f'eval "{titles}"']
        fetch += [f'download "[role=group] img >> nth={k}" --path {UP}/batch/{k}.png' for k in range(n)]
        # one browser call may run five minutes at most: the asking and the saving are also
        # written apart, to be sent as two calls a minute or so from each other
        bdir = os.path.join(ROOT, "_inbox", "flow_up", "batch")
        with open(os.path.join(bdir, "submit.txt"), "w", encoding="utf-8") as f:
            f.write("\n".join(out[:-1]) + "\n")
        with open(os.path.join(bdir, "fetch.txt"), "w", encoding="utf-8") as f:
            f.write("\n".join(fetch) + "\n")
        print("\n".join(out + fetch))
        return
    foe = sys.argv[1]
    poses = sys.argv[2:] or missing(foe)
    out = []
    for i, (foe, pose, prompt) in enumerate(jobs_of(foe, poses)):
        out += ask(foe, prompt, i == 0) + ["wait 60000", f'download "[role=group] img >> nth=0" --path {UP}/{foe}_{pose}.png']
    print("\n".join(out))


if __name__ == "__main__":
    main()
