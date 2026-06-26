# Diff audit — codex (gpt-5.5), 2026-06-24

Adversarial fresh-context review of the staged diff. Codex read the live files
(the scratchpad diff path wasn't visible to it) and reported 1 blocker + 3 major,
0 minor, 0 nit. All four were judged real and fixed before build.

| # | Sev | Finding | Resolution |
|---|-----|---------|------------|
| 1 | BLOCKER | Draft keys are `p_N`; Rust ids reset each launch, so a stale draft could restore into an unrelated future prompt — text leaking across prompts/agents. | **Fingerprint drafts** by prompt content (`draftFingerprint(title+body)`), stored as `{f,v}` JSON. Restore only on fingerprint match; foreign drafts are dropped. New test: `ignores a draft whose fingerprint doesn't match (reused id)`. |
| 2 | MAJOR | `if (v) setItem(...)` never clears a draft when the field is emptied → "type then delete all" restores stale text. | `saveDraft` `removeItem`s on empty. New test: `clears the draft when the field is emptied`. |
| 3 | MAJOR | Restore's dispatched `input` fires before the keepalive listener mounts (effect order), so Rust never gets the floor → a reopened near-expiry prompt can time out instantly. | After a successful restore, floor `interactionFloorRef` + `invoke("keepalive", EDIT_S)` directly in the show effect. |
| 4 | MAJOR | Global shortcut registered before `SuppressionState` is managed → a hotkey press in the startup window could panic `replay_pending` via `handle.state`. | Moved `register_reopen_shortcut` to after `app.manage(suppress)` (registry already managed earlier). |

Codex confirmed: old config.json compatibility is fine (`#[serde(default)]`); no new
network path; press/release double-fire already handled by the `ShortcutState::Pressed` gate.

Post-fix: `npx tsc --noEmit` clean · `npx vitest run` 148 passed · `cargo test --lib` 108 passed.
Full codex transcript: not committed (scratchpad). Re-audit of the fixed diff: see audit-codex-2.md.
