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
- Clear space: one dot-diameter (5.5 units at 22px) on all sides.

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

## Voice

Reporter-minimal. One question per screen; the answer is the content, not the
chrome. If a screen needs decoration, the question is wrong.
