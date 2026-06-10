# Cenno ask_sequence — questions in a row, instant advance (plan 5b)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. TDD each task; `- [ ]` steps.

**Goal (user's words):** "a way to schedule a few questions in a row — when a question is answered, the next one is filled in immediately." One MCP call runs N questions in a single panel; answering one swaps to the next with no hide/reshow gap; hides only after the last.

**Architecture:** a new `ask_sequence` MCP tool runs the questions sequentially in Rust (fires the next `registry.ask` the instant the previous resolves). The panel-event payload gains a `seq` marker so the frontend knows NOT to hide between steps and instead swaps content. Per-question timeout ends the run early, returning answers-so-far. No change to the single `ask_user` tool.

**Protocol additions (additive, optional — existing wire shapes unchanged):**
- New tool `ask_sequence { questions: AskRequest[], flow?: Flow }` → `{ answers: AskResponse[] }`.
- `PromptEvent` gains `seq: Option<{ index: u32, total: u32, last: bool }>` (absent for plain `ask_user`).

---

### Task 1: Rust `ask_sequence` tool + seq-tagged events

**Files:** `src-tauri/src/mcp.rs`, `src-tauri/src/lib.rs` (PromptEvent + the notify closure), `src-tauri/src/protocol.rs` (request/response types), `tests/mcp_socket.rs`.

- [ ] **protocol.rs:** `SequenceRequest { questions: Vec<AskRequest>, #[serde(default)] flow: Option<Flow> }` and `SequenceResponse { answers: Vec<AskResponse> }` (both Serialize/Deserialize + JsonSchema, matching the existing derives). Unit test: round-trips, empty questions → valid (answers empty).
- [ ] **lib.rs PromptEvent:** add `#[serde(skip_serializing_if = "Option::is_none")] seq: Option<SeqMeta>` where `SeqMeta { index: u32, total: u32, last: bool }`. Plain `ask_user` emits `seq: None` (must keep existing PromptEvent tests/JSON green). The notify closure signature currently `(id, &AskRequest, remaining_s)` — extend so a sequence can pass seq meta; simplest: a richer notify or a second emit path. Keep the single-ask path byte-identical on the wire.
- [ ] **mcp.rs `ask_sequence` tool:** for each question i in 0..N:
  - if `req.flow` is set on the sequence and the question has none, apply it; set `question.progress = {step: i+1, total: N}` when absent (auto dots).
  - `let resp = registry.ask(question, notify-with-seq{index:i, total:N, last: i==N-1}).await`.
  - push resp; if `resp` is `TimedOut`, **break** (return answers-so-far).
  - record each outcome to history (same as ask_user does) so the DB has one row per question.
  Return `SequenceResponse { answers }` serialized. Guard: if `questions` is empty, return `{answers: []}`. If any question carries `a2ui`, validate it with the existing `a2ui_guard` before asking (same as ask_user).
- [ ] **Integration test (tests/mcp_socket.rs):** call `ask_sequence` with 3 questions over the socket; an auto-answerer resolves each pending prompt in turn; assert 3 answers returned in order, and that the DB recorded 3 rows. A second test: question 2 times out (short timeout, no answer) → answers has 1 entry (or 2 with the 2nd TimedOut — pick: include the TimedOut entry then stop, so `answers.len()==2` with `answers[1]` TimedOut) — assert that shape and that no row is lost.
- [ ] Gates: `cargo test`, `clippy -D warnings`. Commit `feat(mcp): ask_sequence — run questions back-to-back in one call`.

### Task 2: Frontend instant-advance (no hide between steps)

**Files:** `src/App.tsx`, `src/PromptPanel.tsx` (only if needed), `src/App.test.tsx`.

- [ ] The prompt event type gains optional `seq: {index,total,last}`. When the user answers a prompt whose `seq` exists and `last === false`: invoke `answer_prompt` as usual but **do NOT hide and do NOT run the "noted." linger** — keep the panel visible; the next `prompt` event (arriving ~immediately from the Rust loop) replaces the content via the existing `key={prompt.id}` remount. When `seq` is absent OR `seq.last === true`: hide as today (with the linger).
- [ ] Avoid a flash: between answering step i and the arrival of step i+1, keep the current panel mounted (don't `setActive(null)`); just let the incoming event overwrite `active`. Confirm the `hideGenerationRef` guards still prevent a stray timer from hiding mid-sequence.
- [ ] Tests (App.test.tsx, fake timers + mocked invoke/listen): (a) answering a `seq.last=false` prompt does NOT call `hide()`; a subsequent `prompt` event swaps the rendered title. (b) answering a `seq.last=true` prompt DOES hide after the linger. (c) a plain prompt (no seq) hides as before (existing behavior intact).
- [ ] Gates: vitest, typecheck:tests, build. Commit `feat(panel): advance instantly between sequence questions`.

### Task 3: Docs + live E2E

**Files:** `skills/cenno/SKILL.md`, `README.md`.

- [ ] Skill: document `ask_sequence` — when to use (a short questionnaire / several related questions where the panel should stay up), the shape, auto-progress, timeout-ends-early semantics, and that answers come back as an ordered array. Add a 3-question example.
- [ ] README: add `ask_sequence` to the tools area near `ask_user` (one row + a sentence).
- [ ] **Live E2E:** build (`npx tauri build --no-bundle`), launch `--tray` (disable fullscreen quiet mode for the session; restore after). Fire an `ask_sequence` of 3 questions (e.g. mood scale via a2ui 1–5, a choice, a text) through the socket. Answer them; verify the panel **stays up and swaps instantly** between questions (screencapture two consecutive steps; the window id stays the same, content changes, no hide in between), and that the tool returns 3 ordered answers. `cenno export` shows the 3 new rows. Kill cenno after.
- [ ] Full gate: cargo test, clippy -D warnings, vitest, typecheck:tests, npm build. Commit `docs: ask_sequence in skill + README; live E2E verified`.

---

## Self-review notes
- `ask_sequence` reuses `registry.ask` and the existing show/answer machinery; the only new behavior is "don't hide while `seq.last==false`". Minimal surface.
- Per-question records keep history granular (each answer is its own row), consistent with `ask_user`.
- Timeout-ends-early returns a short `answers` array — the agent sees exactly how far the user got; no partial-row ambiguity.
- All additions are optional/serde-skipped so existing `ask_user` wire tests stay green.
