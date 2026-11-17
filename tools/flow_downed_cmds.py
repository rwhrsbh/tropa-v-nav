"""Prints the Browser MCP commands that paint a maimed hero's picture on the Edge (`_downed`) in Flow.

    python tools/flow_downed_cmds.py <hero> [<mark> ...]

A hero down on the Edge kneels in `<hero>_downed`; one who has lost the head or an arm kneels in
`<hero><mark>_downed` (`_noweapon`, `_noshield`, `_noarms`, `_headless`, `_headless_noweapon`, ...,
see `short_of` in crates/game/src/battle.rs). Each is an edit of the hero's whole downed picture:
the commands upload it on a green square (`_inbox/flow_up/down_<hero>.png`, written here), then ask
for one edit per missing mark without waiting, and print nothing else. With no marks, every mark the
hero has a standing picture for and no downed one yet. Flow orders the tiles as they finish, so
fetch them by eye: `python tools/downed_in.py <tile> <hero><mark>` takes one in.

WEAPON / SHIELD say which arm is which in each hero's downed picture: the weapon arm is the one
`_noweapon` loses.
"""
import os
import sys

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
HEROES = os.path.join(ROOT, "assets", "heroes")
UP = "D:/grok-test/slaythespireclone/_inbox/flow_up"
MARKS = ["_noweapon", "_noshield", "_noarms", "_headless", "_headless_noweapon", "_headless_noshield", "_headless_noarms"]

WHO = {
    "bogatyr": ("kneeling bearded warrior in mail with a round shield", "his", "the arm holding the mace", "the arm holding the round shield"),
    "emelya": ("young man lying on top of a white stove on sledge runners", "his", "his right arm", "his left arm"),
    "skomoroh": ("kneeling masked jester in patched motley", "his", "the arm holding the tambourine", "his other arm, the one pressed to his body"),
    "volhv": ("kneeling old long-bearded sorcerer in furs with a tall staff", "his", "the arm holding the staff", "his other arm, the one without the staff"),
    "volkolak": ("kneeling man in a wolf-skin hood", "his", "the arm holding the knife", "his other arm, the one pressed to his body"),
    "volkolak_beast": ("crouching grey wolf-man", "his", "the arm farther from the viewer", "the arm nearer the viewer"),
    "vorozheya": ("kneeling pale witch with long black hair in a grey shift", "her", "her left arm", "her right arm"),
    "znaharka": ("kneeling old herb woman in a headscarf with a bundle of herbs on her back", "her", "the arm holding the sickle", "her other arm, the one pressed to her body"),
}


def what(hero, mark):
    who, pr, weapon, shield = WHO[hero]
    lost = []
    if "headless" in mark:
        lost.append(f"{pr} whole head with all the hair" + (" and the beard" if pr == "his" else "") + ", cut off at the neck, leaving a raw bloody neck stump")
    if "noarms" in mark:
        lost.append("BOTH arms, cut off at the shoulders, leaving two raw bloody shoulder stumps, and everything they held gone too")
    elif "noweapon" in mark:
        lost.append(f"{weapon} with what it holds, cut off at the shoulder, leaving a raw bloody shoulder stump")
    elif "noshield" in mark:
        lost.append(f"{shield} with what it holds, cut off at the shoulder, leaving a raw bloody shoulder stump")
    return who, pr, "; and also ".join(lost)


def prompt(hero, mark):
    who, pr, lost = what(hero, mark)
    return (f"Edit the attached picture: keep the SAME {who}, {pr} pose, framing, size, position, colours, painted style and plain "
            f"green background pixel for pixel, square 1:1, one single image, no text. Only this changes: {lost}. Plain green where "
            "the missing part was, nothing new added, no other limb moved, exactly as many remaining arms and legs as before.")


def upload(hero):
    pic = Image.open(os.path.join(HEROES, f"{hero}_downed-gemini.png")).convert("RGBA")
    side = max(pic.size)
    sq = Image.new("RGBA", (side, side), (0, 255, 0, 255))
    sq.alpha_composite(pic, ((side - pic.width) // 2, (side - pic.height) // 2))
    path = os.path.join(ROOT, "_inbox", "flow_up", f"down_{hero}.png")
    sq.convert("RGB").save(path)


def missing(hero):
    have = lambda s: os.path.exists(os.path.join(HEROES, f"{s}-gemini.png"))
    return [m for m in MARKS if have(f"{hero}{m}") and not have(f"{hero}{m}_downed")]


def main():
    hero = sys.argv[1]
    marks = sys.argv[2:] or missing(hero)
    upload(hero)
    out = ["click getByRole('button', { name: 'Добавить ингредиенты в окно запроса' })", "wait 1200",
           f"upload getByRole('button', {{ name: 'Загрузить' }}).last() {UP}/down_{hero}.png", "wait 9000"]
    for i, m in enumerate(marks):
        # the ingredients panel is still open after the upload
        out += ([] if i == 0 else ["click getByRole('button', { name: 'Добавить ингредиенты в окно запроса' })", "wait 1200"]) + [
                f'click "text=down_{hero}.png >> visible=true >> nth=0"', "wait 1000",
                "eval \"() => { const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Добавить в запрос'); if(b){b.click(); return 'added'} return 'direct' }\"",
                "wait 800", "press Escape", "wait 500",
                "click getByRole('textbox', { name: 'Что вы хотите создать?' })", f'type "{prompt(hero, m)}"',
                "click getByRole('button', { name: 'Начать генерацию' })", "wait 3000"]
    print("\n".join(out))
    print(" ".join(marks), file=sys.stderr)


if __name__ == "__main__":
    main()
