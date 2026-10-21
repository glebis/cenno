# cenno UI controls — the A2UI catalog and how to extend it

The prompt UI the agent renders is built from a fixed set of controls — the
**cenno catalog** (`cenno:catalog/v1`). This doc is the inventory of those
controls and the recipe for adding a new one.

Code source of truth: [`src/a2ui/catalog.tsx`](../../src/a2ui/catalog.tsx)
(schemas + registration) and [`src/a2ui/views.tsx`](../../src/a2ui/views.tsx)
(rendering). Visual language: [TOKENS.md](TOKENS.md), [BRAND.md](BRAND.md).

## Architecture: three layers + a guard

```
agent (MCP ask_user) ──┬── simple form ──► desugar.ts ──► A2UI messages
                       └── raw `a2ui` payload ───────────► A2UI messages
                                                                │
src-tauri/src/a2ui_guard.rs  ◄── validates raw payloads ────────┤
                                                                ▼
catalog.tsx  — A2UI adapters: zod schema ⇄ view props   (cenno:catalog/v1)
                                                                ▼
views.tsx    — plain React, no A2UI imports; styled by catalog.css
```

1. **`src/a2ui/views.tsx`** — plain React components. No A2UI imports, tested
   directly in `views.test.tsx`.
2. **`src/a2ui/catalog.tsx`** — thin adapters created with
   `createComponentImplementation(Api, ({ props }) => <View …/>)`, registered
   in the `cennoCatalog` array at the bottom of the file. Registration is what
   makes a control renderable.
3. **`src/a2ui/desugar.ts`** — maps the simple `ask_user` prompt forms onto
   catalog components by name. Only touched when the *simple* form should
   emit the new control; raw `a2ui` payloads reach the catalog directly.
4. **`src-tauri/src/a2ui_guard.rs`** — Rust-side boundary validation of raw
   payloads (envelope shape, catalog id, size caps, and per-component rules
   for the props whose invalid values would render *nothing* rather than
   something wrong: image URLs, caps that force scrolling, enums). It does
   **not** whitelist component names — an unknown name is left to the
   renderer — but a control with such a prop DOES need a guard arm.

## House rules

- **Standard API first.** Reuse a `@a2ui/web_core` basic-catalog API
  (`TextApi`, `ButtonApi`, `SliderApi`, …) whenever one fits, extending its
  schema with `.extend({...})` for cenno-specific props. A custom API
  (`Scale`, `Dots`) is a last resort, and its header comment must say why no
  standard API fit.
- **Action contract.** Every user-completing interaction fires an action
  whose name starts with `submit`, with `{ value, via }` in the action
  context (`via`: `"text"` for typed answers, `"choice"` for tap-to-answer).
  Interactive controls carry an *optional* action prop (`selectAction` /
  `submitAction`) — the agent includes it for one-gesture answer flows or
  omits it and pairs the control with a Button.
- **Styling contract.** `catalog.css` consumes ONLY semantic theme vars
  (`--cenno-text`, `--cenno-text-dim`, `--cenno-line`, `--cenno-surface`) and
  token vars (`--cenno-type-*`, `--cenno-space-*`, `--cenno-radius-*`).
  Backgrounds stay transparent (the panel root owns the surface); tap targets
  ≥ 44px via padding/min sizes, never font inflation.
- **Bound props are dynamic.** Schema values like `DynamicNumberSchema` may
  arrive as data-model bindings, so adapters narrow with `typeof` checks
  before passing them to views.

## Catalog inventory

| Component | API | cenno extensions | View |
|---|---|---|---|
| `Text` | standard `TextApi` | — (variant → type role) | `TextView` |
| `Row` / `Column` | standard | — | flex divs |
| `Button` | standard `ButtonApi` | — (borderless → secondary) | `ButtonView` |
| `TextField` | standard, extended | `voice`, `submitAction` (Enter) | `TextFieldView` |
| `ChoicePicker` | standard, extended | `selectAction` (tap); `displayStyle: "checkbox"` renders the radio/checkbox list instead of chips | `ChipsView` / `CheckListView` |
| `Slider` | standard `SliderApi`, extended | `minLabel`, `maxLabel`, `selectAction` (thumb release / Enter) | `SliderView` |
| `DateTimeInput` | standard, extended | `submitAction` (Enter) | `DateTimeView` |
| `Image` | standard `ImageApi` | — (display-only; `data:` URI or local path, three states) | `ImageView` |
| `Scale` | custom (discrete targets; SliderApi is continuous) | — | `ScaleView` |
| `Dots` | custom (no standard pagination API) | — | `DotsView` |
| `Table` | custom (no table component exists upstream in any release) | `header`, `columns[{label,align}]`, `dense` | `TableView` |
| `RatingMatrix` | custom (several bindings in one control) | `rows`, `mode`, `min`/`max`, `minLabel`/`maxLabel` | `MatrixView` |
| `ImageChoice` | custom (`ChoicePicker` options are strictly `{label,value}`) | `options[{value,label?,image}]`, `variant`, `fit`, `columns` | `ImageChoiceView` |

`Slider` vs `Scale`: Scale is the EMA 1..N numeral row — discrete tap
targets, each a 44px circle. Slider is a continuous range for "how much"
questions where granularity matters more than labeled steps.

`RatingMatrix` vs two `Scale`s: the matrix exists because evaluating N items
means the numeral columns must LINE UP across rows — that alignment is the
whole control. It caps at 3 rows × 7 points because `resize_panel` clamps the
panel to 240–560px; more items belong in `ask_sequence`.

`Table` vs a `Column` of `Row`s: the table exists for the header rule, the
per-column alignment (tabular numerals right-aligned), and the row/column
caps. It is display-only and caps at 4 columns × 12 rows.

`DateTimeInput` notes: `enableDate`/`enableTime` map onto native
`date`/`time`/`datetime-local` inputs (system picker UI); values are the
input's native ISO-ish strings ("2026-06-15", "14:30",
"2026-06-15T14:30"), passed to the agent verbatim. `Image` notes: without
a `description` the image is presentational (hidden from assistive tech).
The release build's CSP allows `img-src 'self' data:` only, so a `data:` URI
and a local file path (read through the `load_image` command, 512 KiB cap)
both render while a remote URL is rejected with a named reason in the panel.
A failed image keeps its box (and, in `ImageChoice`, stays selectable) so the
panel never jumps and a missing preview is never mistaken for a missing option.

### Considered and not added (and why)

- `CheckBox` — a single boolean is the confirm flow (Yes/No buttons), and
  multi-select is ChoicePicker's `multipleSelection` chips (or, for images,
  `ImageChoice` with `variant: "multipleSelection"`).
- `Icon`, `Divider` — decorative; the panel is one question per screen and
  Column gap owns spacing.
- `Video`, `AudioPlayer` — heavy media is off-mission for glanceable,
  time-sensitive panels, and CSP-constrained in bundled builds.
- `List`, `Card`, `Tabs`, `Modal` — the panel root owns the surface; one
  question per screen rules out nested navigation chrome.

Revisit any of these when a real prompt flow needs them — the recipe below
makes each a small, mechanical addition.

### Known limitation: schemas are advisory at runtime

The renderer does not zod-validate component props on ingest. A payload
missing a schema-required prop (e.g. a `Slider` without `max`) still
renders, because adapters defensively narrow with `typeof` checks and fall
back to defaults (verified live, 2026-06-10: a max-less slider rendered as
0–10 instead of triggering the PromptPanel fallback). The `.describe()`
strings tell the agent what to send; the adapter defaults decide what a
malformed payload silently becomes. Keep adapter defaults sensible for
that reason.

What the RENDERER drops silently, the GUARD must reject loudly. A component
the catalog cannot parse is discarded by the webview without an error, so
`src-tauri/src/a2ui_guard.rs` is authoritative for the cases where silence
would mislead: image URLs/`data:` shapes, `ChoicePicker.displayStyle`,
`Table` column/row caps, `RatingMatrix` row/point caps, and `ImageChoice`
option count, enum props, and comma-containing values in
`multipleSelection`. When adding a prop whose invalid value would render
nothing (rather than something wrong), add it to the guard too — otherwise
the agent gets a blank panel and no reason.

## Recipe: adding a control

Worked example: the `Slider` (commit `git log --follow src/a2ui/views.tsx`).

1. **Tests first** (`src/a2ui/views.test.tsx`). Specify rendering,
   accessibility name, and the interaction contract (e.g. "reports value
   changes while dragging without committing" / "commits on pointer
   release"). Extend the `cennoCatalog` registration test's expected
   component list. Watch them fail.
2. **View** (`src/a2ui/views.tsx`). Plain React, simple props plus
   callbacks. Seed internal draft state from the bound value and re-sync via
   `useEffect` (see `TextFieldView` / `SliderView`) so the control stays
   usable when the host doesn't echo changes back.
3. **API + adapter** (`src/a2ui/catalog.tsx`). Standard API if one fits,
   `.extend()` for cenno props, then `createComponentImplementation`.
   Describe every extension prop with `.describe()` — those strings are what
   the agent reads to use the control correctly.
4. **Register** it in the `cennoCatalog` array. Without this it renders
   nothing (the renderer silently drops unknown components).
5. **Style** in `src/a2ui/catalog.css` under a `cenno-<name>` block, theme
   and token vars only.
6. **Optionally extend `desugar.ts`** if a simple `ask_user` form should emit
   it, keeping the `submit*` / `{ value, via }` action contract.
7. **Verify**: `npx vitest run`, `npx tsc --noEmit`,
   `npm run typecheck:tests`. Then update the inventory table above.

The agent only uses what it knows exists: if the control should appear in
agent-authored raw payloads, also document it wherever the agent's prompt or
tool description enumerates the catalog.
