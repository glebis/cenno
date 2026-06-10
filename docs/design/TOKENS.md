# cenno design tokens

Extracted from the 2026-06 design pass (Reporter-style, see `frames/final/` and `index.html`).
The PNGs are raster comps — type metrics come from THIS file, not from measuring the images.

Machine source of truth: [`tokens/tokens.json`](../../tokens/tokens.json) (W3C DTCG format,
validated by `npm run validate:tokens`, built to `src/styles/tokens.css` by `npm run tokens`).
Brand — the mark and the typographic solution: [BRAND.md](BRAND.md).

## Palette (one hue per flow)

| Token | Hex | Used for |
|---|---|---|
| `color.flow.mood` | `#FF6250` coral | mood check-ins |
| `color.flow.question` | `#1E4FD8` cobalt | free-text and choice questions |
| `color.flow.ema` | `#0E7C6B` teal | EMA multi-step flows |
| `color.flow.reminder` | `#4A5568` slate | reminders |
| `color.flow.ambient` | `#14171A` ink | expired states, tray inbox/history |
| `color.paper` | `#FAF8F5` | docs/specimen background |
| `color.text` | `#FFFFFF` | primary text on flow colors |
| `color.text.dim` | 60% white | secondary text |
| `color.line` | 40% white | underlines, hairlines, outlines |

## Type scale (SF Pro)

| Token | Size | Role |
|---|---|---|
| `type.question.l` | 44 | fullscreen questions |
| `type.question.m` | 22 | panel questions |
| `type.body` | 17 | answers, choices |
| `type.caption` | 13 | captions, metadata, dB indicator |

## Typography (faces, weights, tracking, leading)

Full rationale in [BRAND.md](BRAND.md).

| Token | Value | Role |
|---|---|---|
| `font.family.default` | system stack (SF Pro on macOS) | the only face |
| `font.weight.question` | 600 | questions, wordmark |
| `font.weight.body` / `font.weight.caption` | 400 | everything else |
| `font.tracking.default` | 0em | questions, body |
| `font.tracking.caption` | 0.08em | uppercase captions, metadata |
| `font.leading.question-l` / `.question-m` | 1.15 / 1.25 | display leading |
| `font.leading.body` / `.caption` | 1.4 / 1.3 | text leading |

Numbers use `font-variant-numeric: tabular-nums` (scale points, dB, timestamps).

## Spacing & radius

- Spacing scale: `8 16 24 40 64`
- Radius: `10` (inputs/cards), `999` (pill chips)
- Tap targets: ≥44px

## Design rules locked during the pass

- One question per screen; the ANSWER is the primary content, not chrome
- Dot pagination, bottom center, fixed position across EMA steps
- Mic affordance = simple circle; recording = pulsing dot, no waveforms
- Expired prompts offer Dismiss only — no late answers (EMA validity)
- Ambient noise indicator: small "ambient NN dB" caption in a corner, opt-in (plan 3)

## Reproducing / re-rolling frames

Every frame is a parameterized prompt + seed: `frames/generate.py`. Per-frame JSON sidecars
in `frames/final/` record the exact prompt and seed used.
