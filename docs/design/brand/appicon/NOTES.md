# cenno app icon — Tahoe (Liquid Glass) master

## Which file to use

- **`master-fullbleed.png`** (1024×1024, opaque square) — **feed this to `npx tauri icon`**.
  macOS 26 Tahoe re-masks app icons into the system squircle itself; the bundler
  must get the full-bleed square so nothing is double-masked.
- `master-squircle.png` (1024×1024, transparent corners) — pre-masked variant
  (continuous-corner squircle, corner radius ≈ 22.37 % of side, superellipse-corner
  approximation). For previews, docs, web, or any target that wants the shape baked in.

## Treatment used: gpt-image-2 edit mode (attempt 3 of 3), then recomposite

1. `render_appicon.py` renders the **exact canonical mark geometry** with Pillow at
   4× supersampling: arc center (11, 9), r 5.5, **stroke 2.5**, sweep 195°→345°
   (PIL convention); dot (11, 16.25), r 2.75; 22-unit grid. Mark height = 55 % of
   icon, optically centered. Backdrop = flow-coral `#FF6250` with a subtle vertical
   luminosity gradient (≈ +5 % top → −6 % bottom).
   → `flat-fullbleed.png` (the edit input).

   ⚠️ Note: the task brief said stroke 2.2, but all three canonical sources
   (`tokens/tokens.json`, `docs/design/brand/cenno-mark.svg`,
   `src-tauri/icons/tray/tray_template_icon.py`) say **2.5** — repo canon was used.

2. Glass treatment via `gpt-image-2` **edit mode** (`--edit flat-fullbleed.png`,
   quality medium, 1024×1024). Three iterations:
   - v1: fully transparent glass — beautiful but the mark nearly vanished at 64 px. Rejected.
   - v2: frosted white mark, but the backdrop went photographic (sheen band + vignette). Rejected.
   - v3 (**kept**, raw output preserved as `edit-v3.png`): frosted milky-white glass mark,
     clean backdrop. Geometric check: mark bbox 354×520 px vs canonical 362×526 px
     (≤ 1.5 % shrink, entirely soft-edge falloff under the detection threshold; centering exact).

   Final v3 prompt:
   > Apply Apple's macOS 26 Tahoe 'Liquid Glass' app icon material treatment.
   > CRITICAL: the coral background (#FF6250) must stay a perfectly clean, smooth
   > digital gradient — only a very subtle vertical luminosity shift, NO photographic
   > lighting, NO vignette, NO dark areas, NO visible sheen bands on the background.
   > Keep the arc-over-dot mark in EXACTLY the same position, size, proportions and
   > shape — do not move, scale, or redraw it. Treatment applies ONLY to the mark:
   > it becomes frosted milky-white translucent glass (about 85 % opaque, clearly
   > reading as white), with slight material thickness — soft inner luminosity, faint
   > coral refraction near its edges, one restrained thin specular highlight along its
   > top edges, and a very soft small drop shadow directly beneath the glass shapes.
   > No skeuomorphic gloss, no border, no text, no extra elements. Flat full-bleed
   > square output, no rounded corners, no 3D perspective.

3. `finalize_master.py` composites v3's mark region (canonical mark alpha, dilated
   ~15 px + 45 px Gaussian halo, so shadow and edge refraction survive) onto the
   **pristine programmatic coral gradient** — v3's backdrop had drifted ~3 % off
   brand coral; the master's backdrop is back to exact `#FF6250` ± gradient.

## Fallback (not used, kept for reference)

`pillow-fallback-fullbleed.png` — pure-Pillow treatment (92 % white mark, 8 % white
glow, inner soft shadow, low-alpha diagonal sheen, top-weighted squircle edge light),
built by `render_appicon.py pillow`. Most legible at 16–32 px but reads grayer/flatter
than the AI glass. If the glass mark ever proves too faint at tiny sizes, this is the
deterministic alternative — or regenerate small sizes from `flat-fullbleed.png`.

## Honest judgment vs real Tahoe icons

Credible Tahoe-adjacent, not a system-grade clone. What matches: frosted glass mark
with thickness, restrained top-edge specular, soft contained shadow, clean gradient
backdrop, exact geometry. What differs from Apple's real renders: Apple's glass has
richer multi-layer refraction/dispersion and the system applies dynamic specular per
appearance mode (light/dark/tinted) — a static PNG can't do that, and macOS will add
its own edge lighting on top anyway. The mark reads slightly pink (coral refracting
through the glass) rather than pure white; that is authentic glass behavior but means
small sizes (≤ 32 px) have modest contrast — acceptable, verified legible at 64 px.

## Reproduce

```sh
python3 render_appicon.py all        # flat + pillow fallback
# gpt-image-2 edit step (see prompt above) -> edit-v3.png
python3 finalize_master.py           # masters from edit-v3.png
```
