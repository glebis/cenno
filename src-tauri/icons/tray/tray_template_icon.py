#!/usr/bin/env python3
from __future__ import annotations

from PIL import Image, ImageDraw

HI_RES = 352
SCALE = 16
TARGETS = [(22, 'trayTemplate.png'), (44, 'trayTemplate@2x.png')]

# Geometry is authored in final 22px space, then upscaled by SCALE.
BASE_SIZE = 22
ARC_CENTER = (BASE_SIZE / 2, 9.0)
ARC_RADIUS = 5.5
ARC_STROKE = 2.5
ARC_START = 195
ARC_END = 345
DOT_RADIUS = 2.75
DOT_CENTER = (BASE_SIZE / 2, 16.25)


def render_template_icon() -> Image.Image:
    img = Image.new('RGBA', (HI_RES, HI_RES), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    cx = HI_RES / 2
    arc_cx = ARC_CENTER[0] * SCALE
    arc_cy = ARC_CENTER[1] * SCALE
    arc_r = ARC_RADIUS * SCALE
    arc_w = ARC_STROKE * SCALE

    # Single arc: centered top mark, no extra strokes.
    draw.arc(
        [
            arc_cx - arc_r,
            arc_cy - arc_r,
            arc_cx + arc_r,
            arc_cy + arc_r,
        ],
        start=ARC_START,
        end=ARC_END,
        fill=(0, 0, 0, 255),
        width=int(round(arc_w)),
    )

    # Single filled dot beneath the arc.
    dot_r = DOT_RADIUS * SCALE
    dot_cx = cx
    dot_cy = DOT_CENTER[1] * SCALE
    draw.ellipse(
        [dot_cx - dot_r, dot_cy - dot_r, dot_cx + dot_r, dot_cy + dot_r],
        fill=(0, 0, 0, 255),
    )

    return img


def downscale(src: Image.Image, size: int, out_name: str) -> None:
    dst = src.resize((size, size), resample=Image.Resampling.LANCZOS)
    # Enforce a clean transparent frame to avoid anti-aliased bleed-to-edge.
    margin = 2
    px = dst.load()
    width, height = dst.size
    for x in range(width):
        for y in range(margin):
            px[x, y] = (0, 0, 0, 0)
            px[x, height - 1 - y] = (0, 0, 0, 0)
    for y in range(height):
        for x in range(margin):
            px[x, y] = (0, 0, 0, 0)
            px[width - 1 - x, y] = (0, 0, 0, 0)
    dst.save(out_name, format='PNG')


def verify_images():
    import os

    for size, name in TARGETS:
        if not os.path.exists(name):
            raise SystemExit(f'Missing output: {name}')

        im = Image.open(name)
        data = im.convert('RGBA')
        if data.size != (size, size):
            raise SystemExit(f'{name} has unexpected size {data.size}, expected ({size}, {size})')
        if data.mode != 'RGBA':
            raise SystemExit(f'{name} mode is {data.mode}, expected RGBA')

        pix = data.getdata()
        alpha_values = [p[3] for p in pix]

        has_transparent = any(a == 0 for a in alpha_values)
        has_opaque_black = False
        for p in pix:
            if p[:3] == (0, 0, 0) and p[3] >= 255:
                has_opaque_black = True
                break

        if not has_transparent:
            raise SystemExit(f'{name} has no fully transparent pixels')
        if not has_opaque_black:
            raise SystemExit(f'{name} has no opaque black pixels')

        print(f'{name}: {data.size}, {data.mode}, transparent={has_transparent}, opaque_black={has_opaque_black}')


def main():
    src = render_template_icon()
    for size, name in TARGETS:
        downscale(src, size, name)
    verify_images()


if __name__ == '__main__':
    main()
