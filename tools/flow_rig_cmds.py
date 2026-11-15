"""Prints the Browser MCP command block that paints one foe's rig edits in Google Flow.

    python tools/flow_rig_cmds.py <id> "<who>" "<head>" [<pronoun>]

<who> describes the foe ("gaunt black hound of the dead"), <head> what the cyan mask and the
beheading take ("its whole head with both ears, down to the iron collar"), <pronoun> is its/his/
her. The block uploads `_inbox/flow_up/rig_<id>.png` (the resting picture on a green square),
paints the colour mask, the headless body, the corpse and the headless corpse, and saves them as
`_inbox/flow_up/<id>_{mask,body,dead,deadhl}.png` for `tools/rig_in.py`. Paste the output into
one `browser` call (it runs about four minutes).
"""
import sys

ADD = "eval \"() => { const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Добавить в запрос'); if(b){b.click(); return 'added'} return 'direct' }\""
UP = "D:/grok-test/slaythespireclone/_inbox/flow_up"


def attach(foe, first):
    lines = ["click getByRole('button', { name: 'Добавить ингредиенты в окно запроса' })", "wait 1200"]
    if first:
        lines += [f"upload getByRole('button', {{ name: 'Загрузить' }}).last() {UP}/rig_{foe}.png", "wait 9000"]
    lines += [f"click getByText('rig_{foe}.png').last()", "wait 1000", ADD, "wait 800"]
    return lines


def ask(prompt, out):
    return [
        "click getByRole('textbox', { name: 'Что вы хотите создать?' })",
        f'type "{prompt}"',
        "click getByRole('button', { name: 'Начать генерацию' })",
        "wait 45000",
        f'download "[role=group] img >> nth=0" --path {UP}/{out}.png',
    ]


def main():
    foe, who, head = sys.argv[1:4]
    its = sys.argv[4] if len(sys.argv) > 4 else "its"
    mask = (f"Edit the attached picture: square 1:1, one single image, no text, no labels. Keep the SAME {who}, {its} pose, framing, size, "
            f"position, lighting and plain green background pixel for pixel. Only paint {head} as ONE flat solid pure cyan #00FFFF "
            "silhouette with no shading, covering every hair and tip. Everything else unchanged.")
    body = (f"DECAPITATE the attached figure. Keep the picture exactly the same: same {who}, same body, pose, framing, size, colours and "
            f"plain green background, square 1:1, one single image, no text. Only {head} is gone, cut off and absent; at the neck, a raw "
            "bloody neck stump in the same painted style, plain green where the head was. Nothing else changes.")
    dead = (f"Use the attached picture for what the {who} looks like (same clothes, colours, painted style and lighting). New picture, "
            f"square 1:1, one single image, no text: lying dead flat on the ground on {its} back or side, seen from the side, head to the "
            "LEFT, limbs sprawled loosely, the whole body lying along the bottom third of the picture, nothing standing up. Everything "
            "inside the frame with a margin on every side. Flat solid pure green #00FF00 background, no ground, no shadow, no blood pool.")
    headless = (f"DECAPITATE the attached lying body. Keep the picture exactly the same: same pose, same place, size, colours and plain green "
                f"background, square 1:1, one single image, no text. Only {head} is gone, cut off and absent; at the neck, a raw bloody neck "
                "stump with a little dark blood under it. Plain green where the head was. Nothing else changes.")
    out = attach(foe, True) + ask(mask, f"{foe}_mask") + attach(foe, False) + ask(body, f"{foe}_body") + attach(foe, False) + ask(dead, f"{foe}_dead")
    out += ['click "[role=group] img >> nth=0" --button right', "wait 1000", 'click "text=Добавить в запрос"', "wait 1000"]
    out += ask(headless, f"{foe}_deadhl")
    print("\n".join(out))


if __name__ == "__main__":
    main()
