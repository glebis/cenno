# Goal Contract — Concurrent-prompt queueing per policy

> Source of truth. Agents may propose **Goal Amendments**; they may not silently rewrite this. Generated spec/plan/tasks under `generated/` are compiled views.

**Date:** 2026-06-16 · **Feature factory pilot #1** · Factory spec now lives in its own project: `~/ai_projects/feature-factory` (`docs/spec.md`; this pilot's case study at `pilots/concurrent-prompt-queueing/`).

## Current state
When a second prompt arrives while one is already visible, the panel **replaces** the visible prompt. The first prompt becomes **unanswerable until it times out** — its answer is effectively lost and the requesting agent is left hanging. (`backlog.md`: "Concurrent-prompt queueing per policy — UI currently replaces the visible prompt; first prompt becomes unanswerable until timeout.")

## Desired future state
Concurrent prompts are **queued**, not clobbered. The visible prompt stays answerable; when it resolves (answered/skipped/timed-out) the **next queued prompt surfaces in order**. Each answer is routed back to the agent that asked. No prompt is silently dropped.

## Current constraint (Theory of Constraints)
The **single visible-prompt slot** is the bottleneck: only one prompt can be answered at a time, and a new arrival currently destroys the in-flight one. The fix adds a queue in front of that slot — it does *not* try to widen the slot (no simultaneous multi-prompt display).

## Target user / job (JTBD)
One or more agents ask the human questions via `ask_user`. The human's job: **answer each question without losing any of them**, even when they arrive close together.

## Non-negotiable constraints
- Local-first, no network (cenno invariant); `cenno.db` stays `0600`.
- **No prompt or answer is dropped** under concurrency.
- The existing single-prompt path is **unchanged when there is no contention** (no regression).
- Deterministic, explainable ordering (the "policy").
- Answer routed to the correct requesting agent (`get_response` returns the right answer to the right caller).

## Desired outcomes (solution-independent, measurable)
1. Under N concurrent prompts, **0 dropped**: each is eventually shown and answerable, or explicitly resolved (answered / skipped / timed-out).
2. Resolving the visible prompt **surfaces the next** per policy within one event cycle.
3. An answer that arrives for a queued/late prompt is **stored and returned** to its caller (not dropped).

## Success evidence
- Rust unit tests (in `src-tauri`): enqueue ordering, surfacing-on-resolve, no-drop under concurrency, late-answer storage+retrieval, eviction bound.
- Deterministic `factory verify`: `cargo test` (src-tauri) + lint + build green.
- Manual scenario: fire two `ask_user` prompts back-to-back; answer the first → second appears → answer it; both agents receive correct answers.

## Visual checkpoints
Light UI: after answering, the next queued prompt appears (no blank flash). A queue-depth affordance is **out of scope** for the pilot (defer to the tray-popover backlog item).

## Risk classification
**R1 — internal dev-assist / local tool.** No user-facing AI decisions about people, no sensitive/behavioral data beyond locally-stored prompts the user authored, no profiling. EU AI Act Art 5 (prohibited use): N/A. Art 50 (labelling): N/A (no AI-generated content surfaced). No transparency/labelling obligations. → record & continue; no external review needed.

## Risks
- **Ordering/policy ambiguity** — FIFO vs priority vs device-hint. Pilot: FIFO by arrival, with the existing policy field honored if present; document the chosen rule.
- **Unbounded queue/pending growth** — backlog flags "pending-map eviction" growing unboundedly; pilot must bound it (cap + eviction of resolved/expired).
- **Late answers dropped** — backlog: "Store late answers in `Pending` so `get_response` can return them (currently dropped; resolve() returns false)." In scope as outcome #3.
- **Cross-IPC ordering race** (panel hide/show) — known edge; don't regress.

## Non-goals
- Simultaneous multi-prompt display.
- Cross-device queueing / routing.
- Persistence of the queue across app restart (unless trivially free).
- Tray-popover inbox UI (separate backlog item).

## Release constraints
- Must not regress the single-prompt path or fullscreen/pause quiet-mode replay.
- No new network calls. Keep `eprintln!`/tracing conventions as-is.
