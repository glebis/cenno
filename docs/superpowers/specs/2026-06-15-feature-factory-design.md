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
A **short** markdown doc the human writes/approves — capped so it stays a goal, not waterfall-in-markdown. Agents may propose **Goal Amendments** but never silently rewrite it.
```
# Goal Contract
## Current state (≤3)        ## Desired future state (≤3)
## Current constraint (ToC)  ## Target user / job (JTBD)
## Non-negotiable constraints (≤5)
## Desired outcomes (solution-independent, measurable; ≤5)
## Smallest shippable slice   ← what ships first without the whole cathedral
## Stop condition             ← "if X, stop and ask for plan approval"
## Success evidence (≤5)     ## Visual checkpoints (only if user-visible UI changes)
## Risk classification (below) ## Rollback note (revert / flag / toggle / N-A)
## Risks (≤5)  ## Non-goals (≤5)  ## Release constraints
```
**Fail rule:** *if a goal can't produce evidence, it's a wish with better formatting* — it doesn't pass. **Smallest shippable slice** and **Stop condition** are required — they're the main guard against process outrunning the feature.

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

### Visual QA (evidence for real UI changes — right-sized)
Unit tests miss layout (the pilot's own margin/pin/landscape bugs were only caught by eye), so capture visual evidence — but only when it earns its keep, and minimally. (Round-2 audit: the matrix-by-default version was "a small bureaucracy wearing Playwright cosplay.")
- **Trigger by change, not by tier:** **blocking** only when the change touches user-visible **layout, styling, onboarding, or auth/safety flows**; otherwise **advisory** (informs review, doesn't block); pure backend/logic → none.
- **Smallest representative evidence:** the *changed flow* at **one primary viewport** by default. Widen to the device/orientation/theme matrix only when the Goal Contract says those surfaces matter, or the bug is known to vary by them.
- **Capture:** `scripts/visual-qa.sh` (macOS panel) · `simctl` (iOS) · Playwright `toHaveScreenshot()` (web/Tauri). Baseline-snapshot updates are approved **in the diff/PR review**, not as a separate gate.
- **Cull is escalated, not default:** use it for substantial UI/design changes worth real review; for routine changes a screenshot in `evidence/` + the diff review is enough.

### Side-effect-aware manifest (`run.json`)
Resume is **not** `if status != completed` — it's a side-effect ledger. Per phase: `status`, `input_hash`, `output_paths`, `external_ids` (beads epic/issue ids), `git.{start_sha,end_sha}`, `approved_by/at`. Prevents duplicate epics / stale stacking on resume.

### Guardrails (baked in)
- Green suite necessary, never sufficient → human at review + merge.
- No same-model critic as truth → verification anchors on tests/compiler/Playwright.
- **Cost control:** max planning turns, max implementation loops before human interrupt, no parallel subagents unless approved, summarize before handoffs, record per-run token/cost.
- **Secrets:** none in prompts/logs/commits/artifacts; no `.env`/prod creds in agent context.
- **Prompt-injection boundary:** for any untrusted text the feature ingests, separate tool calls from content, quote/sandbox, validate outputs before actions.
- **Drift:** CLAUDE.md holds only durable behavior; review runs a drift-check (diff vs goal, acceptance criteria changed?, tasks reflect actual work?).
- **External validators get a hard timeout + fallback:** codex/any external tool runs under a wall-clock cap with a self-validation fallback — never silently block on it (pilot #1: codex hung 12 h overnight, then 28 min).
- **Contract change ⇒ all call-sites verified:** changing a shared function's contract requires enumerating every caller / parallel path and proving each honors it (pilot #1: codex caught `pick_replay` bypassing the new `pending()` policy).
- **Determinism:** no wall-clock-dependent test assertions — use synchronous barriers/callbacks (pilot #1: codex flagged a `20ms`-sleep ordering test).
- **Evidence is an artifact, not a claim:** `verify.log` + codex verdict + screenshots + Goal-Traceability persisted under `evidence/` so "done" is auditable.

## Baseline rules (the constitution)
**A compiler target, not a second job.** These are mostly **machine-enforced defaults inside `factory verify`** — not a scroll the solo dev recites before fixing a bug. Push as much as detectable into the one command; leave only judgment to the human.

**Always automated (`factory verify`):** fmt · lint/clippy · typecheck · test · build · secrets-scan · dependency-policy · no-timing/sleep-based-tests (where detectable) · migration ⇒ rollback-note (if migrations) · public-API change ⇒ contract test (if API changed) · AI-behavior change ⇒ transparency note (if prompts/model changed). One command, identical local & CI.

**Sometimes automated (generated, used when relevant):** visual screenshots · Goal-Traceability draft · contract/property tests (only for a clear invariant/API) · drift-check · generated review summary.

**Human only:** approve the Goal Contract wording · risk-classification sanity/override · judge whether a plan is too invasive (risky work) · approve baseline-screenshot changes · review the final diff · decide whether the shipped slice is enough.

**Independent/adversarial validation** (codex or fresh-context subagent) is **not default** — invoke it only for **non-trivial diffs, shared contracts, or auth/data/risk areas**; anchored on objective signals, never agent opinion; under timeout + fallback.

**Cross-cutting fitness functions (fail the gate):** green suite ≠ sufficient (human at merge) · every Goal outcome maps to evidence (UI ⇒ visual evidence) · contract change ⇒ all call-sites verified · no migration without rollback · no new AI behavior without a transparency note · no flaky/timing-based tests · no new secrets in prompts/logs/commits.

### Feature-size tiers + process budget (the tripwire)
Process scales to **size**, not just risk. When the process exceeds the feature, stop feeding the beast.
- **S** (<½ day, low-risk, no public API/migration): Goal Contract + TDD + `factory verify` + merge. No plan, no beads, no adversarial review, no bake-off paperwork.
- **M** (1–2 days, some UI/integration): + plan if it grows · beads if 2+ real tasks · visual evidence if UI changes · adversarial review if non-trivial.
- **L** (multi-day; shared contracts, migrations, auth/billing/permissions, AI behavior, data retention): full gates, plan approval, adversarial review, rollback, visual matrix if UI.
- **XL**: do **not** run through v0 — split first.

**v0 budget (tripwire, not sacred):** Goal Contract ≈15–30 min · ≤3 core generated docs · visual ≤1 changed flow / 1 viewport by default · external validator **once**, timeout-bound · human gates = goal approval + final merge (plan approval only if a risk/size trigger fires).

### Flake policy
A flaked test is a **failure**, not a retry-until-green. No quarantine; require a deterministic fix before merge. (Otherwise "deterministic verify" is aspirational poetry.)

### Post-ship retro (tiny, mandatory)
After each feature, 4 lines: what slowed shipping? · what caught a real bug? · which artifact was never used? · **what gets deleted before the next feature?** The deletion question is the point — process accumulates like dust.

### Artifacts / persistence
```
docs/superpowers/factory/<date>-<slug>/
  goal.md                              # human source of truth
  generated/{spec,plan,tasks}.md       # compiled views (tool output)
  evidence/{verify.log, screenshots/, visual-review.md, codex-verdict.md, goal-traceability.md, review.md}
  run.json                             # side-effect ledger
```
Reuses cenno's existing `docs/superpowers/` trail + `plans/backlog.md` ledger.

## The bake-off (the actual v0 deliverable)

### Pilot feature (confirmed)
**Concurrent-prompt queueing per policy** (from `plans/backlog.md`): today the UI replaces the visible prompt and the first becomes unanswerable until timeout. Real, bounded, logic-heavy with light UI — exercises the full loop.

### Candidate stacks — baseline vs ONE challenger (not a 4-way fashion show)
Round-2 audit: running 3–4 stacks costs more than the feature and biases toward the nicest paperwork. So:
- **Baseline — Superpowers** (what you already run).
- **One challenger — Goal Contract → OpenSpec** (or GSD Core), as the compiler + Superpowers TDD + visual evidence.
- Add a **third only if** the challenger clearly fails or leaves a question open.
Run on a disposable branch / worktree per stack.

### Scoring rubric (brutally short)
Did it: clarify the goal? · reduce rework? · produce better tests *before* impl? · keep the diff smaller/safer? · **slow me down?** · **would I use it again next week?** The last one is the most honest metric — everything else is supporting evidence. (DORA/SPACE-lite are optional context, not the decision.)

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
