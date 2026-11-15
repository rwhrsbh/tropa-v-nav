"""Keys a figure painted on flat green (or any flat colour) out of its picture by flooding the
background in from the picture's edges: whatever the flood cannot reach stays solid, so green
glints inside fur, eyes or cloth never turn into holes, as a plain colour key makes them. Gaps
of background shut in by the figure (between legs, inside an arm's crook) are found by size: a
patch of the background's own colour larger than `gap` pixels is background too.

    from key_flood import key_flood
    rgba = key_flood(Image.open("flow.jpg"))
"""
import numpy as np
from PIL import Image, ImageFilter
from scipy import ndimage


def key_flood(img, tol=40.0, gap=300):
    a = np.asarray(img.convert("RGB")).astype(np.float32)
    corners = np.concatenate([a[:12, :12].reshape(-1, 3), a[:12, -12:].reshape(-1, 3), a[-12:, :12].reshape(-1, 3), a[-12:, -12:].reshape(-1, 3)])
    bg = np.median(corners, axis=0)
    near = np.linalg.norm(a - bg, axis=2) < tol
    lab, n = ndimage.label(near)
    edge = np.unique(np.concatenate([lab[0], lab[-1], lab[:, 0], lab[:, -1]]))
    sizes = ndimage.sum(np.ones_like(near, dtype=np.float32), lab, range(1, n + 1))
    big = [i + 1 for i in range(n) if sizes[i] >= gap]
    outside = np.isin(lab, np.union1d(edge[edge > 0], big))
    # pure chroma green is never paint: a pocket of it too small for `gap` is background as well
    excess = a[..., 1] - np.maximum(a[..., 0], a[..., 2])
    outside |= ndimage.binary_dilation((excess > 90) & (a[..., 1] > 140), iterations=1)
    # a soft edge one pixel wide, never eating into the figure
    solid = ~ndimage.binary_dilation(outside, iterations=1)
    alpha = np.asarray(Image.fromarray((solid * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.9))).astype(np.float32)
    # take the background's colour back out of the pixels near it, which it tinted
    g = a[..., 1]
    spill = np.clip(g - np.maximum(a[..., 0], a[..., 2]), 0, None)
    rim = ndimage.binary_dilation(outside, iterations=6)
    a[..., 1] = np.where(rim, g - spill, g)
    return Image.fromarray(np.dstack([a, alpha]).clip(0, 255).astype(np.uint8), "RGBA")
