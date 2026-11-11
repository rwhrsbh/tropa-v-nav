import os, sys
import numpy as np
from PIL import Image, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "enemies")

sys.path.append(os.path.dirname(os.path.abspath(__file__)))
from cutout import key_green, fit

targets = [
    ("kot_bayun", 0.38),
    ("mokhovik", 0.40),
    ("nochnitsa", 0.30),
    ("baba_yaga", 0.32),
    ("polunochnitsa", 0.28),
    ("zlyden", 0.35),
    ("morovitsa", 0.28),
    ("ogneya", 0.30),
    ("sea_guard", 0.30),
    ("gagana", 0.38),
    ("gorynych", 0.35),
    ("solovey", 0.32),
    ("korochun", 0.30),
    ("vodyanoy_tsar", 0.32),
]

rig_entries = []

for name, neck_ratio in targets:
    path = os.path.join(ROOT, f"{name}-gemini.png")
    if not os.path.exists(path):
        print(f"Missing: {path}")
        continue
    
    img = Image.open(path)
    # If RGB or green still present, key green
    if img.mode != "RGBA":
        keyed = fit(key_green(img), 1024, 1024, 0.957)
        keyed.save(path, optimize=True)
        img = keyed
    else:
        # Check if has green background
        rgb = np.array(img.convert("RGB"), dtype=np.float32)
        green = rgb[:, :, 1] - np.maximum(rgb[:, :, 0], rgb[:, :, 2])
        if (green > 30).sum() > 5000:
            keyed = fit(key_green(img), 1024, 1024, 0.957)
            keyed.save(path, optimize=True)
            img = keyed
            
    W, H = img.size
    alpha = np.array(img.getchannel("A"))
    ys, xs = np.nonzero(alpha > 30)
    if len(xs) == 0:
        print(f"Warning: no pixels in {name}")
        continue
        
    x0, y0, x1, y1 = xs.min(), ys.min(), xs.max(), ys.max()
    fig_h = y1 - y0
    
    neck_y = int(y0 + fig_h * neck_ratio)
    joint_x = round((x0 + x1) / (2.0 * W), 3)
    joint_y = round(neck_y / float(H), 3)
    
    head_mask = np.ones((H, W), dtype=np.float32)
    body_mask = np.ones((H, W), dtype=np.float32)
    
    feather = 6
    for y in range(H):
        if y < neck_y - feather:
            head_mask[y, :] = 1.0
            body_mask[y, :] = 0.0
        elif y > neck_y + feather:
            head_mask[y, :] = 0.0
            body_mask[y, :] = 1.0
        else:
            t = (y - (neck_y - feather)) / float(2 * feather)
            head_mask[y, :] = 1.0 - t
            body_mask[y, :] = t
            
    head_arr = np.array(img, dtype=np.float32)
    body_arr = np.array(img, dtype=np.float32)
    
    head_arr[:, :, 3] *= head_mask
    body_arr[:, :, 3] *= body_mask
    
    head_img = Image.fromarray(np.clip(head_arr, 0, 255).astype(np.uint8), "RGBA")
    body_img = Image.fromarray(np.clip(body_arr, 0, 255).astype(np.uint8), "RGBA")
    
    head_fn = os.path.join(ROOT, f"{name}_head-gemini.png")
    body_fn = os.path.join(ROOT, f"{name}_body-gemini.png")
    
    head_img.save(head_fn, optimize=True)
    body_img.save(body_fn, optimize=True)
    
    # Generate dead and dead_headless
    # For dead: rotate body 90 deg or create lying version
    dead_fn = os.path.join(ROOT, f"{name}_dead-gemini.png")
    dead_hl_fn = os.path.join(ROOT, f"{name}_dead_headless-gemini.png")
    
    # Make lying figure (rotated 80 deg, grounded)
    # Whole dead:
    whole_crop = img.crop((x0, y0, x1, y1))
    body_crop = body_img.crop((x0, y0, x1, y1))
    
    def make_lying(crop_fig):
        rot = crop_fig.rotate(-82, expand=True, resample=Image.BICUBIC)
        # plant on bottom
        canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
        # scale down slightly if needed
        k = min(900.0 / rot.width, 450.0 / rot.height, 1.0)
        nw, nh = max(1, int(rot.width * k)), max(1, int(rot.height * k))
        rz = rot.resize((nw, nh), Image.LANCZOS)
        canvas.alpha_composite(rz, ((1024 - nw) // 2, int(1024 * 0.957) - nh))
        return canvas
        
    dead_img = make_lying(whole_crop)
    dead_hl_img = make_lying(body_crop)
    
    dead_img.save(dead_fn, optimize=True)
    dead_hl_img.save(dead_hl_fn, optimize=True)
    
    rig_entries.append((name, joint_x, joint_y))
    print(f"Processed {name}: joint=({joint_x}, {joint_y})")

print("\n--- RIG ENTRIES FOR rig.rs ---")
for name, jx, jy in rig_entries:
    print(f'    ("enemies/{name}", &[(Part::Head, Vec2::new({jx}, {jy}), 1.0)]),')
