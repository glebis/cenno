# a2ui widgets

Send a rich `a2ui` payload instead of `input`/`choices` when a built-in kind can't express the question. It works in `ask_user` and in each `ask_sequence` question. Full control inventory: https://github.com/glebis/cenno/blob/main/docs/design/CONTROLS.md

## The envelope

Every payload is the same three A2UI v0.9 messages. Only `COMPONENTS` and `SEED` change per widget:

```json
{
  "title": "…",
  "timeout_s": 120,
  "a2ui": [
    { "version": "v0.9", "createSurface": { "surfaceId": "main", "catalogId": "cenno:catalog/v1" } },
    { "version": "v0.9", "updateComponents": { "surfaceId": "main", "components": COMPONENTS } },
    { "version": "v0.9", "updateDataModel": { "surfaceId": "main", "path": "/", "value": SEED } }
  ]
}
```

- `catalogId` is always `cenno:catalog/v1`. The first component has `"id": "root"` and is a `Column`.
- An answering widget binds `value` to a data path and fires `selectAction` with the same path in `context.value` and `"via": "choice"`. The answer is whatever sits at that path.
- `SEED` is `{}` unless a widget below says otherwise.
- Limits: ≤200 components, ≤256 KiB per payload. Invalid payloads come back as a tool error naming the problem — correct and retry.

The sections below give `COMPONENTS` (and `SEED` where it isn't `{}`).

## Scale — a rating with a custom range or labeled ends

```json
[
  { "id": "root",  "component": "Column", "children": ["col"] },
  { "id": "col",   "component": "Column", "children": ["title", "body", "scale"] },
  { "id": "title", "component": "Text", "text": "Mood today", "variant": "h2" },
  { "id": "body",  "component": "Text", "text": "1 — drained · 5 — great" },
  { "id": "scale", "component": "Scale", "min": 1, "max": 5,
    "minLabel": "drained", "maxLabel": "great", "value": { "path": "/scale" },
    "selectAction": { "event": { "name": "submit-scale",
      "context": { "value": { "path": "/scale" }, "via": "choice" } } } }
]
```

Answer: the number as a string (`"3"`). `body` is optional.

## Slider, DateTimeInput, ChoicePicker

These follow the same `value` + `selectAction` binding as Scale:

- **Slider** — `min`/`max`/`step`; committing fires `selectAction`. Omit `selectAction` and pair it with a **Button** for an explicit confirm.
- **DateTimeInput** — native picker; `enableDate` / `enableTime`.
- **ChoicePicker** — `options: [{label, value}]`. `"displayStyle": "checkbox"` renders radio rows instead of chips (long labels, 4–6 options). `"variant": "multipleSelection"` allows several picks, comma-joined in the answer — so **no option `value` may contain a comma** (the guard rejects it).

## RatingMatrix — several items on one scale

```json
[
  { "id": "root", "component": "Column", "children": ["m"] },
  { "id": "m", "component": "RatingMatrix", "mode": "matrix",
    "rows": [ { "value": "pace", "label": "Pace" },
              { "value": "useful", "label": "Usefulness" },
              { "value": "clarity", "label": "Clarity" } ],
    "min": 1, "max": 5, "minLabel": "low", "maxLabel": "high",
    "value": { "path": "/items" },
    "selectAction": { "event": { "name": "submit-matrix",
      "context": { "value": { "path": "/items" }, "via": "choice" } } } }
]
```

SEED: `{ "items": {} }`.

- Use it instead of N scale prompts — the aligned numeral columns are the point of the control. **Clicking a column numeral rates every row at once**; rows stay individually adjustable afterwards.
- **Nothing answers until `Done`** (rename it with `doneLabel`). Skipping rows is a valid partial answer; `Done` is disabled only when nothing is rated.
- Answer: JSON keyed by row `value`, only the rated rows — `{"answer":"{\"pace\":4,\"useful\":3}","via":"choice"}`. Parse it.
- `mode: "sliders"` gives one continuous slider per row instead.
- Caps: **≤3 rows, ≤7 points** (points apply to `matrix` only). Row values must be distinct — the guard rejects duplicates.

## StepScore — one item on several dimensions plus flags

The the-goal constraint diagnostic is the reference consumer. One panel scores **one** item; to score N items, send N `StepScore` questions through `ask_sequence`. All items in one panel would not fit the 560px panel height.

```json
[
  { "id": "root", "component": "Column", "children": ["s"] },
  { "id": "s", "component": "StepScore", "step": "Review PRs",
    "dimensions": [
      { "value": "sensitivity", "label": "If this step were 2× faster, would throughput rise?", "decisive": true },
      { "value": "wait", "label": "wait before" },
      { "value": "starve", "label": "starves next" },
      { "value": "cap", "label": "capacity gap" },
      { "value": "annoy", "label": "annoyance" } ],
    "flags": [ { "value": "necessary", "label": "necessary" },
               { "value": "policy_gate", "label": "policy gate" } ],
    "value": { "path": "/score" }, "doneLabel": "Next",
    "selectAction": { "event": { "name": "submit-score",
      "context": { "value": { "path": "/score" }, "via": "choice" } } } }
]
```

SEED: `{ "score": {} }` — or a prior score to re-open it. A good `title`: `"Score this step (0 none · 3 high)"`.

- **Every dimension must be scored before Done enables.** `0` means "none" and is a real score, so nothing defaults to 0. Flags are chips that are always on or off, so they never block and always come back as `true`/`false`.
- Answer: JSON with every dimension and flag — `{"answer":"{\"sensitivity\":2,\"wait\":1,\"starve\":0,\"cap\":3,\"annoy\":2,\"necessary\":false,\"policy_gate\":true}","via":"choice"}`. Parse it.
- The `decisive` dimension (at most one) leads full width with its label as the question; the others sit on an aligned grid with short labels (≤ ~12 characters fit the 96px label column).
- `max` defaults to 3 (a 0–3 scale), at most 6.
- Caps: **≤5 dimensions, ≤3 flags**; values distinct across dimensions **and** flags (they share one answer object).

## Table — compare records

Display-only — pair it with a widget that asks the actual question.

```json
[
  { "id": "root", "component": "Column", "children": ["t"] },
  { "id": "t", "component": "Table", "caption": "Plans",
    "columns": [ { "label": "Plan" }, { "label": "Seats", "align": "end" }, { "label": "Cost", "align": "end" } ],
    "rows": [ ["free", 1, "$0"], ["pro", 3, "$20"], ["team", 10, "$60"] ] }
]
```

- Caps: **≤4 columns, ≤12 rows.** A 5th column forces horizontal scroll in a panel that must not scroll; more rows belong in `ask_sequence` or a summary.
- `align: "end"` right-aligns a column **and** switches it to tabular numerals — use it for every number.
- Cells are strings or numbers. A short row is padded with an em dash; a row longer than `columns` is **rejected**, naming the row.
- `header: false` drops the header row for a key/value legend; `dense: true` tightens padding to fit all 12 rows.

## Images

The release build's CSP allows only `img-src 'self' data:`:

- **`data:` URIs work.** The whole payload is capped at 256 KiB, so downscale first (~600px JPEG ≈ 60 KB is plenty at panel size):
  ```bash
  sips -Z 640 -s format jpeg -s formatOptions 60 input.png --out /tmp/small.jpg
  B64=$(base64 -i /tmp/small.jpg)   # → "data:image/jpeg;base64,$B64"
  ```
- **Local absolute paths work** (`/Users/me/out/thumb.png`): the app reads the file and converts it. It must have an image extension (`.png .jpg .jpeg .gif .webp`), be readable, and be **≤512 KiB** — a fixed cap, not settable from a payload. This avoids routing base64 through the agent's context.
- **Remote `http(s):` URLs never render.** cenno does not fetch — download the file and pass the path or a `data:` URI.
- **A failed image does not blank the panel.** The box keeps its size and names the reason (`file not found`, `not an image`, `too large — 1.2 MB, the limit is 512 KB`, `failed to decode`, `remote images are not allowed`). Downscale and re-send on "too large".

### Image — judge one artifact

Images are display-only (no zoom/pan). Always set `"fit": "contain"` (the default `fill` distorts). Use `"variant": "largeFeature"` (160px, full width) when the image **is** the question, `"mediumFeature"` (120px) when it accompanies one; `icon`/`avatar` are decoration only.

```json
[
  { "id": "root",  "component": "Column", "children": ["col"] },
  { "id": "col",   "component": "Column", "children": ["title", "img", "picker"] },
  { "id": "title", "component": "Text", "text": "Keep this thumbnail?", "variant": "h2" },
  { "id": "img",   "component": "Image", "url": "data:image/jpeg;base64,…",
    "description": "generated thumbnail candidate", "fit": "contain", "variant": "largeFeature" },
  { "id": "picker", "component": "ChoicePicker", "options": [
      { "label": "Keep", "value": "keep" },
      { "label": "Regenerate", "value": "regenerate" },
      { "label": "Skip", "value": "skip" } ],
    "value": { "path": "/pick" },
    "selectAction": { "event": { "name": "submit-pick",
      "context": { "value": { "path": "/pick" }, "via": "choice" } } } }
]
```

### ImageChoice — pick among up to 10

Prefer this when candidates are visually distinguishable and there are ≤10. Use one `ask_sequence` panel per candidate instead when each needs a written rationale rather than a pick.

```json
[
  { "id": "root", "component": "Column", "children": ["ic"] },
  { "id": "ic", "component": "ImageChoice", "variant": "mutuallyExclusive",
    "options": [
      { "value": "warm",  "label": "warm",  "image": "data:image/jpeg;base64,…" },
      { "value": "cool",  "label": "cool",  "image": "/Users/me/out/cool.jpg" },
      { "value": "quiet", "label": "quiet", "image": "data:image/jpeg;base64,…" } ],
    "value": { "path": "/pick" },
    "selectAction": { "event": { "name": "submit-image",
      "context": { "value": { "path": "/pick" }, "via": "choice" } } } }
]
```

SEED: `{ "pick": [] }`.

- **≤10 options** — beyond that a tile is too small to judge. For more, send the top 4 as images and the rest as a `ChoicePicker`, or split across `ask_sequence`.
- `variant: "mutuallyExclusive"` (one answer) or `"multipleSelection"` (comma-joined — no commas inside `value`).
- `fit` defaults to `cover`; pass `"contain"` when cropping would ruin the comparison (screenshots, UI diffs). `columns` (1–5) overrides the adaptive layout.
- A tile whose image fails **keeps its place and stays selectable** — a missing preview is not a missing option. `label` renders under the tile.
