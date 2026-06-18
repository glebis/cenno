# cenno iOS companion MVP — native A2UI v0.9 renderer + landscape

**Date:** 2026-06-13
**Status:** Design approved, ready for implementation plan
**Scope:** iPhone target only (Watch unchanged)

## Goal

Bring the iPhone companion app to **A2UI rendering parity with the tauri
macOS app**: every prompt — simple (`title`/`body`/`input`/`choices`) or a
raw `a2ui` passthrough layout supplied by an agent — renders through a single
A2UI v0.9 runtime, with full Markdown and rich layout support. The app must
also work in **landscape orientation** (today it is portrait-only).

## Background / current state

- The Mac relay writer (`src-tauri/src/relay.rs`) writes the **entire
  `AskRequest` JSON** as the CloudKit `payload` — including the `a2ui`
  passthrough field, `progress`, and `urgency`. The data is already on the
  wire; the Swift `PromptPayload` Codable simply does not decode those fields.
  **No CloudKit schema change is required.**
- `PhonePromptDetailView` renders simple prompts with hand-rolled native
  SwiftUI controls (confirm/choice/scale/text/voice_text) and renders
  `bodyMd` as plain `Text(String)` — which does **not** render Markdown.
  Raw `a2ui` passthrough prompts cannot be rendered at all.
- The tauri app renders **everything** through one A2UI runtime
  (`@a2ui/web_core`), desugaring simple prompts into v0.9 messages
  (`src/a2ui/desugar.ts`) and rendering them with the cenno catalog
  (`src/a2ui/catalog.tsx`, id `cenno:catalog/v1`).

### Decisive version finding

cenno's catalog imports `@a2ui/web_core/v0_9` and `@a2ui/react/v0_9`, and
`desugar.ts` emits `version: "v0.9"` envelopes. **cenno speaks A2UI v0.9 on
the wire**, despite the npm package being `0.10.0` (npm version ≠ spec
version; the v1.0 spec "was previously known as 0.10 in draft"). The chosen
Swift runtime targets v0.9, so **no version-translation shim is needed.**

## Decisions

| Decision | Choice |
|---|---|
| Render path | Single A2UI path (literal parity with tauri) |
| A2UI runtime | Adopt `BBC6BAE9/a2ui-swift` (MIT, SPM, SwiftUI, v0.9), **vendored as a pinned fork** under `companion/Vendor/` |
| Desugar | **Port `desugar.ts` → Swift** pure function; iOS desugars simple prompts locally (no Mac changes) |
| Watch | **Out of scope** — Watch keeps its current native simple-prompt UI |

## Architecture

### Rendering pipeline

```
PromptRecord (from CloudKitRelay)
   │
   ├─ payload.a2ui present?  ── yes ──▶ use a2ui messages verbatim
   │                          ── no  ──▶ A2UIDesugar(payload) → v0.9 messages
   ▼
A2UISurfaceView(viewModel:catalog: CennoCatalog)   // a2ui-swift
   │  renders component tree, manages data model + bindings
   ▼
action closure { name, context:{ value, via } }
   │  (any name starting with "submit")
   ▼
PromptAnswer(answer: value, via: via, elapsedS: <timer>, device: "iphone")
   ▼
CloudKitRelay.submit(answer:for:)
```

### Components / units

1. **`A2UIDesugar` (Swift)** — pure function, port of `src/a2ui/desugar.ts`.
   Input: `PromptPayload`. Output: `[A2uiMessage]` (createSurface +
   updateComponents + updateDataModel) with the **same deterministic
   component ids** (`root`, `col`, `title`, `body`, `input`, `send`, `dots`,
   `choices`, `scale`, `actions`/`yes`/`no`, label children). Mirrors
   `desugar.test.ts` for byte-parity. No dependency on the renderer.

2. **`CennoCatalog` (Swift)** — conforms to a2ui-swift's
   `CustomComponentCatalog`. Provides native SwiftUI views for the
   `cenno:catalog/v1` component set. a2ui-swift accepts the opaque
   `catalogId` and routes unknown `typeName`s to this catalog's
   `build(typeName:node:surface:)`. Components:
   - **Custom types:** `Scale` (discrete numeral row + end labels),
     `Dots` (step pagination).
   - **Extended standard types** (re-declared because a2ui-swift's built-in
     property structs drop cenno's extra props): `Text`
     (Markdown via `AttributedString(markdown:)` + h1–h5/caption → role
     sizing), `Row`, `Column`, `Button` (`primary`/`borderless`/`quiet`),
     `TextField` (+`voice` push-to-talk dictation), `ChoicePicker`
     (chips + `words` variant), `Slider` (+`minLabel`/`maxLabel`),
     `DateTimeInput` (native system picker), `Image` (remote load).

3. **`PromptPayload` decode extension** — add `a2ui: JSONValue?`,
   `progress: Progress?`, `urgency: String?` to the Codable so the iOS side
   can read what the Mac already ships.

4. **`A2UIPromptView` (Swift)** — replaces `PhonePromptDetailView`'s input
   switch. Owns the a2ui-swift `SurfaceViewModel`, feeds it the desugared /
   passthrough messages, wraps `A2UISurfaceView` in a `ScrollView`, starts
   the elapsed timer on appear, and bridges the action closure to
   `relay.submit` / `relay.markTimedOut`.

5. **Voice dictation** — the `TextField` view reuses the app's existing
   on-device SpeechTranscriber path already shipped for `voice_text`.

### Landscape

- Info.plist: add landscape to `UISupportedInterfaceOrientations~iphone`.
- `A2UIPromptView` wraps the surface in a `ScrollView` with keyboard
  avoidance so inputs stay reachable in landscape.
- Verify cenno `Row`/`Column` views consume horizontal space and wrap
  correctly when wide; the queue list also rotates.

## Data flow / boundary safety

Prompts are untrusted input (same threat model as the Mac side's
`a2ui_guard.rs`). The Swift side must validate/clamp before rendering:
a2ui-swift performs schema validation on `processMessages`; the desugar path
produces only known-good trees. Unknown component `typeName`s outside the
cenno catalog render as an empty/placeholder view rather than crashing.

## Testing

- **Desugar parity:** Swift unit tests mirroring `desugar.test.ts` — same
  inputs produce the same message arrays and component ids.
- **Catalog rendering:** a render smoke test per cenno component (Text
  markdown, Scale, Dots, chips/words, Slider labels, Button variants,
  DateTimeInput, Image).
- **Action → answer mapping:** `submit*` actions produce the correct
  `PromptAnswer` (value, via, device, elapsed).
- **Landscape:** layout check that inputs remain reachable and Row/Column
  use horizontal space when wide.
- **Passthrough:** a raw `a2ui` payload renders without going through
  desugar.

## Out of scope (YAGNI for MVP)

- watchOS A2UI parity (Watch stays native simple-prompt).
- v1.0-only features: `surfaceProperties` theming, `actionResponse` /
  `callFunction` round-trips, null-deletion data-model semantics.
- Offline answer editing / draft sync.

## Risks & mitigations

- **a2ui-swift bus factor** (~37★, 2 contributors, single maintainer):
  vendor a **pinned fork** under `companion/Vendor/`, not a floating SPM
  ref, so upstream abandonment or spec drift can't break the build and we
  can patch the v0.9 renderer directly.
- **Markdown:** cenno `Text` must render Markdown itself
  (`AttributedString(markdown:)`); a2ui-swift's basic Text won't.
- **Desugar drift:** two desugar implementations (TS + Swift) — kept in sync
  by mirroring the shared test corpus; treat `desugar.test.ts` as the
  contract.

## Open implementation questions (resolve during planning)

- Exact a2ui-swift public API for registering a `CustomComponentCatalog` and
  reading two-way bound values for submit actions (verify against the
  vendored commit).
- Whether the elapsed timer should start on surface appear or first
  interaction (web uses display time — match it).
