# Visual QA — panel states vs Reporter frames (Plan-2 Task 9)

Captured with `scripts/visual-qa.sh` (release binary, 420x240 panel,
`screencapture -l <window-id>`, 2x Retina → 840x480 PNGs). Reference frames
are raster comps — per TOKENS.md, type metrics come from TOKENS.md, not from
measuring the images.

Two capture rounds: round 1 found the gaps, fixes were applied inline, round 2
(the `qa-*.png` files committed here) shows the post-fix state.

## Per-state verdicts (post-fix)

| Capture | Reference frame | Verdict |
|---|---|---|
| qa-mood.png | panel-mood-checkin.png | **Matches direction.** Coral surface, question-m title, white text, ≥44px tap targets. Deviates: choices render as outline pill chips (the shared ChoicePicker treatment from panel-choice.png), while the mood frame shows bare oversized words in a single row. Also no `cenno` wordmark / close ✕ chrome (all frames have it; the panel has none yet). |
| qa-text.png | panel-free-text.png | **Matches direction.** Cobalt, question-m title, underline-only field with dim placeholder, Send fully visible. Deviates: Send is a white primary pill bottom-left, frame shows a quiet text "Send" bottom-right; mic circle only appears for `voice`/`voice_text` kinds (frame always shows it — voice is plan 3); a 3+ line body will still scroll. |
| qa-choice.png | panel-choice.png | **Matches.** Outline pill chips, body-size labels, cobalt surface — the closest state to its frame. Chips wrap to two rows at 420px where the frame fits four in one row (frame's chips are smaller-padded); acceptable at this window size. |
| qa-scale.png | fullscreen-ema-1-scale.png | **Matches design language** (frame is fullscreen, ours is the 420x240 panel — compared treatment, not geometry). Outlined 44px circle targets with numerals, dim end labels under the row ends, centered dot pagination with active-step emphasis. Deviates: no top "CHECK-IN — 1 OF 3" caption; dots sit after content, not pinned to the bottom edge; title is question-m not question-l (correct for the panel surface per TOKENS.md). |
| qa-confirm.png | panel-reminder.png | **Matches direction.** Slate surface, white primary pill + quiet text secondary side by side — same shape language as Done/Snooze/Dismiss. Deviates: labels are Yes/No (the `confirm` protocol kind is binary; Snooze is a protocol-level concept that doesn't exist yet). |

## Gaps found in round 1 → classification → action

| Gap | Class | Action |
|---|---|---|
| Panel titles rendered at question-l (44px): desugar emitted `variant:"h1"` → `question-l`, contradicting TOKENS.md ("type.question.m = panel questions"). Title ate half the panel and pushed Send/choices off the bottom. | desugar variant fix | **Fixed** — desugar emits `h2` → `question-m` (22px). h1/question-l stays reserved for future fullscreen surfaces. |
| **Clicking a markdown link navigated the entire webview to the linked page** (round-1 captures show the panel replaced by example.com — CSP does not cover top-level navigation; the React app is gone until restart). | catalog component fix (small) | **Fixed** — `TextView` renders links with preventDefault + `openUrl` from `@tauri-apps/plugin-opener` (`opener:default` already allows http/https). Regression test added. |
| Markdown link color: UA default blue, invisible on cobalt. | catalog CSS fix | **Fixed** — `.cenno-text a` inherits the flow text color, hairline underline. |
| Send button clipped at 420x240 with body text. | mostly a consequence of the 44px title | **Fixed by the title fix** for 1–2 line bodies (round 2: Send fully visible). Longer bodies still scroll — see backlog (content-driven window height is structural). |
| Buttons stretched to full-width slabs (Column cross-axis stretch); confirm Yes/No stacked vertically. Frame shows hug-content pills in a row. | desugar + catalog CSS fix | **Fixed** — confirm buttons wrapped in an `actions` Row in desugar; `.cenno-button { align-self: flex-start; border-radius: pill }` (panel-reminder.png "Done" is a pill, and radius.control=10 is specified for inputs/cards, not buttons). |
| Scale rendered bare numerals with underline-on-select; frame shows outlined circle targets. | catalog CSS fix | **Fixed** — 44px circle outline per target, numeral at body size, selected = filled circle with flow-color numeral, hover = full-opacity border. |
| Scale overflow at narrow width (known item). | n/a at this window | Not reproduced at 420px: 7×44px circles + gaps ≈ 352px, fits the 372px content width. Would overflow below ~400px window width — backlog note only. |
| WebKit text-selection artifact on title autofocus (known item). | n/a | **Not reproduced** in either round on the release build. |

## Backlog (not ≤30min token/CSS/desugar fixes)

1. **Window height vs content (structural):** 240px fits title + 2-line body +
   input + button at question-m, but longer bodies scroll. Want
   content-driven `set_size` (Rust) before showing the window.
2. **Panel chrome (structural):** every frame shows a `cenno` wordmark
   (top-left, caption style) and a close ✕ (top-right). The panel has no
   chrome; ✕ should resolve the prompt as dismissed.
3. **Mood choice treatment (structural):** mood frame uses bare oversized
   words in one row, not pill chips — needs a flow-aware ChoicePicker variant,
   not a CSS tweak.
4. **EMA header caption (structural):** frame shows "CHECK-IN — 1 OF 3" top
   center; desugar could synthesize it from `flow`+`progress`, but the
   wording is content, not styling — needs a protocol decision.
5. **Dots not pinned to the bottom edge:** frame fixes pagination at bottom
   center across steps; ours flows after content. Needs the surface column to
   fill the panel height (renderer wrapper markup, not plain CSS on .cenno-dots).
6. **Send placement:** frame has a quiet bottom-right "Send"; ours is a
   primary pill bottom-left. Revisit with the window-height work (1).
7. **Top-level navigation hardening (Rust):** links are now intercepted in
   the catalog, but a rich `a2ui` payload could still inject an anchor via a
   future component; a WebView navigation handler denying non-app URLs would
   close the class.
8. **visual-qa.sh polish:** expired prompts keep the panel visible (Dismiss
   state), so the between-state wait always runs its full budget and warns;
   captures are correct because each new prompt replaces the surface.

## Gates

- `cargo test` — 28 + 2 passed, 1 ignored (requires live app), 0 failed
- `npx vitest run` — 5 files, 49 tests passed (includes updated desugar
  table tests and the new external-link regression test)
- `npm run build` — clean (tsc + vite, tokens regenerated)
