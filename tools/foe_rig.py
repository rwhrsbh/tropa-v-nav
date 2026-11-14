"""Makes a foe's game sprites from one picture painted on green in Google Flow.

    python tools/foe_rig.py <picture> <id> <size> <neck>

<size> is the sheet's side or WxH (512x768 for a common foe, 1024 for an elite or a boss);
<neck> is how far down the figure, 0..1, the head comes off; a beast on all fours facing left takes
<x>,<y> instead: the head is what lies left of x and above y, both as shares of the figure's box;
<l>,<r>,<t>,<b> takes just that box, for a head with wings or arms around it. Writes into assets/enemies/:
  <id>-gemini.png                 the whole figure, keyed and planted on the baseline
  <id>_head / <id>_body           cut at the neck with a soft seam, for the rig's nod and the killing blow
  <id>_dead / <id>_dead_headless  the figure laid on the ground
and prints the rig.rs line with the head joint.
"""
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cutout import fit, key_green  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "enemies")
FOOT = 0.957


def lying(fig, w, h):
    rot = fig.rotate(-82, expand=True, resample=Image.BICUBIC)
    sheet = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    k = min(w * 0.88 / rot.width, h * 0.45 / rot.height, 1.0)
    rz = rot.resize((max(1, int(rot.width * k)), max(1, int(rot.height * k))), Image.LANCZOS)
    sheet.alpha_composite(rz, ((w - rz.width) // 2, int(h * FOOT) - rz.height))
    return sheet


def main():
    src, name, size, neck = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
    w, h = (int(v) for v in size.split("x")) if "x" in size else (int(size), int(size))
    img = fit(key_green(Image.open(src)), w, h, FOOT)
    img.save(os.path.join(ROOT, f"{name}-gemini.png"), optimize=True)
    a = np.asarray(img.getchannel("A"))
    ys, xs = np.nonzero(a > 30)
    x0, y0, x1, y1 = xs.min(), ys.min(), xs.max(), ys.max()
    feather = 6
    def ramp(n, at, up):
        r = np.clip((np.arange(n) - (at - feather)) / (2.0 * feather), 0, 1)
        return r if up else 1 - r
    if neck.count(",") == 3:
        l, r, tp, b = (float(v) for v in neck.split(","))
        L, R, T, B = (int(x0 + (x1 - x0) * l), int(x0 + (x1 - x0) * r), int(y0 + (y1 - y0) * tp), int(y0 + (y1 - y0) * b))
        inside = (ramp(h, T, True) * ramp(h, B, False))[:, None] * (ramp(w, L, True) * ramp(w, R, False))[None, :]
        t = 1 - inside
        neck_y = B
        joint = ((L + R) // 2, B)
    elif "," in neck:
        fx, fy = (float(v) for v in neck.split(","))
        cut_x, neck_y = int(x0 + (x1 - x0) * fx), int(y0 + (y1 - y0) * fy)
        ty = np.clip((np.arange(h) - (neck_y - feather)) / (2.0 * feather), 0, 1)[:, None]
        tx = np.clip((np.arange(w) - (cut_x - feather)) / (2.0 * feather), 0, 1)[None, :]
        t = 1 - (1 - ty) * (1 - tx)
        joint = (cut_x, (y0 + neck_y) // 2)
    else:
        neck_y = int(y0 + (y1 - y0) * float(neck))
        t = np.clip((np.arange(h) - (neck_y - feather)) / (2.0 * feather), 0, 1)[:, None]
        row = np.nonzero(a[neck_y] > 30)[0]
        joint = ((row.min() + row.max()) / 2 if len(row) else (x0 + x1) / 2, neck_y)
    arr = np.asarray(img, np.float32)
    head, body = arr.copy(), arr.copy()
    head[..., 3] *= 1 - t
    body[..., 3] *= t
    head = Image.fromarray(head.astype(np.uint8), "RGBA")
    body = Image.fromarray(body.astype(np.uint8), "RGBA")
    head.save(os.path.join(ROOT, f"{name}_head-gemini.png"), optimize=True)
    body.save(os.path.join(ROOT, f"{name}_body-gemini.png"), optimize=True)
    box = (x0, y0, x1 + 1, y1 + 1)
    lying(img.crop(box), w, h).save(os.path.join(ROOT, f"{name}_dead-gemini.png"), optimize=True)
    lying(body.crop(box), w, h).save(os.path.join(ROOT, f"{name}_dead_headless-gemini.png"), optimize=True)
    jx, jy = joint
    fill = round((y1 - y0) / h, 3)
    print(f'    ("enemies/{name}", &[(Part::Head, Vec2::new({jx / w:.3f}, {jy / h:.3f}), 1.0)]),  // fills {fill}')


main()
