# Rich panel widgets — tables, choice presentations, multi-item evaluation, images, image choice

Date: 2026-07-18
Status: draft, awaiting review

## Problem

`cenno:catalog/v1` cannot express five things agents routinely need:

1. **Tabular data.** No Table component exists in A2UI v0.9 (verified against
   `@a2ui/web_core` 0.10.7 and 0.14.0 — the stock catalog is Text, Image,
   Icon, Video, AudioPlayer, Row, Column, List, Card, Tabs, Modal, Divider,
   Button, TextField, CheckBox, ChoicePicker, Slider, DateTimeInput). An
   agent comparing plans, test runs, or candidates has to flatten a table into
   prose.
2. **Radio buttons / checkboxes.** `ChoicePickerApi` carries
   `displayStyle: "checkbox" | "chips"` and `filterable`, but
   `CennoChoicePicker` (`src/a2ui/catalog.tsx:213`) ignores both and always
   renders `ChipsView`. A stacked radio list is unavailable, and multi-select
   rendered as filled pills reads as a pressed button rather than a state.
3. **Evaluating several items at once.** The user asked for "multiple lines,
   each with slider / some form of radio buttons — so that I can evaluate
   multiple items". Today the only path is N separate prompts
   (`ask_sequence`), which shows one item at a time.
4. **Images that reliably load.** `ImageView` (`src/a2ui/views.tsx:404`) is a
   bare `<img>` with no load checking. The release CSP
   (`src-tauri/tauri.conf.json:26`) is `img-src 'self' data:`, so remote URLs
   and local file paths render as a silent blank box, and `a2ui_guard.rs`
   validates nothing about image URLs.
5. **Choosing between images.** The user asked for images to also work as a
   choice — "select from up to 10 images, adaptive layout for each number".
   `ChoicePicker` options are strictly `{label, value}` in every published
   release of `@a2ui/web_core`, so there is no place to put a picture, and the
   chips layout cannot show one anyway.

## Decisions

| Question | Decision | Rationale |
|---|---|---|
| Remote (`https:`) images | **Not allowed** | The panel is a local, non-activating surface. Fetching an agent-supplied URL turns every prompt into a network client and a privacy leak (the panel would beacon to an arbitrary host). AGENTS.md permits network only for the user-initiated updater and the one-time voice model download. |
| Local file images | **Rust command → `data:` URI** | Tauri's asset protocol would need a filesystem scope in capabilities, and any useful scope exposes more than images to the webview. A command takes one path, caps size, sniffs type. |
| Choice presentations | **All three: radios, checkboxes, chips** | Chips stay the default when `displayStyle` is omitted, so no existing payload changes appearance. |
| Multi-item widget | **One component, `mode: "matrix" \| "sliders"`** | Both shapes are the same data (a set of keyed values); two components would duplicate validation and styling. |
| Multi-item answer | **JSON object string keyed by row value** | `{"pace":4,"usefulness":3}` is self-describing and parses in the agent's language without a schema fetch. Row `value` is returned; `label` is display-only. |
| Table | **Display-only, literal rows** | Input cells multiply the guard's job (cell → binding paths) and every answer must flatten to one string anyway. Largest value for the smallest validation surface. |
| Table column cap | **Hard cap at 4** | Panels are ~320–420px. A 5th column forces horizontal scroll in a floating panel that must not be scrolled. The guard rejects with an actionable error. |
| Image error verbosity | **Name the reason in the panel** | A silent blank is the bug being fixed; "image failed to load" alone doesn't tell the agent what to change. The panel line is caption-size and dim — it does not compete with the question. |
| Combined panel height | **Matrix capped at 3 rows** | `resize_panel` clamps to 240–560px. A table plus a longer matrix exceeds that and scrolls. More items belong in `ask_sequence`. |
| Images as a choice | **New `ImageChoice` component**, not an extension of `ChoicePicker` | An option whose type depends on its content is a renderer nobody can predict. `ChoicePicker` options are strictly `{label, value}` in every release, and keeping them so means a shipped payload can't silently change shape. |
| `ImageChoice` variants | **Both** `mutuallyExclusive` + `multipleSelection` | "Keep which of these?" is the real multi-image case, and the selection-mark shape already distinguishes the two (round vs square). |
| `ImageChoice` geometry | **Square tiles, `cover` by default, `fit:"contain"` opt-in** | Cover keeps an even grid with no letterbox bars at every count; `contain` is one enum away for agents judging screenshots or UI diffs where cropping destroys the comparison. |
| `ImageChoice` count | **Hard cap at 10** | Below ~58px a tile stops being a thumb you can judge. Ten also bounds the payload: ~25 KB per tile inside the existing 256 KiB envelope. |

## Components

### 1. `CheckListView` — a new view behind the existing `ChoicePicker`

This is not a new catalog component: `ChoicePicker` stays the component name
and the schema is unchanged. What changes is that `CennoChoicePicker` stops
discarding `displayStyle` and renders one of three presentations from
the new `CheckListView`. Selected values remain
`string[]` in the data model; single-select still unwraps to one element in
`src/PromptPanel.tsx:151`, so the answer contract is unchanged.

No new schema. The adapter reads the stock `displayStyle` and `variant`:

| `displayStyle` | `variant` | rendering |
|---|---|---|
| `"chips"` (cenno default) | either | existing `ChipsView` pills |
| `"checkbox"` | `"mutuallyExclusive"` | radio list, `role="radiogroup"` + `role="radio"` + `aria-checked` |
| `"checkbox"` | `"multipleSelection"` | checkbox list, `role="group"` + `role="checkbox"` + `aria-checked` |
| — | `"words"` (cenno extension) | existing bare-word row, unchanged |

The marks are drawn in CSS (a border-radius circle with an inset filled dot;
a 3px-radius square with an inset check) rather than native
`<input type=radio>`, because the panel's type and line tokens must survive —
native controls ignore `--cenno-type-*` and render at OS sizes on a colored
surface. Keyboard and VoiceOver support comes from the ARIA roles plus
`tabIndex` and Space/Enter handling, tested in `views.test.tsx`.

The default is `"chips"` when `displayStyle` is absent. Upstream defaults it
to `"checkbox"`, but cenno's identity is the chip and every shipped payload
omits the field — adopting the upstream default would silently restyle every
existing prompt.

### 2. `RatingMatrix` — custom (new)

One component, `mode` selects the presentation. Both modes render from a
single view, `MatrixView`, which switches on `mode` — the two presentations
share the row model, the caps, and the answer encoding.

```
{ id: "items", component: "RatingMatrix",
  mode: "matrix" | "sliders",
  rows: [ { value: "pace", label: "Pace" }, … ],
  min, max, step,                        // matrix: 1..max points; sliders: range
  minLabel, maxLabel,
  value: { path: "/items" },
  selectAction: { event: { name: "submit-items",
                           context: { value: { path: "/items" }, via: "choice" } } } }
```

- `mode: "matrix"` — a grid: first column is the row label, then one radio
  point per value from `min` to `max`. One tap answers that row. The dot shows
  its numeral once selected (and on hover), so the scale is legible without
  counting.
- `mode: "sliders"` — one label + numeric readout + slider per row. Commits on
  thumb release (pointer) or Enter (keyboard), matching `SliderView`'s existing
  contract in `src/a2ui/views.tsx:290`.

Rows carry `value` (the returned key) and `label` (display only). A row with a
label but no value derives its key by slugifying the label, so the simplest
payload still works.

`value` binds to the data model as an object (`/items`), not an array: each row
writes `path + "/" + row.value`. That keeps the mapping positional-free — row
order can change without corrupting answers.

`rows` length is capped at **3** and `max - min + 1` at **7**, both enforced in
`a2ui_guard.rs` with errors naming the limit and pointing at `ask_sequence`.
Over either cap the guard rejects before a prompt is registered, so the failure
is an actionable tool error rather than a clipped panel.

### 3. `Table` — custom (new)

```
{ id: "plan", component: "Table",
  header: true,
  columns: [ { label: "Plan" }, { label: "/mo", align: "end" },
             { label: "Seats", align: "end" } ],
  rows: [ ["Starter", "9", "1"], ["Team", "29", "5"] ],
  dense: false }
```

- Display only. `rows` is an array of arrays of strings; numbers may be sent as
  JSON numbers and are stringified on render.
- `columns[].align` accepts `"start"` (default) or `"end"`; the end alignment
  uses tabular numerals so digits line up. A per-column alignment array is the
  reason numbers stop wobbling — a single table-wide setting cannot express
  "labels left, money right".
- `header: false` renders the body only.
- `dense: true` halves vertical cell padding.
- Hairlines only — bottom borders on each row, no outer border, no zebra
  striping, no background. It sits on the flow color like every other
  component.
- Header cells use the caption size, dim color, uppercase, and letter spacing,
  matching the existing `ScaleView` end labels.
- Caps: **4 columns**, **12 rows** — both in `a2ui_guard.rs`.

### 4. `Image` — existing component, hardened

Keeps the stock `ImageApi` (`url`, `description`, `fit`, `variant`). Adds
behaviour, no schema change beyond what the guard accepts.

**Accepted `url` forms:**

| form | handling |
|---|---|
| `data:image/<png\|jpeg\|gif\|webp>;base64,…` | used directly; scheme and mime prefix validated |
| `https:` / `http:` | rejected with `remote images are not allowed — embed it as a data: URI or pass a local file path` |
| absolute path (`/Users/x/a.png`), `~/a.png` | resolved by the new Rust command into a `data:` URI |

**New Rust command** `load_image(app, path, max_bytes) -> Result<String, String>`
in a new `src-tauri/src/image.rs`, registered in the
`tauri::generate_handler!` list (`src-tauri/src/lib.rs:965`):

1. Expand a leading `~` to `$HOME`, then reject anything still not absolute —
   a relative path is ambiguous (relative to what, the app bundle?) and is
   rejected with a copy that says so.
2. `metadata()` the path (which follows symlinks), branch on `io::ErrorKind` so
   a permission error does not masquerade as "file not found", then require
   `is_file()` — this is what keeps `read` away from directories, FIFOs and
   device nodes.
3. Require an extension in `png/jpeg/jpg/gif/webp` *before* any read, so an
   unrelated file is rejected on its name alone.
4. `metadata().len()` against `max_bytes` (default 512 KiB, clamped to
   1–4096 KiB) — and the read itself is capped with `File::take(max_bytes + 1)`
   and re-checked, so a file that grows between stat and read cannot exceed the
   cap.
5. Sniff the magic number and reject if it disagrees with the extension
   (`PNG` signature, `\xFF\xD8\xFF`, `GIF8`, `RIFF…WEBP`).
6. Return `data:<mime>;base64,<…>`.

Every failure is a specific `Err` string, reason first and path last so the
reason survives truncation at caption size in a 320–420px panel: `file not
found: <path> — the assistant sent a path that does not exist`,
`not an image (extension ".txt"): <path> — cenno only loads png, jpg, jpeg,
gif or webp`, `too large (1.4 MB, max 512 KB): <path>`, `contents are not a
valid PNG: <path>`. These strings are what the panel shows.

**Frontend helper** `resolveImageUrl(url)` in a new `src/a2ui/image.ts`:
classifies the URL and either returns a `data:` URI, throws a named error for a
rejected scheme, or awaits `invoke("load_image", …)`. Results are memoised in a
small `Map` keyed by path so a table of thumbnails does not re-read a file on
every re-render.

**`ImageView` states**, all with a fixed-size box matching the `variant` cap so
the panel never jumps:

- *loading* — shimmer tint, `prefers-reduced-motion` gets a static tint
- *ok* — the `<img>`, with `onLoad`/`onError` wired so a decode failure in the
  webview is caught too (a valid `data:` URI can still fail to decode)
- *error* — an icon glyph plus a caption-size dim line reading
  `image failed to load — <reason>`

The view never throws: a broken image degrades to the error line and the prompt
stays answerable.

### 5. `ImageChoice` — custom (new)

Select from up to 10 images. A deliberate new component rather than an extra
field on `ChoicePicker` (see Decisions): the shape of the option decides the
shape of the renderer.

**Schema:**

```jsonc
{
  "component": "ImageChoice",
  "id": "pick",
  "options": [
    { "value": "v2", "label": "v2 · quiet", "image": "data:image/png;base64,…" }
  ],
  "variant": "mutuallyExclusive",   // | "multipleSelection"
  "fit": "cover",                    // | "contain"
  "columns": 4                       // optional override; default is the adaptive table
}
```

`options[]` is `{value, label?, image}` — `value` is returned, `label` is
display-only and sits **under** the tile, never over it. `image` accepts
exactly what hardened `Image` accepts (`data:` URI or a local path), through
the same `resolveImageUrl` helper and the same `load_image` command.

**Adaptive layout.** Column count is a lookup, not `auto-fill`:

| n | columns | rows | tile @340px panel |
|---|---|---|---|
| 1 | 1 | 1 | ~300px |
| 2 | 2 | 1 | ~155px |
| 3 | 3 | 1 | ~100px |
| 4 | 2 | 2 | ~155px |
| 5 | 3 | 2 | ~100px |
| 6 | 3 | 2 | ~100px |
| 7 | 4 | 2 | ~75px |
| 8 | 4 | 2 | ~75px |
| 9 | 3 | 3 | ~100px |
| 10 | 5 | 2 | ~58px |

The rule behind the table: keep tiles ≥ ~58px and never leave a final row with
a single orphan tile. This is why 5 is 3+2 rather than 4+1, 7 is 4+3 rather
than 3+3+1, and 9 is 3×3 rather than 4+4+1 — 3×3 also keeps tiles bigger than
a 5-column layout would. Ten tiles at 5 columns is ~140px of tile height,
which fits the 560px clamp next to a question and labels.

**Selection.** Top-right mark: a filled circle for `mutuallyExclusive`, a filled
square for `multipleSelection` — the same visual language as radios vs
checkboxes, so the mark shape alone says which answer you will get. Selected
tile gets a solid border in `--cenno-text`; unselected is `1px transparent` so
the grid never shifts on select.

**Failure states.** A tile whose image fails keeps its box (dashed
`--cenno-line` border, caption-size dim reason) and **stays selectable** — a
missing preview is not a missing option. This matters for the agent-judging
-images case: a 404 preview must not silently delete a candidate from the
comparison.

**Answer.** `mutuallyExclusive` → the one `value`, unwrapped by the existing
`src/PromptPanel.tsx:151` path. `multipleSelection` → values joined with `,`
through the same array flattening `ChoicePicker` already relies on. `via` is
`"choice"`. No change to `protocol.rs`.

## Boundary validation (`src-tauri/src/a2ui_guard.rs`)

The guard grows from "is this a well-formed envelope" to "could this render".
New checks, each with an error naming the limit and the fix:

- `Table`: `columns.len() <= 4`, `rows.len() <= 12`, every row length equals
  `columns.len()`, `columns[].align` in `{"start","end"}`.
- `RatingMatrix`: `rows.len() <= 3`, `max - min + 1 <= 7`, `mode` in
  `{"matrix","sliders"}`, every row has a non-empty `label` or `value`.
- `Image`: `url` is a string; if it starts with a scheme, the scheme is in
  `{"data"}`; `data:` URIs must match
  `^data:image/(png|jpeg|gif|webp);base64,`.
- `ChoicePicker`: `displayStyle` in `{"chips","checkbox"}` when present.
- `ImageChoice`: `options.len() in 1..=10`, every option has a non-empty
  `value` and an `image` that passes the same scheme/data-URI check as `Image`,
  `variant` in `{"mutuallyExclusive","multipleSelection"}`, `fit` in
  `{"cover","contain"}` when present, `columns` in `1..=5` when present.

Validation walks `updateComponents.components`, keying on the `component`
string. Components not in the catalog are left to the renderer, as today.

The existing 200-component and 256 KiB caps stay. A 4×12 table is 1 component,
so tables cost almost nothing against them; a matrix is likewise one component.

## Files touched

| file | change |
|---|---|
| `src/a2ui/views.tsx` | add `CheckListView`, `MatrixView`, `TableView`, `ImageChoiceView`; harden `ImageView` |
| `src/a2ui/views.test.tsx` | tests for each new view + image three-state behaviour + adaptive column lookup |
| `src/a2ui/catalog.tsx` | honour `displayStyle`; add `TableApi`, `RatingMatrixApi`, `ImageChoiceApi` + adapters |
| `src/a2ui/catalog.css` | styles for the list, matrix, slider rows, table, image states |
| `src/a2ui/image.ts` | new — URL classification + `load_image` invocation + memo |
| `src-tauri/src/image.rs` | new — the loader command |
| `src-tauri/src/lib.rs` | register `image::load_image` |
| `src-tauri/src/a2ui_guard.rs` | new structural checks + tests |
| `src-tauri/src/protocol.rs` | no change (input kinds unchanged) |
| `skills/cenno/SKILL.md` | document the three new widgets, the image rules, the caps |
| `docs/design/CONTROLS.md` | record the new presentations |

## Testing

- `npx vitest run` — view-level: radio semantics (`role="radio"` +
  `aria-checked`), checkbox toggle, matrix row selection reporting a keyed
  object, slider-row commit on release/Enter, table alignment classes, image
  loading/ok/error transitions with a mocked `invoke`, `ImageChoice` column
  lookup for every n in 1..=10, round-vs-square mark by variant, multi-select
  answer joined with `,`, and a failed tile remaining selectable.
- `cargo test` in `src-tauri/` — guard rejects oversized tables, too many
  matrix rows, a `https:` image, a malformed `data:` URI, an unknown
  `displayStyle`; `load_image` rejects a missing file, a non-image extension, a
  file over the cap, and mismatched magic bytes; accepts a small real PNG.
- `npm run typecheck:tests`.
- Manual: `./scripts/demo.sh` gains one demo prompt per new widget (table,
  matrix, slider rows, image ok + image error) for visual QA.

## Risks

- **Panel height.** Even capped at 3 matrix rows, a table + matrix + chips
  approaches the 560px clamp. Mitigation: the caps above, plus the existing
  content-driven `resize_panel` fit. If a combination still overflows, the
  panel scrolls — acceptable, but the caps are tuned to make it rare.
- **ARIA-only controls.** Hand-rolled radios/checkboxes lose native keyboard
  behaviour, so Space/Enter and arrow-key navigation must be implemented and
  tested explicitly. If that proves fragile, the fallback is a visually hidden
  native `<input type=radio>` behind the drawn mark — same look, real
  semantics.
- **`ImageChoice` is the biggest height consumer.** Ten tiles at 5 columns is
  ~140px, and a prompt with a long question, a caption line, and buttons still
  fits — but 10 tiles plus a table in the same surface will clamp and scroll.
  The guard caps `options.len()` at 10; compositions that stack widgets are the
  agent's choice and the existing clamp is the backstop.
- **`load_image` is a file-read primitive reachable from the webview.** It is
  bounded (one path, image extensions only, size cap, magic-byte sniff) but it
  is still a new read surface. It is only reachable from the panel webview,
  which the CSP pins to `'self'`.
- **The image memo map grows** with distinct paths per panel. It is cleared on
  each `prompt.id` change with the rest of the surface state.

## Out of scope

- Input cells in tables, sorting, filtering, column resize.
- Remote image fetching, including an opt-in config flag.
- More than 3 matrix rows, more than 4 table columns, more than 10 image
  options.
- Zoom/lightbox on an image tile, image reordering, upload-from-panel.
- Any change to the answer contract (`{answer, via, elapsed_s}` is untouched).
