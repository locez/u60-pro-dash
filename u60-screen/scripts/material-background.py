#!/usr/bin/env python3
"""Bake the static slate/light surface field for the software renderer (Pillow)."""
from pathlib import Path
import math
from PIL import Image

ASSETS = Path(__file__).resolve().parents[1] / "assets"
WIDTH, HEIGHT = 320, 180

for theme, base in [("light", (244, 246, 250)), ("dark", (32, 42, 55))]:
    pixels = []
    for y in range(HEIGHT):
        for x in range(WIDTH):
            # A fixed, faint light field. No runtime noise, animation or blur.
            glow = 4 * math.exp(-((x - 80) / 250) ** 2) * (1 - y / (HEIGHT - 1)) ** 2
            lower = int(glow)
            seed = ((x + 1) * 374761393 + (y + 1) * 668265263) & 0xFFFFFFFF
            seed = ((seed ^ (seed >> 13)) * 1274126177) & 0xFFFFFFFF
            threshold = ((seed ^ (seed >> 16)) & 0xFFFF) / 65536
            lift = lower + (1 if threshold < glow - lower else 0)
            pixels.append(tuple(min(255, v + lift) for v in base))
    image = Image.new("RGB", (WIDTH, HEIGHT))
    image.putdata(pixels)
    image.save(ASSETS / f"material-{theme}.png", optimize=True)
