#!/usr/bin/env python3
"""Finalize cenno app-icon masters.

Takes the AI glass-treated edit (edit-v3.png, gpt-image-2 edit mode) and
composites its mark region (plus shadow/refraction halo) onto the pristine
brand-coral gradient from render_appicon.backdrop(), restoring exact #FF6250
backdrop fidelity. Outputs:

  master-fullbleed.png  1024 full square  -> feed to `npx tauri icon`
  master-squircle.png   1024 pre-masked (macOS squircle, r ~= 22.37%)
"""
from __future__ import annotations

import sys
from PIL import Image, ImageFilter

sys.path.insert(0, "/Users/glebkalinin/ai_projects/cenno/docs/design/brand/appicon")
from render_appicon import backdrop, mark_layer, mask_squircle

OUT = "/Users/glebkalinin/ai_projects/cenno/docs/design/brand/appicon"


def main() -> None:
    glass = Image.open(f"{OUT}/edit-v3.png").convert("RGBA").resize((1024, 1024))
    base = backdrop(1024)

    # Soft halo mask around the canonical mark: keeps the AI's glass material,
    # shadow and edge refraction, blends to pristine coral over ~100 px.
    halo = mark_layer(1024).split()[3]
    halo = halo.filter(ImageFilter.MaxFilter(31))          # dilate ~15 px
    halo = halo.filter(ImageFilter.GaussianBlur(45))       # wide soft falloff
    halo = halo.point(lambda a: min(255, int(a * 1.6)))    # solid core, soft skirt

    final = Image.composite(glass, base, halo)
    final.convert("RGB").convert("RGBA").save(f"{OUT}/master-fullbleed.png")

    sq = mask_squircle(final)
    sq.save(f"{OUT}/master-squircle.png")
    print("wrote master-fullbleed.png, master-squircle.png")


if __name__ == "__main__":
    main()
