# Feature-Factory — goal-driven, local-first feature pipeline (pilot + bake-off)

**Date:** 2026-06-15
**Status:** Design draft, awaiting review
**Scope:** A repeatable way to plan + implement a cenno feature. Proven first as a **bake-off pilot** on one real backlog feature; generalized later only if it earns it.

## Goal

A repeatable workflow to take a feature from **intent to shipped**, baking in: TDD, checklists, standardized docs, epic+issues, an ethics/risk screen, and prioritization. The human works at the **goal** level (desired end-state + constraints); agents compile and maintain the downstream docs. The whole thing stays **interactive and goal-driven up to approval**, then runs as a **supervised** (not autonomous) implementation loop.

This spec deliberately scopes the **first deliverable as a pilot + bake-off**, not a finished engine — per the converged research (see References). We prove the loop on one feature, compare candidate toolchains, then adopt the winner and build only the small novel glue.

## Background / why this shape

Five research streams (two local agents + three GPT-5.5 Pro threads, archived at `~/Brains/brain/ai-research/20260615-feature-factory-research/`) converged on:

1. **Don't build a bespoke engine first.** Off-the-shelf systems already implement most of the loop — **Superpowers** (owned), **GSD Core** (`Discuss→Plan→Execute→Verify→Ship`), **Spec Kit / OpenSpec**. A standalone config engine is "bureaucracy with a plugin architecture" until proven.
2. **No end-to-end autonomy / parallel swarm for coding** (~15× tokens, breadth-first, poor fit). Single focused TDD loop; humans at cheap-to-change (goal) and irreversible (merge) moments.
3. **Every gate is a gamed proxy** — agents reward-hack tests, people game RICE, orgs ethics-wash. Anchor verification on compiler/tests/linter + **Playwright evidence**, never agent opinion.
4. **Goal-as-source, spec-as-compiled-artifact.** Human approves a **Goal Contract**; agents generate spec/plan/tasks. EARS stays in the compiler layer.

## Non-goals (YAGNI)

- **No global/reusable engine yet.** cenno-local first; extract to a skill only after 5–10 features.
- **No config-driven `pipeline.config` / executor abstraction yet** — premature; revisit post-pilot.
- **No parallel multi-agent implementation** (Agent Teams / fan-out) for the coding phase.
- **No separate "ethics gate" phase** — risk is folded into the Goal Contract/spec.

## Design

### The loop
```
Goal Contract (human-approved)         ← source of truth
  → compile: spec + plan + tasks        ← candidate SDD tool (bake-off)
  → epic + issues (beads / `bd`)        ← conditional; epic = parent bead, tasks = child beads, deps = `bd link`
  → TDD implement (Red → Green)         ← single focused loop, no swarm
  → verify (deterministic) + evidence   ← `factory verify` + Playwright
  → human review (diff)
  → ship (PR / merge)
  → record (CLAUDE.md / changelog / risk note)
```

### Goal Contract (`goal.md` — the human source of truth)
A short markdown doc the human writes/approves. Agents may propose **Goal Amendments** but never silently rewrite it.
```
# Goal Contract
## Current state            ## Desired future state
## Current constraint (ToC) ## Target user / job (JTBD)
## Non-negotiable constraints
## Desired outcomes (solution-independent, measurable)
## Success evidence         ## Visual checkpoints (if UI)
## Risk classification (below)
## Risks  ## Non-goals  ## Release constraints
```
**Fail rule:** *if a goal can't produce evidence, it's a wish with better formatting* — it doesn't pass.

### Risk classification (ethics folded in, not a separate gate)
Inside the Goal Contract, classify and apply rules:
- **R0** no AI / no meaningful risk → record & continue
- **R1** internal dev-assist only → record & continue
- **R2** user-facing AI, low-stakes → transparency note + tests for user-visible behavior
- **R3** sensitive/behavioral data, recommendations, profiling, education/productivity outcomes → explicit mitigation + logging/monitoring + rollback + human review
- **R4** possibly prohibited/high-risk (EU AI Act Art 5) or needs legal review → **stop**; do not implement until externally reviewed
Plus the two near-term binding EU checks: not an Art 5 prohibited use; Art 50 labelling of AI-generated/chatbot/deepfake content where applicable. Recorded as **self-assessed** (the known weakness); value is catching issues cheaply at goal time.

### Gates (2 required + 1 conditional)
- **Required — Goal+Risk approval** (cheap-to-change moment).
- **Conditional — Plan approval**, only if the feature: is >1 day, or touches auth/billing/PII/permissions/prompts/model-behavior/data-retention/migrations/prod, or changes public API/UX, or has irreversible user-data effects.
- **Required — Review + merge** (irreversible moment).
Interactive gate UI = cenno `ask_user` MCP (approve/deny).

### Verify (deterministic spine, not hooks-as-truth)
One project command does the work; the hook only *calls* it: `local hook → same command as CI → review`.
```
factory verify  →  test · lint · typecheck · build · secrets-scan · prompt-injection check
factory evidence →  Playwright screenshots/visual-diff (UI goals) · property tests (fast-check/Hypothesis) · Goal Traceability table
```
Plus **fitness functions**: no unauthorized deps; no public API without a contract; no migration without a rollback note; no new AI behavior without a transparency note; visual snapshots approved.

### Side-effect-aware manifest (`run.json`)
Resume is **not** `if status != completed` — it's a side-effect ledger. Per phase: `status`, `input_hash`, `output_paths`, `external_ids` (beads epic/issue ids), `git.{start_sha,end_sha}`, `approved_by/at`. Prevents duplicate epics / stale stacking on resume.

### Guardrails (baked in)
- Green suite necessary, never sufficient → human at review + merge.
- No same-model critic as truth → verification anchors on tests/compiler/Playwright.
- **Cost control:** max planning turns, max implementation loops before human interrupt, no parallel subagents unless approved, summarize before handoffs, record per-run token/cost.
- **Secrets:** none in prompts/logs/commits/artifacts; no `.env`/prod creds in agent context.
- **Prompt-injection boundary:** for any untrusted text the feature ingests, separate tool calls from content, quote/sandbox, validate outputs before actions.
- **Drift:** CLAUDE.md holds only durable behavior; review runs a drift-check (diff vs goal, acceptance criteria changed?, tasks reflect actual work?).

### Artifacts / persistence
```
docs/superpowers/factory/<date>-<slug>/
  goal.md                              # human source of truth
  generated/{spec,plan,tasks}.md       # compiled views (tool output)
  evidence/{verify.log, screenshots/, goal-traceability.md, review.md}
  run.json                             # side-effect ledger
```
Reuses cenno's existing `docs/superpowers/` trail + `plans/backlog.md` ledger.

## The bake-off (the actual v0 deliverable)

### Pilot feature (confirmed)
**Concurrent-prompt queueing per policy** (from `plans/backlog.md`): today the UI replaces the visible prompt and the first becomes unanswerable until timeout. Real, bounded, logic-heavy with light UI — exercises the full loop.

### Candidate stacks (same feature, same Goal Contract, same model, same timebox)
- **A — Superpowers** (current baseline you already run).
- **B — GSD Core** (`open-gsd/gsd-core`) — closest off-the-shelf phased loop.
- **C — Goal Contract → OpenSpec** (primary) / **Spec Kit** (comparison) as the compiler, + Superpowers TDD + Playwright evidence.
Run on a disposable branch / worktree per stack.

### Scoring rubric (per stack)
time-to-usable-plan · clarification loops · # generated artifacts · first verification pass-rate · human corrections needed (babysitting tax) · goal-deviation count · tests-written-before-impl · diff quality (minimal/idiomatic) · context stability · evidence completeness · token/cost · cross-agent portability · **"would I reuse this?"**. Process scorecard: **DORA** (lead time, deploy freq, change-fail, recovery, rework) + **SPACE-lite** (satisfaction/flow) so we don't optimize into "faster but exhausted."

### Decision criteria (what we adopt after)
- OpenSpec gives enough structure at low friction → adopt as compiler backend.
- Spec Kit catches more ambiguity / better tests, worth the overhead → adopt with Goal-Contract presets.
- Both too much ceremony → keep Superpowers + steal only templates/checklists.
- GSD Core clearly best for phased work → adopt for larger features.
- Playwright catches real issues → make it mandatory for UI goals.

## What we build vs adopt
- **Adopt:** the winning compiler/methodology stack; Superpowers TDD; **beads (`bd`)** for epic/issues; Playwright; fast-check/Hypothesis.
- **Build (small, novel):** Goal Contract template + scoring harness; risk-classification rubric; `factory verify`/`evidence`/`drift-check` wrappers; side-effect-aware `run.json`; beads wiring (`bd init`; epic→parent bead, tasks→child beads via `bd create`/`link`).
- **Defer:** the generic engine + per-project config + executor abstraction (only after 5–10 features).

## Success criteria
1. The pilot feature ships via the loop with tests-first, deterministic verify, and a human-reviewed diff.
2. The bake-off produces a scored, written recommendation of which stack to standardize on.
3. The Goal Contract + risk rubric + verify/evidence wrappers exist and were actually used.
4. A written go/no-go on whether to invest further (extract glue, eventually a skill).

## Risks
- **Bake-off overhead** eclipses the feature — mitigate with a strict timebox and a small pilot.
- **Tooling churn** (Spec Kit/OpenSpec command names drift) — pin versions; verify before quoting.
- **Self-assessed risk = theater** — accepted; documented as self-review, R4 forces a real stop.
- **Goal Contract becomes spec-by-another-name** — guard with the "must produce evidence" fail rule and solution-independence check.

## Resolved
- **Pilot feature:** concurrent-prompt queueing per policy (confirmed).
- **Issue tracking:** Linear deferred; use **beads (`bd`)** — git-native, dependency-aware. No `.beads` store in cenno yet, so `bd init` is the first wiring task. Epic+issues is conditional for the pilot (only if the feature decomposes into >1 tracked task).

## References
Vault: `~/Brains/brain/ai-research/20260615-feature-factory-research/` — `[[00-index]]`, `[[01-software-factory-approaches]]`, `[[02-build-vs-adopt-workflow-systems]]`, `[[03-chatgpt-design-audit]]`, `[[04-chatgpt-goal-driven-sdd]]`, `[[05-chatgpt-deep-research-dev-os]]`.
