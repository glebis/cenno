# cenno brand

*Un cenno* — a nod, a beckon, the small sign. The brand is the same idea
three times: a mark that beckons, type that asks one question, color that
names the flow. Everything else is absence.

Token source of truth: [`tokens/tokens.json`](../../tokens/tokens.json) (W3C DTCG)
→ built to `src/styles/tokens.css` → values documented in [TOKENS.md](TOKENS.md).
Validated by `npm run validate:tokens`.

## The mark — arc over dot

![cenno mark](brand/cenno-mark.svg)

An arc beckoning over a dot. The app summons; the person answers. Two shapes,
nothing else — it survives 22px in a menu bar, which is where it lives.

- **Canonical source:** the tray icon. Generator:
  [`src-tauri/icons/tray/tray_template_icon.py`](../../src-tauri/icons/tray/tray_template_icon.py)
  (22px space: arc center 11,9 / r 5.5 / stroke 2.5 / sweep 195°–345°;
  dot center 11,16.25 / r 2.75).
- **Vector:** [`brand/cenno-mark.svg`](brand/cenno-mark.svg) — same geometry,
  draws in `currentColor`.
- **Machine-readable geometry:** `tokens/tokens.json` →
  `brand.$extensions["app.cenno.mark"]`.

### Mark usage

- **System contexts** (tray, menus, notifications): template/monochrome only —
  black on transparent, let macOS invert it.
- **Brand contexts** (docs, site, splash): the mark may take any single
  `color.flow` hue, or white on a flow surface.
- Never multi-color, never outlined dot, never redrawn — regenerate from the
  generator or reuse the SVG.
- Clear space: one arc-radius (5.5 units at 22px) on all sides.

## The lockup — the dot joins the word

The canonical lockup deconstructs the mark: the dot drops onto the text
baseline and becomes the bullet of **cenno**; the arc stays above it,
centered. The app summons (arc), the person answers (dot) — and the answer
is the name.

Everything is parametric in **D**, the dot diameter, which equals the
wordmark x-height. Arc and dot keep their canonical proportions and are
rendered from the SVG geometry, never redrawn.

| Relation | Value |
|---|---|
| dot diameter D | wordmark x-height (= 5.5 canonical units) |
| dot position | on the baseline, first "character" of the word |
| gap dot → word | 1.0 D |
| arc outer width | 2.4545 D (canonical 13.5u/5.5u), centered over the dot |
| gap arc → dot | 1.0 D |
| clear space | 1.0 D on all sides (= one arc radius) |

Wordmark: **cenno**, lowercase, SF Pro (`font.family.default`) weight 600.
Generator: [`brand/renders/make_inline_lockup.py`](brand/renders/make_inline_lockup.py).
Primary colorway: white on `color.flow.mood`; also black on transparent,
white on `color.flow.ambient`, `color.flow.question` on `color.paper`.

## The typographic solution

One face. SF Pro through the system stack (`font.family.default`). Hierarchy
comes from size, weight and color — never from a second family.

| Role | Size | Weight | Leading | Tracking | Tokens |
|---|---|---|---|---|---|
| Question L (fullscreen) | 44 | 600 | 1.15 | 0 | `type.question.l` · `font.weight.question` · `font.leading.question-l` |
| Question M (panel) | 22 | 600 | 1.25 | 0 | `type.question.m` · `font.weight.question` · `font.leading.question-m` |
| Body (answers, choices) | 17 | 400 | 1.4 | 0 | `type.body` · `font.weight.body` · `font.leading.body` |
| Caption (metadata, dB) | 13 | 400 | 1.3 | 0.08em + UPPERCASE | `type.caption` · `font.weight.caption` · `font.tracking.caption` |

Numbers — scale points, dB readouts, timestamps — set `font-variant-numeric:
tabular-nums` so they hold still.

The wordmark is not lettering: it is the word **cenno**, lowercase, caption
treatment (600 where it must carry weight, `font.tracking.caption`, uppercase
optional in caption rows). The mark does the identifying; the word just names it.

## Large-format assets

Exact renders, regenerated from the SVG/generator — never redrawn
(`brand/renders/`):

- `cenno-mark-2048-{black,question,mood,ema,reminder}.png` — mark at 2048px,
  one per permitted hue, transparent background.
- `cenno-mark-2048-white-on-ambient.png` — white mark on `color.flow.ambient`.
- `cenno-lockup-inline-{white-on-mood,black,white-on-ambient,question-on-paper}.png`
  — **the canonical lockup** (dot on the baseline, arc above; see "The lockup"),
  ~6900×3100px. Generator: `renders/make_inline_lockup.py`.
- `cenno-lockup-{black,question-on-paper,white-on-ambient}.png` — legacy
  side-by-side lockup: mark + lowercase **cenno** (SF Pro 600), ~5700×2100px.
- `cenno-flow-grid-2048.png` — white mark tiled over the four flow hues.

AI stylizations for brand contexts only — splash, poster, icon explorations
(`brand/generated/`, GPT Image 2, prompts in the `.json` sidecars):
`splash-ambient`, `poster-paper`, `app-icon`, `flow-grid`. These are mood
pieces; any shipped mark must come from the renders above.

## Voice

Reporter-minimal. One question per screen; the answer is the content, not the
chrome. If a screen needs decoration, the question is wrong.
