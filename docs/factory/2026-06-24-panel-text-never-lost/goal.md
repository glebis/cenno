# Goal Contract — Panel text is never lost

> Source of truth. Agents may propose **Goal Amendments**; they may not silently rewrite this.
> Keep it short — caps below. A capped goal stays a goal, not waterfall-in-markdown.
>
> **APPROVED** by user 2026-06-24. Default shortcut suggestion in Settings: `Cmd+Shift+C` (user-changeable).

## Current state (≤3)
- A prompt panel that closes mid-typing (timeout in ≤0.3.1, external dismiss, or any teardown) loses the user's typed text with no recovery — this just happened on the installed 0.3.1.
- 0.3.2 (committed, not yet installed) adds keepalive + a localStorage draft *save* (`App.tsx:306`), but **nothing reads it back** — the save is write-only.
- Once a panel is hidden, there is no user-facing way to bring a still-pending prompt back; `replay_pending()` (`src-tauri/src/lib.rs:354`) exists but is only reachable from internal resume logic.

## Desired future state (≤3)
- A re-shown prompt restores the user's in-progress draft text.
- The user can manually re-open a parked/pending prompt via a tray menu item **and** a configurable global keyboard shortcut.
- The global-shortcut combo is set by the user in Settings and persists across restarts (`~/.cenno/config.json`).

## Current constraint (Theory of Constraints)
Typed answers are destroyed by panel teardown and cannot be recovered — the data-loss path has no read-back and no manual re-open.

## Target user / job (JTBD)
The cenno user answering an agent's question: "When I'm typing an answer and the panel disappears, let me get the panel — and my text — back, instead of retyping from scratch."

## Non-negotiable constraints (≤5)
- No new network calls (AGENTS.md): everything stays local (localStorage + `~/.cenno`).
- Draft persistence stays per-prompt-id and is cleared on answer/dismiss (no stale leakage between prompts).
- Restore must not clobber text the user is actively typing (restore once, on show, before paint).
- Global-shortcut registration must fail soft — a bad/unavailable combo logs and degrades, never crashes the app or blocks startup.
- Validate the configured shortcut at the boundary (untrusted `~/.cenno/config.json`) like other external config.

## Desired outcomes (solution-independent, measurable; ≤5)
- A draft saved for prompt X is written back into the field when prompt X re-shows. (test)
- Triggering re-open (tray item or shortcut) while a pending prompt exists brings the panel back on screen. (test + manual)
- The shortcut combo configured in Settings is the one registered after restart. (test + manual)
- Answering or dismissing a prompt clears its draft (no restore on the next, different prompt). (test)
- The Settings dialog no longer shows the version string. (test/manual)

## Smallest shippable slice   <!-- required -->
Draft restore (frontend read-back of the existing localStorage save) + a tray "Show pending prompt" item calling `replay_pending`. This alone closes the data-loss hole and gives a manual re-open with zero new dependencies. Global-shortcut + Settings config + version removal layer on after.

## Stop condition   <!-- required -->
If adding `tauri-plugin-global-shortcut` forces a capability/entitlement or CSP change that risks the signed-build/updater flow, OR if making the shortcut configurable balloons the Settings/config surface beyond a single field — stop and ask for re-scope (ship the slice; defer the shortcut).

## Success evidence (≤5)
- Vitest: draft restored on re-show; draft cleared on answer/dismiss → not restored for a different id.
- Vitest/manual: Settings renders no version string.
- Rust `cargo test`: shortcut-string parse/validate accepts good combos, rejects garbage without panic.
- Manual: type → close panel → tray item AND shortcut each reopen with text restored (recorded in `evidence/verify.log`).
- `evidence/verify.log`: `npx vitest run`, `cargo test`, typecheck, `npx tauri build` output.

## Visual checkpoints
Settings dialog (version line removed) at the standard Settings window size — one screenshot in `evidence/screenshots/`.

## Risk classification
R2 user-facing low-stakes (personal productivity tool, local-only, no sensitive data/recommendations/profiling).
EU AI Act: Art 5 prohibited use? no · Art 50 labelling? N/A

## Rollback note
Single feature branch; revert the merge commit. Draft restore and reopen are independent — either can be reverted alone. Shortcut is config-gated (empty/absent combo = no global shortcut registered).

## Risks (≤5)
- DOM-level restore may not bind for every a2ui input kind (textarea works; date/contenteditable need handling) — scope restore to text/textarea first.
- Global-shortcut combo can collide with a system/other-app binding — fail soft + let the user change it in Settings.
- New plugin dependency could affect the signed `tauri build` / updater path (see Stop condition).
- macOS may require Accessibility permission for global shortcuts — degrade gracefully if unauthorized.

## Non-goals (≤5)
- Restoring non-text selections (chips/scale/rating button state).
- Cross-prompt or cross-restart draft survival beyond what localStorage already gives.
- Reworking the keepalive/timeout logic already shipped in 0.3.2.
- A full Settings redesign — only add the shortcut field and remove the version line.
- Publishing the GitHub release (local build & install only this round; codex audits first).

## Release constraints
- Build with `PATH="/usr/bin:$PATH" npx tauri build` (xattr gotcha, AGENTS.md).
- Stays 0.3.2 (already-bumped versions); install locally, do not publish to GitHub yet.
- Codex audits the diff before merge.

## Tracker
bd — decomposes into >1 task (draft restore · tray item · global-shortcut plugin+register · Settings config field · version removal).

---
**Fail rule:** if a goal can't produce evidence, it's a wish with better formatting — it doesn't pass.
