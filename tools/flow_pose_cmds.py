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
    "strike": "{He} ATTACKS: lunging forward to the LEFT, striking hard with {his} weapon, claws or fists, body leaning into the blow, face fierce.",
    "hurt": "{He} has just been struck hard and REELS BACK to the right, away from the blow, head thrown back, face twisted in pain, arms flung out, a little dark blood on {his} shoulder.",
    "guard": "{He} braces in a tight DEFENSIVE stance, body turned half away and crouched, weapon, arms or wings raised across the front to ward off a blow.",
    "cast": "{He} CASTS a curse: standing tall, one arm or claw raised high and the other thrust forward to the LEFT, fingers spread, mouth open in a chant. No glow, no sparks, no magic effects at all, the pose alone.",
}
TAIL = ("Whole figure inside the frame with a margin on every side, the feet near the bottom, the same size as in the attached picture. "
        "Flat solid pure green #00FF00 background, no ground texture, no shadow, no green on the figure.")


def who_of(foe):
    cmd = os.path.join(ROOT, "_inbox", "flow_up", f"cmd_{foe}.txt")
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
    if "Defend" in intents:
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


def main():
    foe = sys.argv[1]
    poses = sys.argv[2:] or missing(foe)
    who, its = who_of(foe)
    he = {"his": "He", "her": "She", "its": "It"}[its]
    green_square(foe)
    out = ["click getByRole('button', { name: 'Добавить ингредиенты в окно запроса' })", "wait 1200",
           f"upload getByRole('button', {{ name: 'Загрузить' }}).last() {UP}/rig_{foe}.png", "wait 9000"]
    for i, pose in enumerate(poses):
        if i:
            out += ["click getByRole('button', { name: 'Добавить ингредиенты в окно запроса' })", "wait 1200"]
        out += [f"click getByText('rig_{foe}.png').last()", "wait 1000", ADD, "wait 800"]
        what = POSES[pose].format(He=he, his=its)
        prompt = (f"Use the attached picture only for what the {who} looks like (same body, clothes, colours, painted style, lighting and size). "
                  f"Paint {'him' if its == 'his' else 'her' if its == 'her' else 'it'} in a NEW pose, square 1:1, one single image, no text: {what} {TAIL}")
        out += ["click getByRole('textbox', { name: 'Что вы хотите создать?' })", f'type "{prompt}"',
                "click getByRole('button', { name: 'Начать генерацию' })", "wait 45000",
                f'download "[role=group] img >> nth=0" --path {UP}/{foe}_{pose}.png']
    print("\n".join(out))


if __name__ == "__main__":
    main()
