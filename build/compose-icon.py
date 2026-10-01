#!/usr/bin/env python3
"""
Compose the app icon: place the source logo on a dark (matte)
macOS-style rounded-square background.

Usage: python3 compose-icon.py <logo.png> <out.png> [size]
"""
import sys
from PIL import Image, ImageDraw

SRC = sys.argv[1]
OUT = sys.argv[2]
SIZE = int(sys.argv[3]) if len(sys.argv) > 3 else 1024

# --- background: dark rounded square (macOS "squircle"-ish) ---
# Work at high resolution then downscale for smooth edges.
SS = 4  # supersample factor
S = SIZE * SS
radius = int(S * 0.225)  # macOS icon corner radius ratio

# Vertical dark gradient (top slightly lighter -> bottom darker) for depth.
top = (46, 52, 68)     # #2E3444
bottom = (22, 25, 34)  # #161922
grad = Image.new("RGB", (1, S))
for y in range(S):
    t = y / (S - 1)
    r = round(top[0] + (bottom[0] - top[0]) * t)
    g = round(top[1] + (bottom[1] - top[1]) * t)
    b = round(top[2] + (bottom[2] - top[2]) * t)
    grad.putpixel((0, y), (r, g, b))
bg = grad.resize((S, S))

# Rounded-square alpha mask.
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=radius, fill=255)

icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))
icon.paste(bg, (0, 0), mask)

# --- logo on top, centered, with padding ---
logo = Image.open(SRC).convert("RGBA")
pad = 0.16  # fraction of canvas as margin on each side
target = int(S * (1 - 2 * pad))
lw, lh = logo.size
scale = target / max(lw, lh)
logo = logo.resize((round(lw * scale), round(lh * scale)), Image.LANCZOS)
lx = (S - logo.width) // 2
ly = (S - logo.height) // 2
icon.alpha_composite(logo, (lx, ly))

# Downscale to final size.
icon = icon.resize((SIZE, SIZE), Image.LANCZOS)
icon.save(OUT)
print(f"wrote {OUT} ({SIZE}x{SIZE})")
