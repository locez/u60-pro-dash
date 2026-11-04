#!/usr/bin/env python3
"""Bake a subtle neutral light field onto RGB565-exact grays (Pillow)."""
from pathlib import Path
import math
from PIL import Image

ASSETS = Path(__file__).resolve().parents[1] / "assets"
WIDTH, HEIGHT = 320, 180

for theme, base in [("light", 239), ("dark", 16)]:
    pixels = []
    for y in range(HEIGHT):
        for x in range(WIDTH):
            # The field is baked once. A fixed spatial dither avoids visible
            # bands; every stored gray survives RGB565 without a green cast.
            glow = math.exp(-((x - 80) / 250) ** 2) * (1 - y / (HEIGHT - 1)) ** 2
            value = base + 16 * glow
            lower = base + int((value - base) // 8) * 8
            mix = (value - lower) / 8
            seed = ((x + 1) * 374761393 + (y + 1) * 668265263) & 0xFFFFFFFF
            seed = ((seed ^ (seed >> 13)) * 1274126177) & 0xFFFFFFFF
            threshold = ((seed ^ (seed >> 16)) & 0xFFFF) / 65536
            gray = min(base + 16, lower + (8 if threshold < mix else 0))
            pixels.append((gray, gray, gray))
    image = Image.new("RGB", (WIDTH, HEIGHT))
    image.putdata(pixels)
    image.save(ASSETS / f"material-{theme}.png", optimize=True)
