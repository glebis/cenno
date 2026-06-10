#!/usr/bin/env python3
"""White-on-mood (#FF6250) lockup explorations: positions x typefaces.

Mark is always the canonical SVG rendered by rsvg — never redrawn.
Each variant tile is 1600x1000; contact sheets are assembled from tiles.
"""
import pathlib
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFont

HERE = pathlib.Path(__file__).resolve().parent
MARK_SVG = HERE.parent / "cenno-mark.svg"
RED = "#FF6250"
TILE_W, TILE_H = 1600, 1000
HOME = pathlib.Path.home()

FONTS = [
    ("SF Pro 600",            "/System/Library/Fonts/SFNS.ttf",                          {"wght": 600}),
    ("PP Mori SemiBold",      HOME / "Library/Fonts/PPMori-SemiBold.otf",                 None),
    ("ABC Favorit Medium",    HOME / "Library/Fonts/ABCFavoritPro-Medium-Trial.otf",      None),
    ("Neue Machina Medium",   HOME / "Library/Fonts/PPNeueMachina-PlainMedium.otf",       None),
    ("Radio Grotesk Regular", HOME / "Library/Fonts/PPRadioGrotesk-Regular.otf",          None),
    ("Aeroport Medium",       HOME / "Library/Fonts/Aeroport-Medium.ttf",                 None),
    ("Pangram Rounded SemiB", HOME / "Library/Fonts/PPPangramSansRounded-Semibold.otf",   None),
    ("EB Garamond 500",       HOME / "Library/Fonts/EBGaramond-VariableFont_wght.ttf",    {"wght": 500}),
    ("Futura Medium",         "/System/Library/Fonts/Supplemental/Futura.ttc",            None),
]

_mark_cache = {}

def mark(px):
    if px not in _mark_cache:
        svg = MARK_SVG.read_text().replace("currentColor", "#FFFFFF")
        with tempfile.NamedTemporaryFile(suffix=".svg", mode="w", delete=False) as f:
            f.write(svg)
        out = f.name + ".png"
        subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), f.name, "-o", out],
                       check=True)
        _mark_cache[px] = Image.open(out).convert("RGBA")
    return _mark_cache[px]

def load_font(path, size, axes):
    f = ImageFont.truetype(str(path), size=size)
    if axes:
        try:
            f.set_variation_by_axes(list(axes.values()) if len(axes) > 1 else [axes["wght"]])
        except Exception:
            # variable axis order differs per file; map by name
            names = [a.axis.decode() if isinstance(a.axis, bytes) else a.axis
                     for a in f.get_variation_axes()]
            f.set_variation_by_axes([axes.get(n, a.default) for n, a in
                                     zip(names, f.get_variation_axes())])
    return f

def label(d, text):
    lf = ImageFont.truetype(str(HOME / "Library/Fonts/Inter-VariableFont_opsz,wght.ttf"), 34)
    d.text((40, TILE_H - 70), text.upper(), font=lf, fill=(255, 255, 255, 160))

def tile():
    img = Image.new("RGBA", (TILE_W, TILE_H), RED)
    return img, ImageDraw.Draw(img)

def centered_word(d, font, cx, cy):
    tb = font.getbbox("cenno")
    w, h = tb[2] - tb[0], tb[3] - tb[1]
    d.text((cx - w / 2 - tb[0], cy - h / 2 - tb[1]), "cenno", font=font, fill="#FFFFFF")
    return w, h

# ---- position variants (all SF Pro 600) ----------------------------------

def pos_horizontal(font_spec, name):
    img, d = tile()
    f = load_font(font_spec[1], 220, font_spec[2])
    m = mark(380)
    tb = f.getbbox("cenno")
    tw = tb[2] - tb[0]
    total = 380 + 140 + tw
    x = (TILE_W - total) // 2
    img.alpha_composite(m, (x, (TILE_H - 380) // 2))
    d.text((x + 380 + 140 - tb[0], TILE_H / 2 - (tb[3] - tb[1]) / 2 - tb[1]),
           "cenno", font=f, fill="#FFFFFF")
    label(d, name)
    return img

def pos_stacked(font_spec, name):
    img, d = tile()
    f = load_font(font_spec[1], 190, font_spec[2])
    m = mark(400)
    img.alpha_composite(m, ((TILE_W - 400) // 2, 130))
    centered_word(d, f, TILE_W / 2, 670)
    label(d, name)
    return img

def pos_diacritic(font_spec, name):
    img, d = tile()
    f = load_font(font_spec[1], 260, font_spec[2])
    tb = f.getbbox("cenno")
    tw, th = tb[2] - tb[0], tb[3] - tb[1]
    x = (TILE_W - tw) // 2 - tb[0]
    y = TILE_H * 0.58 - th / 2 - tb[1]
    d.text((x, y), "cenno", font=f, fill="#FFFFFF")
    # center of the 'e' glyph
    e_cx = (TILE_W - tw) // 2 + f.getlength("c") + (f.getlength("ce") - f.getlength("c")) / 2
    msz = 150
    img.alpha_composite(mark(msz), (round(e_cx - msz / 2), round(TILE_H * 0.58 - th / 2 - msz * 1.18)))
    label(d, name)
    return img

def pos_trailing(font_spec, name):
    img, d = tile()
    f = load_font(font_spec[1], 220, font_spec[2])
    m = mark(340)
    tb = f.getbbox("cenno")
    tw = tb[2] - tb[0]
    total = tw + 120 + 340
    x = (TILE_W - total) // 2
    d.text((x - tb[0], TILE_H / 2 - (tb[3] - tb[1]) / 2 - tb[1]), "cenno", font=f, fill="#FFFFFF")
    img.alpha_composite(m, (x + tw + 120, (TILE_H - 340) // 2))
    label(d, name)
    return img

def pos_poster(font_spec, name):
    img, d = tile()
    f = load_font(font_spec[1], 110, font_spec[2])
    m = mark(820)
    img.alpha_composite(m, ((TILE_W - 820) // 2, (TILE_H - 820) // 2 - 40))
    d.text((70, TILE_H - 190), "cenno", font=f, fill="#FFFFFF")
    label(d, name)
    return img

# ---- sheets ---------------------------------------------------------------

def sheet(tiles, cols, out):
    rows = (len(tiles) + cols - 1) // cols
    s = Image.new("RGBA", (TILE_W * cols, TILE_H * rows), RED)
    for i, t in enumerate(tiles):
        s.paste(t, ((i % cols) * TILE_W, (i // cols) * TILE_H))
    s = s.resize((s.width // 2, s.height // 2), Image.LANCZOS)
    s.convert("RGB").save(out, quality=95)
    print(out)

sf = FONTS[0]
positions = [
    pos_horizontal(sf, "A · horizontal"),
    pos_stacked(sf, "B · stacked"),
    pos_diacritic(sf, "C · diacritic"),
    pos_trailing(sf, "D · trailing"),
    pos_poster(sf, "E · poster"),
]
sheet(positions, 3, HERE / "sheet-positions.png")

fonts = [pos_stacked(spec, spec[0]) for spec in FONTS]
sheet(fonts, 3, HERE / "sheet-fonts.png")

# full-res singles for every position variant
for img, n in zip(positions, ["horizontal", "stacked", "diacritic", "trailing", "poster"]):
    img.convert("RGB").save(HERE / f"pos-{n}.png", quality=95)
print("singles saved")
