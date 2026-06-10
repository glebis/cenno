# Cenno Rendering & Tokens Implementation Plan (Plan 2 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One rendering path — the simple `ask_user` form desugars to A2UI and renders through our token-styled component catalog, looking like the Reporter-style frames in `docs/design/`.

**Architecture:** DTCG `tokens.json` → Style Dictionary → CSS custom properties; per-flow theming via a `data-flow` attribute. A TS desugar module maps `AskRequest` → A2UI component list; `@a2ui/react@0.10` (v0_9 entry) renders it through the `cenno:catalog/v1` custom catalog (validated by the spike — see `docs/superpowers/research/2026-06-a2ui-react-spike.md` and the working harness in `spike/a2ui/src/`). Rust validates incoming `a2ui` payloads at the MCP boundary (the renderer silently ignores bad versions — spike finding).

**Tech Stack:** style-dictionary 4, @a2ui/react 0.10 + @a2ui/web_core 0.10, existing React 19 / vitest / Tauri 2 stack. Design source of truth: `docs/design/TOKENS.md` + `docs/design/frames/final/`.

**Out of scope (later plans):** actual voice capture (plan 3 — mic button here is a stub), fullscreen/tray windows and urgency policy (plan 4), multi-prompt queueing. The catalog and tokens must be surface-agnostic so plan 4 reuses them.

---

### Task 1: DTCG tokens + Style Dictionary pipeline

**Files:**
- Create: `tokens/tokens.json` (DTCG format)
- Create: `style-dictionary.config.mjs`
- Modify: `package.json` (devDep `style-dictionary@^4`, script `"tokens": "style-dictionary build --config style-dictionary.config.mjs"`, hook into `"dev"`/`"build"` via `pre` scripts)
- Generated (committed): `src/styles/tokens.css`
- Test: `src/styles/tokens.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// src/styles/tokens.test.ts
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

describe("generated tokens.css", () => {
  it("contains every flow color from TOKENS.md", () => {
    for (const [name, hex] of [
      ["--cenno-color-flow-mood", "#FF6250"],
      ["--cenno-color-flow-question", "#1E4FD8"],
      ["--cenno-color-flow-ema", "#0E7C6B"],
      ["--cenno-color-flow-reminder", "#4A5568"],
      ["--cenno-color-flow-ambient", "#14171A"],
    ]) expect(css.toLowerCase()).toContain(`${name.toLowerCase()}: ${hex.toLowerCase()}`);
  });
  it("contains type scale and spacing", () => {
    expect(css).toContain("--cenno-type-question-l: 44px");
    expect(css).toContain("--cenno-type-caption: 13px");
    expect(css).toContain("--cenno-space-3: 24px");
    expect(css).toContain("--cenno-radius-control: 10px");
  });
});
```

- [ ] **Step 2:** `npx vitest run src/styles` → FAIL (no tokens.css).

- [ ] **Step 3: Author tokens.json from docs/design/TOKENS.md** — DTCG format (`$type`/`$value`), groups: `color.flow.{mood,question,ema,reminder,ambient}`, `color.{paper,text,text-dim,line}` (dim/line as rgba of white at 0.6/0.4), `type.{question-l,question-m,body,caption}` (dimension px), `space.{1..5}` = 8/16/24/40/64, `radius.{control,pill}` = 10/999. Style Dictionary config: css/variables transform group, prefix `cenno`, output `src/styles/tokens.css`, selector `:root`. Run `npm run tokens`.

- [ ] **Step 4:** `npx vitest run src/styles` → 2 passed. Commit: `feat(tokens): DTCG tokens compiled to CSS custom properties`

---

### Task 2: Flow theming — `data-flow` attribute + semantic aliases

**Files:**
- Create: `src/styles/theme.css` (semantic aliases per flow)
- Modify: `src/main.tsx` (import tokens.css + theme.css)
- Test: `src/styles/theme.test.ts`

- [ ] **Step 1: Failing test** — parse theme.css as text (jsdom can't compute custom-prop cascades reliably):

```ts
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
const css = readFileSync(new URL("./theme.css", import.meta.url), "utf8");
describe("theme.css", () => {
  for (const flow of ["mood", "question", "ema", "reminder", "ambient"])
    it(`maps data-flow=${flow} to its hue`, () =>
      expect(css).toContain(`[data-flow="${flow}"]`));
  it("defines the semantic surface var", () =>
    expect(css).toContain("--cenno-surface: var(--cenno-color-flow-question)"));
});
```

- [ ] **Step 2:** FAIL. **Step 3:** Implement: default `:root { --cenno-surface: var(--cenno-color-flow-question); }`; each `[data-flow="X"] { --cenno-surface: var(--cenno-color-flow-X); }`. Components consume ONLY semantic vars (`--cenno-surface`, `--cenno-color-text`, etc.) — never flow colors directly. **Step 4:** green. Commit: `feat(tokens): per-flow theming via data-flow attribute`

---

### Task 3: Protocol — optional `flow` and `progress` fields

**Files:**
- Modify: `src-tauri/src/protocol.rs`, `src/PromptPanel.tsx` Prompt interface (TS mirror)

- [ ] **Step 1: Failing Rust tests** (add to protocol.rs tests):

```rust
#[test]
fn flow_and_progress_roundtrip() {
    let json = r#"{"title":"t","flow":"mood","progress":{"step":2,"total":3}}"#;
    let req: AskRequest = serde_json::from_str(json).unwrap();
    assert!(matches!(req.flow, Some(Flow::Mood)));
    assert_eq!(req.progress.as_ref().unwrap().step, 2);
}

#[test]
fn flow_and_progress_absent_from_wire_when_none() {
    let req: AskRequest = serde_json::from_str(r#"{"title":"t"}"#).unwrap();
    let back = serde_json::to_string(&req).unwrap();
    assert!(!back.contains("flow"));
    assert!(!back.contains("progress"));
}
```

- [ ] **Step 2:** FAIL. **Step 3:** Implement:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Flow { Mood, Question, Ema, Reminder, Ambient }

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Progress { pub step: u32, pub total: u32 }

// in AskRequest:
#[serde(default, skip_serializing_if = "Option::is_none")] pub flow: Option<Flow>,
#[serde(default, skip_serializing_if = "Option::is_none")] pub progress: Option<Progress>,
```
`skip_serializing_if` keeps the existing exact-JSON wire tests passing — verify they still do. **Step 4:** `cargo test protocol` all green (now 5+). Commit: `feat(protocol): optional flow theme and progress fields`

---

### Task 4: A2UI catalog — cenno components styled by tokens

**Files:**
- Create: `src/a2ui/catalog.tsx` (components: CText, CTextField, CButton, CChips, CScale, CDots, CColumn/CRow), `src/a2ui/catalog.test.tsx`
- Modify: `package.json` (deps `@a2ui/react@0.10`, `@a2ui/web_core@0.10`)

**Reference first:** `spike/a2ui/src/catalog.tsx` (working `createComponentImplementation(ButtonApi, ...)` + `new Catalog('cenno:catalog/v1', ...)` idioms) and the spike findings doc. Reuse the standard component APIs (TextApi, TextFieldApi, ButtonApi…) with our implementations; add custom APIs only where no standard type fits (Chips, Scale, Dots — check web_core's standard set first; MultipleChoice/Slider may exist — prefer standard APIs with our rendering, fall back to custom types in the `cenno:` catalog namespace).

- [ ] **Step 1: Failing component tests** — render each component standalone (not via renderer yet) with token CSS classes asserted, e.g.:

```tsx
it("CScale renders n tap targets and reports selection", () => {
  const onAction = vi.fn();
  render(<CScaleView min={1} max={7} onSelect={onAction} />);
  expect(screen.getAllByRole("button")).toHaveLength(7);
  fireEvent.click(screen.getByRole("button", { name: "5" }));
  expect(onAction).toHaveBeenCalledWith(5);
});
it("CText renders markdown strong", () => {
  render(<CTextView markdown="is **bold**" />);
  expect(screen.getByText("bold").tagName).toBe("STRONG");
});
```
(Each catalog component = thin A2UI adapter around a plain `*View` component; test the views directly — adapters get covered in Task 6's integration.)

- [ ] **Step 2:** FAIL. **Step 3:** Implement views with ONLY semantic token vars in a `src/a2ui/catalog.css`; markdown via react-markdown inside CText (avoids the MarkdownContext gap — spike finding). Mic button in CTextField is a visual stub: rendered when `voice` enabled, disabled with `title="voice arrives in plan 3"`. Match the frames: chips = outline pills (radius-pill), scale = big numerals, dots = fixed bottom-center. **Step 4:** green. Commit: `feat(a2ui): cenno component catalog styled by tokens`

---

### Task 5: Desugar module — AskRequest → A2UI component list

**Files:**
- Create: `src/a2ui/desugar.ts`, `src/a2ui/desugar.test.ts`

- [ ] **Step 1: Failing table tests** — for each input.kind assert the produced flat list (shape per spike's `messages.ts`): text → [Card[Column[Text(title), Text(body), TextField, Button(Send)]]]; choice → Chips with the request's choices; scale → CScale 1..7 (or choices-derived range); confirm → two Buttons; none → no input row; voice+text → TextField with voice flag. progress present → Dots component appended. Root carries `dataModel` bindings for the answer path. Test ids are deterministic (`title`, `body`, `input`, `send`, `dots`).

- [ ] **Step 2:** FAIL. **Step 3:** Implement as a pure function `desugar(req: AskRequest): A2uiMessage[]` producing the same message envelope the spike used (`beginRendering` + components list, v0.9 shape). **Step 4:** green. Commit: `feat(a2ui): simple-form desugaring to A2UI component list`

---

### Task 6: PromptPanel v2 — render everything through the A2UI path

**Files:**
- Modify: `src/PromptPanel.tsx`, `src/PromptPanel.test.tsx`, `src/App.tsx` (pass full request through; set `data-flow` from request.flow ?? default mapping), `src/App.css` (panel chrome only — content styling now lives in catalog.css)

- [ ] **Step 1: Extend the existing tests, keep the old ones passing** — same two behaviors (markdown STRONG; submit → onAnswer(id, text, "text")) but now THROUGH the A2UI renderer + desugar path, plus: choice request renders chips and clicking one calls onAnswer(id, choice, "choice"); flow="mood" sets data-flow="mood" on the panel root.

- [ ] **Step 2:** FAIL (old direct-render path still in place). **Step 3:** Replace PromptPanel internals: desugar(request) → A2UI renderer with cenno catalog (init pattern from spike App.tsx: processor + injectBasicCatalogStyles + action handler routing `submit` actions to onAnswer). Keep the component's external contract (`prompt`, `onAnswer`) unchanged. **Step 4:** all vitest green (old + new). **Step 5:** `npm run build` + `cargo test` still green. Commit: `feat(panel): single A2UI rendering path for simple and rich prompts`

---

### Task 7: Native a2ui passthrough + Rust boundary validation

**Files:**
- Modify: `src/PromptPanel.tsx` (if `request.a2ui` present: validated payload renders instead of desugared list)
- Modify: `src-tauri/src/protocol.rs` or new `src-tauri/src/a2ui_guard.rs`: `validate_a2ui(&serde_json::Value) -> Result<(), String>` — checks: top level is the expected v0.9 message-list shape, version field (if present) is `v0.9`-compatible, component count ≤ 200, total size ≤ 256 KiB. Spike finding: the renderer SILENTLY ignores wrong versions — this guard is the only protection.
- Modify: `src-tauri/src/mcp.rs` ask_user tool body: on invalid a2ui return a tool error (rmcp error result) with the validation message — agent can retry; never a blank surface.
- Tests: Rust unit tests for the guard (valid passes, wrong version rejects with message, oversize rejects); TS test: PromptPanel with a tiny valid a2ui payload renders its Text instead of desugared title.

- [ ] Steps: failing tests → implement → green (`cargo test`, vitest) → commit: `feat(a2ui): native payload passthrough with boundary validation`

---

### Task 8: CSP tightening

**Files:**
- Modify: `src-tauri/tauri.conf.json`: `"csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'"` (unsafe-inline needed by Vite-injected styles and React inline styles; no remote origins, no script escape hatches)
- Modify: README security note (CSP now set; remaining TODO only if remote content ever allowed)

- [ ] Verify: `npm run build && npx tauri build --no-bundle`, launch built binary, trigger a prompt via CLI (`cenno ask "csp check" --timeout 8`), confirm the panel renders with styles intact (screencapture; a CSP break = unstyled/blank panel + console errors in stderr log). cargo test + vitest green. Commit: `fix(security): enable CSP`

---

### Task 9: Visual QA against the frames

**Files:**
- Create: `scripts/visual-qa.sh` + `docs/design/qa/` (screenshots)

- [ ] With the built binary + a helper loop: trigger one prompt per state — mood (flow=mood, choices as mood words), free text (cobalt), choice chips, reminder (flow=reminder, confirm kind), scale with progress dots (flow=ema) — screencapture each panel (`screencapture -l<windowid>`), save alongside the matching `docs/design/frames/final/*.png` names into `docs/design/qa/`. 
- [ ] Read each pair side by side (the Read tool renders PNGs) and write `docs/design/qa/REPORT.md`: per-frame verdict (matches direction / deviates how), gaps that are token fixes vs catalog fixes. Fix cheap CSS gaps inline (≤30 min), file the rest into the backlog.
- [ ] Commit: `test(design): visual QA screenshots and report vs Reporter frames`

---

### Task 10: Docs + plan close-out

- [ ] README: update status (plan 2 done), tools table note (`a2ui` field now live), add a "Design system" section pointing at TOKENS.md / tokens.json pipeline (`npm run tokens`).
- [ ] backlog.md: remove completed plan-2 items; add anything Task 9's report deferred.
- [ ] Full gate: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `npx vitest run`, `npm run build`, `./scripts/smoke.sh` against the rebuilt binary. Commit: `docs: plan 2 close-out`

---

## Self-review notes

- **Spec coverage (plan-2 scope):** one rendering path ✓ (Task 6 deletes the direct path), token pipeline ✓ (1–2), Reporter look ✓ (4 + 9 QA gate), a2ui field live + validated ✓ (7), spike findings all addressed (MarkdownContext avoided via own CText; version guard; catalog namespace) ✓, CSP backlog item ✓ (8).
- **Protocol change risk:** Task 3's `skip_serializing_if` preserves the pinned wire format; the exact-JSON tests are the regression guard.
- **Known approximation:** exact A2UI renderer init/API calls are referenced to the working spike harness rather than spelled out — same rationale as plan 1's rmcp approach: copy proven idioms, don't invent API syntax.
- **Type consistency:** `Flow`/`Progress` (Task 3) are consumed by desugar (5: `progress` → Dots) and App.tsx `data-flow` (6); names match across tasks.
