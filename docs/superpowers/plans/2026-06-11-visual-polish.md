# Cenno Visual Polish — match the Reporter frames (plan 5a)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. TDD each task; steps use `- [ ]`.

**Goal:** Make the live panels match the Reporter design comps in `docs/design/frames/final/` — the structural gaps QA deferred: panel chrome (wordmark + dismiss ✕), mood bare-word choices, dots pinned to the bottom, quiet Send placement.

**Reference:** the frames (Read renders PNGs) — `panel-mood-checkin.png` (wordmark top-left, ✕ top-right, bare words "Awful Bad Okay Good Great" in one row), `panel-free-text.png` (underline + mic + quiet "Send" bottom-right), `fullscreen-ema-1-scale.png` (dots pinned bottom-center). Gap list: `docs/design/qa/REPORT.md` backlog §2,3,5,6.

**Architecture:** chrome is a layer in `PromptPanel.tsx` OUTSIDE the A2UI surface (consistent across simple + a2ui payloads). Mood/Send treatments flow through `desugar.ts` → catalog variant props → `views.tsx`/`catalog.css`. The panel root becomes a full-height flex column so dots pin to the bottom.

**Out of scope:** EMA header caption (needs a protocol wording decision), Send placement for `a2ui` payloads (only the desugared Send moves), fullscreen surface.

---

### Task 1: Panel chrome — wordmark + dismiss ✕

**Files:** `src/PromptPanel.tsx`, `src/App.tsx`, `src/PromptPanel.test.tsx`, `src/App.css` (+ a `src/Chrome.tsx` if cleaner).

Every panel gets a fixed chrome layer rendered by PromptPanel *around* the surface (not inside the A2UI catalog, so it's identical for simple and rich payloads):
- top-left: `cenno` wordmark, caption size (`--cenno-type-caption`), `--cenno-text-dim`, lowercase.
- top-right: a `✕` button, dim, ≥24px hit area, `aria-label="Dismiss"`.

Dismiss semantics: clicking ✕ resolves the prompt immediately as **no answer** — same wire shape the agent already handles on timeout (`{answered:false, prompt_id}`), so no protocol change. It calls a new `onDismiss(promptId)` that invokes a Tauri command `dismiss_prompt(id)` → `registry.resolve` is wrong (that delivers an answer); instead the Rust side must make the parked `ask()` return TimedOut early. Simplest correct approach: add `registry.dismiss(id) -> bool` that takes the oneshot sender and drops it (causes the `ask()` rx to error → the existing timeout arm returns `TimedOut`). Add `#[tauri::command] fn dismiss_prompt`. Then App clears + hides like an answer.

- [ ] **Rust first (TDD `registry.rs`):** `dismiss(&self, id) -> bool` — take the pending sender (drop it) so `ask()`'s `rx.await` resolves to `Err` and returns `TimedOut`; returns false for unknown id. Test: spawn `ask()`, `dismiss(id)`, assert the result is `TimedOut` and the id matches; dismiss-unknown → false. `cargo test registry`.
- [ ] `lib.rs`: `#[tauri::command] fn dismiss_prompt(state, id) -> bool { state.dismiss(&id) }`, register it.
- [ ] **Frontend TDD (`PromptPanel.test.tsx`):** chrome renders the `cenno` wordmark and a `Dismiss` button; clicking Dismiss calls `onDismiss(prompt.id)`. Markdown-strong + submit tests still pass (chrome doesn't disturb them).
- [ ] Implement chrome in PromptPanel; App wires `onDismiss` → `invoke("dismiss_prompt", {id})` then clears state + hides (reuse the answered/hide path WITHOUT the "noted." linger — dismiss is silent).
- [ ] CSS: chrome row absolutely positioned top, `justify-content: space-between`, caption type, dim color; doesn't overlap content (content padding-top accounts for it).
- [ ] Gates (vitest, cargo test, build) + commit `feat(panel): wordmark + dismiss chrome on every panel`.

### Task 2: Mood choices as bare words

**Files:** `src/a2ui/desugar.ts`, `src/a2ui/catalog.tsx`, `src/a2ui/views.tsx`, `src/a2ui/catalog.css`, `desugar.test.ts`, `views.test.tsx`.

The mood frame shows choices as bare oversized words in one row, no pill outline. Make it flow-aware:
- [ ] desugar: when `req.flow === "mood"` and kind is `choice`, set `variant: "words"` on the ChoicePicker component (default/absent = current pills). Table test asserts the variant appears only for mood.
- [ ] catalog/views: `ChipsView` (or ChoicePicker view) honors `variant: "words"` → render each option as a bare text button (question-m size, white, generous gap, wraps if needed), no border/background; pressed = full-weight/underline. Keep ≥44px tap target via padding. Test: words variant renders buttons without the `.cenno-chip` pill class; click still reports the value.
- [ ] Gates + commit `feat(a2ui): mood flow renders choices as bare words`.

### Task 3: Dots pinned to the bottom

**Files:** `src/PromptPanel.tsx` / surface wrapper, `src/a2ui/catalog.css`, `src/App.css`.

- [ ] The surface column must fill the panel height so the Dots component sits at the bottom edge (frame: dots fixed bottom-center across steps). Make the rendered root a full-height flex column; the Dots row gets `margin-top: auto`. Verify the Dots stay bottom-center while content sits top, with a tall and a short prompt (vitest can assert the class/structure; visual confirmed in Task 5).
- [ ] Don't break non-progress prompts (no dots → no empty reserved space that looks wrong).
- [ ] Gates + commit `feat(panel): pin pagination dots to the bottom edge`.

### Task 4: Quiet Send placement

**Files:** `src/a2ui/desugar.ts`, `src/a2ui/catalog.css`, `desugar.test.ts`.

Frame shows "Send" as quiet text, bottom-right — not a white primary pill bottom-left.
- [ ] desugar: the Send button for text/voice gets `variant: "quiet"` (or `borderless`); keep the action identical. Test asserts the Send variant.
- [ ] catalog.css: quiet Send = text-only (no pill bg), `--cenno-text`, aligned bottom-right (`align-self: flex-end`). Confirm Yes/No stay as-is (those are real choices, the reminder frame shows them as pills).
- [ ] Gates + commit `feat(a2ui): quiet bottom-right Send for text prompts`.

### Task 5: Visual QA re-capture + close-out

- [ ] Rebuild (`npx tauri build --no-bundle`), launch `--tray` (disable fullscreen quiet mode for the session), fire mood / text / scale-with-progress / confirm via the demo/socket pattern, screencapture each panel window, READ each against its frame. Update `docs/design/qa/REPORT.md` with the new verdicts; re-save `docs/design/qa/qa-*.png`.
- [ ] Fix any cheap CSS gaps inline; file structural leftovers in backlog.
- [ ] Full gate: cargo test, clippy -D warnings, vitest, typecheck:tests, npm build. Commit `test(design): visual QA re-capture after chrome/mood/dots/send polish`.

---

## Self-review notes
- Chrome lives outside the A2UI surface so a2ui payloads also get wordmark + ✕ for free.
- Dismiss reuses the existing TimedOut wire contract — zero protocol change, agents already handle it.
- Mood/Send variants are additive desugar fields; existing exact-JSON/table tests get deliberate updates, not loosened.
- Type roles unchanged (question-m for panels per TOKENS.md); this is layout + treatment, not new tokens.
