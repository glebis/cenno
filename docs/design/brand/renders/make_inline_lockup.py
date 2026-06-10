#!/usr/bin/env python3
"""cenno inline lockup — the canonical large-size logo.

The mark's dot sits on the text baseline as the bullet of 'cenno'; the arc
floats above it, centered. All proportions are parametric in D, the dot
diameter, which equals the wordmark x-height:

  dot diameter      D  = x-height of the wordmark (= 5.5 canonical units)
  word gap          1.0 D   (dot right edge -> 'c')
  arc outer width   2.4545 D  (canonical 13.5u / 5.5u, arc rendered from SVG)
  arc gap           1.0 D   (arc bottom -> dot top)
  clear space       1.0 D   on all sides (= one arc radius, per BRAND.md)

Wordmark: 'cenno', lowercase, SF Pro (SFNS.ttf) weight 600.
Arc and dot are rendered with rsvg from canonical geometry — never redrawn.
"""
import pathlib
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFont

HERE = pathlib.Path(__file__).resolve().parent
FONT = "/System/Library/Fonts/SFNS.ttf"
SIZE = 1200  # font size in px; everything scales from it

ARC = '<path d="M 5.6874 7.5765 A 5.5 5.5 0 0 1 16.3126 7.5765" fill="none" stroke="{c}" stroke-width="2.5" stroke-linecap="butt"/>'
DOT = '<circle cx="11" cy="16.25" r="2.75" fill="{c}"/>'

def rsvg(body, px):
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 22 22" '
           f'width="22" height="22">{body}</svg>')
    with tempfile.NamedTemporaryFile(suffix=".svg", mode="w", delete=False) as f:
        f.write(svg)
    out = f.name + ".png"
    subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), f.name, "-o", out],
                   check=True)
    img = Image.open(out).convert("RGBA")
    return img.crop(img.getbbox())

def lockup(color, bg, out):
    font = ImageFont.truetype(FONT, size=SIZE)
    font.set_variation_by_axes([600])
    xb = font.getbbox("x")
    D = xb[3] - xb[1]
    unit = D / 5.5
    arc = rsvg(ARC.format(c=color), round(22 * unit))
    dot = rsvg(DOT.format(c=color), round(22 * unit))

    tb = font.getbbox("cenno")
    word_w = tb[2] - tb[0]
    pad = D                                  # clear space
    gap = round(1.0 * D)
    w = pad + D + gap + word_w + pad
    arc_h = arc.height
    h = pad + arc_h + round(1.0 * D) + D + pad

    img = Image.new("RGBA", (w, h), bg)
    d = ImageDraw.Draw(img)
    baseline = pad + arc_h + round(1.0 * D) + D
    dot_cx = pad + D / 2
    img.alpha_composite(dot, (round(dot_cx - dot.width / 2), round(baseline - dot.height)))
    img.alpha_composite(arc, (round(dot_cx - arc.width / 2), pad))
    d.text((pad + D + gap - tb[0], baseline - tb[3]), "cenno", font=font, fill=color)
    img.save(out)
    print(out, img.size)

lockup("#FFFFFF", "#FF6250", HERE / "cenno-lockup-inline-white-on-mood.png")
lockup("#000000", (0, 0, 0, 0), HERE / "cenno-lockup-inline-black.png")
lockup("#FFFFFF", "#14171A", HERE / "cenno-lockup-inline-white-on-ambient.png")
lockup("#1E4FD8", "#FAF8F5", HERE / "cenno-lockup-inline-question-on-paper.png")
