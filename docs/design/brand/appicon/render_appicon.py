#!/usr/bin/env python3
"""cenno macOS Tahoe app-icon master renderer.

Renders the canonical arc-over-dot mark (tokens/tokens.json ->
brand.$extensions["app.cenno.mark"]) onto a flow-coral backdrop:

  - flat-fullbleed.png      flat composition (input for AI glass edit)
  - pillow-fullbleed.png    pure-Pillow Liquid-Glass-adjacent treatment
  - squircle masking helper (mask_squircle) for the pre-masked deliverable

Geometry (22-unit grid, canonical): arc center (11,9) r 5.5 stroke 2.5,
sweep 195..345 deg (PIL convention), dot (11,16.25) r 2.75.
Mark height (arc outer top 2.25 .. dot bottom 19.0 = 16.75 units) scaled
to ~55% of icon height, optically centered.
"""
from __future__ import annotations

import math
import sys
from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
SS = 4  # supersample factor
HI = SIZE * SS

# --- canonical mark geometry (22-unit grid) ---
GRID = 22.0
ARC_C = (11.0, 9.0)
ARC_R = 5.5
ARC_W = 2.5
ARC_START, ARC_END = 195, 345
DOT_C = (11.0, 16.25)
DOT_R = 2.75

MARK_TOP = ARC_C[1] - ARC_R - ARC_W / 2      # 2.25
MARK_BOT = DOT_C[1] + DOT_R                  # 19.0
MARK_H_UNITS = MARK_BOT - MARK_TOP           # 16.75
MARK_CY_UNITS = (MARK_TOP + MARK_BOT) / 2    # 10.625

MARK_FRAC = 0.55  # mark height as fraction of icon height

CORAL = (255, 98, 80)  # #FF6250


def mark_layer(size: int = SIZE, color=(255, 255, 255, 255)) -> Image.Image:
    """White mark, exact canonical geometry, centered, ~55% height."""
    hi = size * SS
    img = Image.new("RGBA", (hi, hi), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    scale = (MARK_FRAC * size) / MARK_H_UNITS * SS  # px per grid unit (hi-res)
    cx = hi / 2
    cy = hi / 2

    def to_px(ux: float, uy: float) -> tuple[float, float]:
        return (cx + (ux - ARC_C[0]) * scale, cy + (uy - MARK_CY_UNITS) * scale)

    # Arc, butt caps (PIL arc with width gives butt-like ends; at 4x SS it's clean)
    acx, acy = to_px(*ARC_C)
    r = ARC_R * scale
    w = ARC_W * scale
    d.arc([acx - r, acy - r, acx + r, acy + r], start=ARC_START, end=ARC_END,
          fill=color, width=int(round(w)))

    dcx, dcy = to_px(*DOT_C)
    dr = DOT_R * scale
    d.ellipse([dcx - dr, dcy - dr, dcx + dr, dcy + dr], fill=color)

    return img.resize((size, size), Image.Resampling.LANCZOS)


def backdrop(size: int = SIZE) -> Image.Image:
    """flow-coral with a very subtle vertical luminosity gradient (Tahoe-style)."""
    img = Image.new("RGB", (1, size))
    top = tuple(min(255, int(c * 1.055 + 4)) for c in CORAL)      # gently lighter
    bot = tuple(max(0, int(c * 0.94)) for c in CORAL)             # gently darker
    for y in range(size):
        t = y / (size - 1)
        # ease so most of the field stays close to brand coral
        t2 = t * t * (3 - 2 * t)
        img.putpixel((0, y), tuple(int(top[i] + (bot[i] - top[i]) * t2) for i in range(3)))
    return img.resize((size, size)).convert("RGBA")


def squircle_mask(size: int = SIZE, radius_frac: float = 0.2237) -> Image.Image:
    """macOS-style continuous-corner squircle mask (superellipse corners)."""
    hi = size * SS
    r = radius_frac * size * SS
    n = 4.0  # superellipse exponent for continuous-corner feel
    mask = Image.new("L", (hi, hi), 0)
    d = ImageDraw.Draw(mask)

    pts = []
    steps = 256
    corners = [  # (corner center, quadrant angle range)
        ((hi - r, r), (-90, 0)),      # top-right
        ((hi - r, hi - r), (0, 90)),  # bottom-right
        ((r, hi - r), (90, 180)),     # bottom-left
        ((r, r), (180, 270)),         # top-left
    ]
    for (ccx, ccy), (a0, a1) in corners:
        for i in range(steps + 1):
            a = math.radians(a0 + (a1 - a0) * i / steps)
            ca, sa = math.cos(a), math.sin(a)
            x = ccx + r * math.copysign(abs(ca) ** (2 / n), ca)
            y = ccy + r * math.copysign(abs(sa) ** (2 / n), sa)
            pts.append((x, y))
    d.polygon(pts, fill=255)
    return mask.resize((size, size), Image.Resampling.LANCZOS)


def flat_composition(size: int = SIZE) -> Image.Image:
    base = backdrop(size)
    base.alpha_composite(mark_layer(size))
    return base


def pillow_glass(size: int = SIZE) -> Image.Image:
    """Restrained Tahoe-adjacent glass treatment, pure Pillow."""
    base = backdrop(size)
    mark_a = mark_layer(size).split()[3]  # alpha of the mark

    # 1. soft drop shadow of the mark (inside the icon), dark coral, down-offset
    sh = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    shadow_col = (120, 30, 22, 70)
    sh_layer = Image.new("RGBA", (size, size), shadow_col)
    sh.paste(sh_layer, (0, int(size * 0.012)), mark_a)
    sh = sh.filter(ImageFilter.GaussianBlur(size * 0.012))
    base.alpha_composite(sh)

    # 2. outer frosted glow behind the mark (8% white, wide blur)
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    glow_layer = Image.new("RGBA", (size, size), (255, 255, 255, 255))
    glow.paste(glow_layer, (0, 0), mark_a)
    glow = glow.filter(ImageFilter.GaussianBlur(size * 0.028))
    glow.putalpha(glow.split()[3].point(lambda a: int(a * 0.08)))
    base.alpha_composite(glow)

    # 3. the mark itself: frosted glass white. 92% white with a faint internal
    #    vertical gradient (brighter top edge -> slightly cooler base), plus a
    #    1.5px darker bottom-edge inside the shapes for depth.
    grad = Image.new("L", (1, size))
    for y in range(size):
        t = y / (size - 1)
        grad.putpixel((0, y), int(255 * (0.97 - 0.10 * t)))  # 97% .. 87% white
    grad = grad.resize((size, size))
    mark_rgb = Image.merge("RGB", (grad, grad, grad)).convert("RGBA")
    mark_rgba = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    mark_rgba.paste(mark_rgb, (0, 0), mark_a.point(lambda a: int(a * 0.94)))

    # inner bottom-edge shade: alpha minus alpha shifted up => bottom rim
    up = Image.new("L", (size, size), 0)
    up.paste(mark_a, (0, -max(2, int(size * 0.004))))
    rim = Image.composite(Image.new("L", (size, size), 255),
                          Image.new("L", (size, size), 0),
                          mark_a)
    import PIL.ImageChops as C
    bottom_rim = C.subtract(mark_a, up)
    bottom_rim = bottom_rim.filter(ImageFilter.GaussianBlur(size * 0.002))
    rim_layer = Image.new("RGBA", (size, size), (180, 70, 58, 255))
    rim_rgba = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    rim_rgba.paste(rim_layer, (0, 0), bottom_rim.point(lambda a: int(a * 0.35)))
    mark_rgba.alpha_composite(rim_rgba)

    # inner top-edge light: alpha minus alpha shifted down => top rim highlight
    down = Image.new("L", (size, size), 0)
    down.paste(mark_a, (0, max(2, int(size * 0.004))))
    top_rim = C.subtract(mark_a, down)
    top_rim = top_rim.filter(ImageFilter.GaussianBlur(size * 0.0015))
    tl_layer = Image.new("RGBA", (size, size), (255, 255, 255, 255))
    tl_rgba = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    tl_rgba.paste(tl_layer, (0, 0), top_rim.point(lambda a: int(a * 0.55)))
    mark_rgba.alpha_composite(tl_rgba)

    base.alpha_composite(mark_rgba)

    # 4. very subtle diagonal highlight band across the top third (glass sheen)
    sheen = Image.new("L", (HI // SS, HI // SS), 0)
    sd = ImageDraw.Draw(sheen)
    band = [(-size * 0.2, -size * 0.25), (size * 0.75, -size * 0.25),
            (size * 0.35, size * 0.42), (-size * 0.2, size * 0.42)]
    sd.polygon(band, fill=255)
    sheen = sheen.filter(ImageFilter.GaussianBlur(size * 0.09))
    sheen = sheen.point(lambda a: int(a * 0.07))
    sheen_rgba = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    sheen_rgba.paste(Image.new("RGBA", (size, size), (255, 255, 255, 255)), (0, 0), sheen)
    base.alpha_composite(sheen_rgba)

    # 5. faint inner edge light around the squircle boundary (glass slab edge)
    m = squircle_mask(size)
    inner = m.filter(ImageFilter.GaussianBlur(size * 0.006))
    import PIL.ImageChops as C2
    edge = C2.subtract(m, inner)
    # top-weighted: fade edge light toward the bottom
    fade = Image.new("L", (1, size))
    for y in range(size):
        fade.putpixel((0, y), int(255 * max(0.0, 1.0 - y / (size * 0.85))))
    fade = fade.resize((size, size))
    edge = C2.multiply(edge, fade)
    edge = edge.point(lambda a: int(a * 0.5))
    edge_rgba = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    edge_rgba.paste(Image.new("RGBA", (size, size), (255, 255, 255, 255)), (0, 0), edge)
    base.alpha_composite(edge_rgba)

    return base


def mask_squircle(img: Image.Image) -> Image.Image:
    out = img.convert("RGBA").copy()
    out.putalpha(squircle_mask(out.size[0]))
    return out


if __name__ == "__main__":
    out_dir = "/Users/glebkalinin/ai_projects/cenno/docs/design/brand/appicon"
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    if which in ("flat", "all"):
        flat_composition().save(f"{out_dir}/flat-fullbleed.png")
        print("wrote flat-fullbleed.png")
    if which in ("pillow", "all"):
        pillow_glass().save(f"{out_dir}/pillow-fullbleed.png")
        print("wrote pillow-fullbleed.png")
