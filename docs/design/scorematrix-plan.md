# Implementation plan — `ScoreMatrix` control

**Status:** ✅ implemented (catalog control + tests + docs landed; app rebuild pending for installed use)
**Owner:** Gleb / agent
**Consumer:** the-goal skill (`~/ai_projects/claude-skills/the-goal`), cenno mode
**References:** [CONTROLS.md](CONTROLS.md) (catalog + recipe), [TOKENS.md](TOKENS.md), [BRAND.md](BRAND.md)

## Why

The-goal skill scores candidate work-steps on five 0–3 dimensions plus two booleans to find the Theory-of-Constraints bottleneck. Today that's N×7 separate panels (chat or `ask_sequence`). A single-panel **matrix** is the appropriate control. cenno is "one question per screen" with no `CheckBox`/`Card`, so the matrix must be **one registered custom component** (the path `Scale`/`Dots` took), rendering its own stacked-card layout internally and emitting the whole matrix in one `submit`.

Must work on **macOS panel and iPhone** (cenno iOS app) — drives the responsive, touch-first layout.

## Scope

| In | Out (later) |
|---|---|
| New `ScoreMatrix` catalog component (view + adapter + register + css) | in-widget "+ add step" (MVP: agent passes a fixed step list) |
| Stacked-card responsive layout (iPhone 1-col, Mac 2-col secondary) | drag-reorder of steps |
| Single `submit-matrix` action returning the full array | custom per-dimension max beyond 0–3 (default 3) |
| Tests, typecheck, docs, rebuild | desugar (`ask_user` simple form) — agent uses raw `a2ui` only |

## Control contract

**Props (schema in `catalog.tsx`):**
| Prop | Type | Default | Note |
|---|---|---|---|
| `steps` | `string[]` | — | row labels (pipeline steps) |
| `dimensions` | `{id,label,max}[]` | sens/wait/starve/cap/annoy, max 3 | `sensitivity` is decisive (rendered prominent) |
| `flags` | `{id,label}[]` | necessary, policy_gate | rendered as `ChoicePicker` multi chips |
| `legend` | `string` | "0 none · 3 high" | shown once at top |
| `value` | `{path}` | — | bound data-model path holding the matrix |
| `submitAction` | action | — | fired by the Score button |

**Data model shape (bound at `value.path`):**
```json
{ "steps": [
  {"name":"Audience","sensitivity":3,"wait":3,"starve":3,"cap":3,"annoy":2,"necessary":false,"policy_gate":false}
] }
```
Defaults: ordinals `0`, flags `false`.

**Answer (action contract — must start with `submit`, carry `{value, via}`):**
`submit-matrix` → `{ value: <steps array>, via: "choice" }`. Agent maps keys to the scorer's
(`sensitivity→throughput_sensitivity`, `wait→wait_before`, `starve→downstream_starvation`,
`cap→capacity_gap`, `annoy→annoyance`, `necessary→necessary_condition`, `policy_gate→policy_gate`)
and pipes to `score_constraints.py`.

## Layout

Stacked cards, one per step, vertical scroll, sticky Score button.

**iPhone (portrait, secondary dims 1-col):**
```
The Goal · score the constraint        0 none → 3 high
╭─ Audience ─────────────────╮
│ Sensitivity · if 2×→T? ★   │
│   ⓪   ①   ②   ⬤          │
│ wait      ⓪ ① ② ⬤        │
│ starve    ⓪ ① ② ⬤        │
│ capacity  ⓪ ① ② ⬤        │
│ annoyance ⓪ ① ⬤ ③        │
│ ⟨necessary⟩  ⟨policy gate⟩ │
╰────────────────────────────╯
            [ Score ▶ ]
```
**Mac panel (≥420px, secondary dims 2-col):** same card, `wait/capacity` and `starve/annoy`
paired side by side; sensitivity always full-width.

| Rule | Behavior |
|---|---|
| sensitivity | full-width, accent (★) — decisive |
| secondary 4 dims | 1-col `< 360px`, else 2-col |
| targets | every 0–3 cell + chip ≥ 44px (styling contract) |
| scroll | cards scroll; Score button sticky bottom |
| a11y | each Scale named "`<step>` — `<dimension>`" |
| styling | semantic theme + token vars only; transparent background |

## File-by-file

| # | File | Change |
|---|---|---|
| 1 | `src/a2ui/views.test.tsx` | TDD: renders one card per `steps[]`; sensitivity prominent; tapping a Scale updates draft; toggling a flag chip flips it; Score fires `submit-matrix` with full array; re-syncs when bound value changes. Extend `cennoCatalog` registration test's expected list with `ScoreMatrix`. Watch fail. |
| 2 | `src/a2ui/views.tsx` | `ScoreMatrixView` — plain React; internal draft state seeded from bound value + `useEffect` re-sync (mirror `SliderView`/`TextFieldView`); composes `ScaleView`/chips/`Text`; emits via callback. |
| 3 | `src/a2ui/catalog.tsx` | custom API (header comment: composite multi-value control, no standard single-value API fits) + `.describe()` on every prop; `createComponentImplementation`. |
| 4 | `src/a2ui/catalog.tsx` (`cennoCatalog`) | register `ScoreMatrix` (without this the renderer drops it). |
| 5 | `src/a2ui/catalog.css` | `cenno-scorematrix` block — responsive grid, 44px targets, theme/token vars. |
| 6 | `docs/design/CONTROLS.md` + `the-goal/references/cenno-mode.md` | add `ScoreMatrix` to the inventory + show the raw `a2ui` payload the agent sends. |

## Test plan (acceptance)

- [ ] renders N cards for N `steps`
- [ ] sensitivity row visually prominent and full-width on both breakpoints
- [ ] secondary dims reflow 1-col ↔ 2-col at 360px
- [ ] each ordinal target ≥ 44px; reachable on iPhone portrait
- [ ] flag chips toggle independently; multi-select
- [ ] Score fires exactly one `submit-matrix` with the complete array (defaults filled)
- [ ] bound-value change re-syncs the view (host echo)
- [ ] `npx vitest run` green · `npx tsc --noEmit` clean · `npm run typecheck:tests` clean

## Rollout

1. Land control + tests on a branch; verify suite.
2. Rebuild app (`/Applications/cenno.app` must contain the new catalog) for installed use.
3. Update `the-goal` cenno-mode to prefer `ScoreMatrix` when present.
4. **Fallback stays:** if the installed cenno lacks `ScoreMatrix` (older build), the-goal falls back to `ask_sequence` 0–3 choices — already shipped, no build needed. Never block on the widget.

## Risks

| Risk | Mitigation |
|---|---|
| Many bound inputs + single whole-model submit untested in cenno | views seed/echo via `useEffect` per house rules; the control owns its own draft state and emits once on Score |
| Component/size caps (≤200 components, ≤256 KiB) | one custom component renders the whole matrix internally — well under caps regardless of step count |
| Over-build vs `ask_sequence` | keep `ask_sequence` as the zero-build path; ScoreMatrix is the upgrade, not a dependency |
