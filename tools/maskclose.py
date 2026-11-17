"""Closes gaps Flow leaves inside one flat colour of a rig colour mask (a bracer or a belt left
unpainted across an arm), so tools/rigparts.py cuts the part whole.

    python tools/maskclose.py <colour_mask.jpg> <colour: arm|head|shield|arm2> [<radius px>=25]

Overwrites the mask with the closed colour painted back in.
"""
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

COLOURS = {"arm": (255, 0, 255), "head": (0, 255, 255), "shield": (255, 255, 0), "arm2": (0, 255, 0)}
path, part = sys.argv[1], sys.argv[2]
r = int(sys.argv[3]) if len(sys.argv) > 3 else 25
img = Image.open(path).convert("RGB")
a = np.asarray(img).copy()
c = np.array(COLOURS[part])
m = np.linalg.norm(a.astype(float) - c, axis=2) < 90
disk = np.hypot(*np.mgrid[-r:r + 1, -r:r + 1]) <= r
closed = ndimage.binary_closing(m, structure=disk)
added = closed & ~m
a[added] = c
Image.fromarray(a).save(path, quality=98)
print(part, "added", int(added.sum()), "px")
