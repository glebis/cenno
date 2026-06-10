# Visual QA — panel states vs Reporter frames (Plan-2 Task 9)

Captured with `scripts/visual-qa.sh` (release binary, 420x240 panel,
`screencapture -l <window-id>`, 2x Retina → 840x480 PNGs). Reference frames
are raster comps — per TOKENS.md, type metrics come from TOKENS.md, not from
measuring the images.

Three capture rounds: round 1 found the gaps, round 2 applied inline catalog
fixes. Round 3 (the `qa-*.png` files committed here) is the post-polish state
after the visual-polish branch landed Tasks 1–4: panel chrome (wordmark + ✕),
bare-word mood, bottom-pinned dots, and quiet bottom-right Send.

## Per-state verdicts (post-polish, visual-polish branch)

| Capture | Reference frame | Verdict |
|---|---|---|
| qa-mood.png | panel-mood-checkin.png | **Matches.** Coral surface, `cenno` wordmark top-left + dim ✕ top-right, question-m title, and the five choices render as **bare oversized words in a single row** (Awful · Bad · Okay · Good · Great) — no pill chips. This is the frame. Only deviation: title + words are left-aligned where the frame centers them (deliberate panel convention, applies to every state). ✕ does not overlap the wordmark or content. |
| qa-text.png | panel-free-text.png | **Matches.** Cobalt, chrome present, underline-only field with dim "Your reply" placeholder, and a **quiet text "Send" pinned bottom-right** — no primary pill. Deviates only where voice is out of scope: the frame shows a mic circle in the field and "type or speak" (voice = plan 3); a 3+ line body still scrolls (structural, backlog §1). |
| qa-scale.png | fullscreen-ema-1-scale.png | **Matches design language** (frame is fullscreen, ours is the 420×240 panel — treatment, not geometry). Chrome present, outlined 44px circle targets 1–7 with the selected one ringed, dim "not at all"/"completely" end labels, and the **pagination dots are pinned to the bottom edge** (content sits at the top, dots at the bottom, step 1-of-3 active). Deviates: no top "CHECK-IN — 1 OF 3" caption (the panel carries the wordmark instead; caption is content not styling — backlog §1). |
| qa-confirm.png | panel-reminder.png | **Matches direction, intentionally unchanged.** Slate surface, chrome present, **white primary pill + quiet secondary still in pill/quiet pair form** — the quiet-Send treatment from Task 4 correctly did NOT bleed into confirm actions. Deviates: labels are Yes/No (the `confirm` kind is binary; the frame's Done/Snooze/Dismiss are protocol concepts that don't exist yet). |

### Inline fix this round
None. All four polished states render cleanly against their frames and no
cheap CSS gap was spotted (no ✕/content overlap, no flicker-resize on the
short confirm/mood titles, dots pin reliably). The one cross-state deviation —
left-aligned vs the frames' centered title/actions — is a deliberate panel
convention, not a regression, and re-centering globally is not a safe ≤20-min
change (would touch every state's layout); left for a design decision in
backlog.

### Capture note
The panel's text field autofocuses; if the terminal sends typeahead during the
~4s render wait it leaks into the field (round-3 first text capture caught a
stray "wr"). Re-captured clean. Not an app bug — a capture-harness artifact.

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

## Done ✓ (closed by the visual-polish branch, verified this round)

- **Panel chrome** (was backlog §2): `cenno` wordmark top-left + dim ✕ top-right
  now render on every panel state and the ✕ resolves the prompt as dismissed.
  Confirmed in all four captures; no overlap with title/content.
- **Mood choice treatment** (was backlog §3): mood flow now renders bare
  oversized words in one row instead of pill chips. Confirmed in qa-mood.png.
- **Dots pinned to the bottom edge** (was backlog §5): the surface column fills
  the panel height, pagination dots sit at the bottom edge across steps.
  Confirmed in qa-scale.png.
- **Send placement** (was backlog §6): text prompts now show a quiet text
  "Send" bottom-right instead of a primary pill bottom-left. Confirmed in
  qa-text.png; the quiet treatment correctly does NOT apply to confirm pills.

## Backlog (still open)

1. **Window height vs content (structural):** 240px fits title + 2-line body +
   input + button at question-m, but longer bodies scroll. Want
   content-driven `set_size` (Rust) before showing the window. Also covers the
   EMA "CHECK-IN — 1 OF 3" header caption (frame top-center) — desugar could
   synthesize it from `flow`+`progress`, but the wording is content, not
   styling, so it needs a protocol decision alongside the height work.
2. **Centered vs left-aligned (design decision):** the Reporter frames center
   title/actions; the panel left-aligns every state. Consistent across our
   states and arguably cleaner at panel size — left as a deliberate convention
   pending a design call, not a defect.
3. **Top-level navigation hardening (Rust):** links are now intercepted in
   the catalog, but a rich `a2ui` payload could still inject an anchor via a
   future component; a WebView navigation handler denying non-app URLs would
   close the class.
4. **visual-qa.sh / capture polish:** expired prompts keep the panel visible
   (Dismiss state), so the between-state wait runs its full budget; and the
   autofocused text field can swallow terminal typeahead during the render
   wait. Both are harness artifacts — captures are correct on retry.

## Gates

- `cargo test` — 28 + 2 passed, 1 ignored (requires live app), 0 failed
- `npx vitest run` — 5 files, 49 tests passed (includes updated desugar
  table tests and the new external-link regression test)
- `npm run build` — clean (tsc + vite, tokens regenerated)
