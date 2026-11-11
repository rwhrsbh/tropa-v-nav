import sys, os
sys.path.append(os.path.dirname(os.path.abspath(__file__)))
from cutout import key_green
from PIL import Image
import numpy as np

enemies = [
    'kot_bayun', 'mokhovik', 'nochnitsa', 'baba_yaga',
    'polunochnitsa', 'zlyden', 'morovitsa', 'ogneya',
    'sea_guard', 'gagana', 'gorynych'
]

results = []

for name in enemies:
    fn = f'assets/enemies/{name}-gemini.png'
    if not os.path.exists(fn):
        continue
    raw = Image.open(fn)
    keyed = key_green(raw)
    W, H = keyed.size
    
    # save transparent full sprite
    keyed.save(fn, optimize=True)
    
    # find bounding box of alpha
    alpha = np.array(keyed.getchannel('A'))
    ys, xs = np.nonzero(alpha > 30)
    if len(xs) == 0:
        continue
    x0, y0, x1, y1 = xs.min(), ys.min(), xs.max(), ys.max()
    fig_h = y1 - y0
    
    neck_ratio = 0.32
    if name in ['kot_bayun', 'gagana']:
        neck_ratio = 0.38
    elif name in ['mokhovik']:
        neck_ratio = 0.40
    elif name in ['morovitsa', 'polunochnitsa']:
        neck_ratio = 0.26
        
    neck_y = int(y0 + fig_h * neck_ratio)
    joint_x = round((x0 + x1) / (2.0 * W), 3)
    joint_y = round(neck_y / float(H), 3)
    
    head_mask = np.ones((H, W), dtype=np.float32)
    body_mask = np.ones((H, W), dtype=np.float32)
    
    for y in range(H):
        if y < neck_y - 4:
            head_mask[y, :] = 1.0
            body_mask[y, :] = 0.0
        elif y > neck_y + 4:
            head_mask[y, :] = 0.0
            body_mask[y, :] = 1.0
        else:
            t = (y - (neck_y - 4)) / 8.0
            head_mask[y, :] = 1.0 - t
            body_mask[y, :] = t
            
    head_arr = np.array(keyed, dtype=np.float32)
    body_arr = np.array(keyed, dtype=np.float32)
    
    head_arr[:, :, 3] *= head_mask
    body_arr[:, :, 3] *= body_mask
    
    head_img = Image.fromarray(np.clip(head_arr, 0, 255).astype(np.uint8), 'RGBA')
    body_img = Image.fromarray(np.clip(body_arr, 0, 255).astype(np.uint8), 'RGBA')
    
    head_fn = f'assets/enemies/{name}_head-gemini.png'
    body_fn = f'assets/enemies/{name}_body-gemini.png'
    
    head_img.save(head_fn, optimize=True)
    body_img.save(body_fn, optimize=True)
    
    results.append((name, joint_x, joint_y))
    print(f'Processed {name}: joint=({joint_x}, {joint_y})')

print('\nRig entries for crates/game/src/battle/rig.rs:')
for name, jx, jy in results:
    print(f'    ("enemies/{name}", &[(Part::Head, Vec2::new({jx}, {jy}), 1.0)]),')
