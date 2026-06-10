#!/usr/bin/env python3
"""Deconstructed lockup: the mark's dot drops onto the text baseline and
becomes the bullet of 'cenno'; the arc hovers above it at canonical scale.

Proportions derive from the canonical 22px-space mark (arc r 5.5 stroke 2.5,
dot r 2.75): arc and dot keep their relative sizes, only the dot's position
changes. Dot diameter is tied to the wordmark x-height. White on
color.flow.mood #FF6250.
"""
import pathlib
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFont

HERE = pathlib.Path(__file__).resolve().parent
MARK_SVG = HERE.parent / "cenno-mark.svg"
RED = "#FF6250"
TILE_W, TILE_H = 1500, 750
HOME = pathlib.Path.home()

ARC_PATH = '<path d="M 5.6874 7.5765 A 5.5 5.5 0 0 1 16.3126 7.5765" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-linecap="butt"/>'

def arc_image(unit_px):
    """Render only the canonical arc, cropped to its alpha bbox."""
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 22 22" '
           f'width="22" height="22">{ARC_PATH}</svg>')
    with tempfile.NamedTemporaryFile(suffix=".svg", mode="w", delete=False) as f:
        f.write(svg)
    out = f.name + ".png"
    px = round(22 * unit_px)
    subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), f.name, "-o", out],
                   check=True)
    img = Image.open(out).convert("RGBA")
    return img.crop(img.getbbox())

FONTS = [
    ("SF Pro 600",           "/System/Library/Fonts/SFNS.ttf",                        0, 600),
    ("Helvetica Neue Bold",  "/System/Library/Fonts/HelveticaNeue.ttc",               1, None),
    ("PP Mori SemiBold",     HOME / "Library/Fonts/PPMori-SemiBold.otf",              0, None),
    ("ABC Favorit Medium",   HOME / "Library/Fonts/ABCFavoritPro-Medium-Trial.otf",   0, None),
    ("Pangram Rounded SemiB", HOME / "Library/Fonts/PPPangramSansRounded-Semibold.otf", 0, None),
]

def label(d, text):
    lf = ImageFont.truetype(str(HOME / "Library/Fonts/Inter-VariableFont_opsz,wght.ttf"), 32)
    d.text((36, TILE_H - 64), text.upper(), font=lf, fill=(255, 255, 255, 160))

def measure(font):
    xb = font.getbbox("x")
    D = xb[3] - xb[1]                      # dot diameter = x-height
    tb = font.getbbox("cenno")
    total_w = D + 1.0 * D + (tb[2] - tb[0])
    total_h = (13.5 / 5.5) * 0.62 * D + 2.0 * D + D   # arc + gap + dot, approx
    return D, tb, total_w, total_h

def variant(name, font_path, index, wght, arc_shift, size=300, tag=""):
    def load(sz):
        f = ImageFont.truetype(str(font_path), size=sz, index=index)
        if wght:
            f.set_variation_by_axes([wght])
        return f
    font = load(size)
    _, _, total_w, total_h = measure(font)
    scale = min(1.0, 0.72 * TILE_W / total_w, 0.58 * TILE_H / total_h)
    font = load(round(size * scale))
    D, tb, total_w, _ = measure(font)
    unit = D / 5.5                         # canonical 22px-space unit
    arc = arc_image(unit)

    word_w = tb[2] - tb[0]
    gap = round(1.0 * D)                   # dot -> word gap
    x0 = (TILE_W - total_w) / 2            # dot left edge
    baseline = TILE_H * 0.66               # text baseline = dot bottom

    img = Image.new("RGBA", (TILE_W, TILE_H), RED)
    d = ImageDraw.Draw(img)
    # dot on the baseline (canonical circle — geometry is exact by definition)
    d.ellipse([x0, baseline - D, x0 + D, baseline], fill="#FFFFFF")
    # arc above: vertical gap 1.0*D from dot top, center shifted by arc_shift*D
    dot_cx = x0 + D / 2
    ax = dot_cx + arc_shift * D - arc.width / 2
    ay = baseline - D - 1.0 * D - arc.height
    img.alpha_composite(arc, (round(ax), round(ay)))
    # word: 'c' starts after the dot, sitting on the same baseline
    d.text((x0 + D + gap - tb[0], baseline - tb[3]), "cenno", font=font, fill="#FFFFFF")
    label(d, f"{name}{tag}")
    return img

tiles = [variant(spec[0], *spec[1:], arc_shift=0.0) for spec in FONTS]

names = ["sfpro", "helvetica", "ppmori", "favorit", "pangram-rounded"]
for t, n in zip(tiles, names):
    t.convert("RGB").save(HERE / f"inline-{n}.png", quality=95)

cols, rows = 2, 3
s = Image.new("RGBA", (TILE_W * cols, TILE_H * rows), RED)
for i, t in enumerate(tiles):
    s.paste(t, ((i % cols) * TILE_W, (i // cols) * TILE_H))
s.convert("RGB").save(HERE / "sheet-inline.png", quality=95)
print("done:", ", ".join(f"inline-{n}.png" for n in names))
