# Concurrent-prompt queueing per policy — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface concurrent prompts in a deterministic **priority order** — a High-urgency prompt jumps ahead of older Normal/Low ones — instead of strict arrival order, so the most urgent queued question is shown next when the visible slot frees.

**Architecture:** The frontend (`src/App.tsx`) **already queues** ("Queue, don't steamroll" guard + `advanceOrHide` shows `pending()[0]`). The only "per policy" gap is server-side ordering: `PromptRegistry::pending()` (`src-tauri/src/registry.rs`) sorts strictly FIFO by arrival id. We reorder it by **(urgency rank, arrival id)**. `Urgency` (High/Normal/Low) is the policy signal — no new protocol field. Pure-Rust, no frontend change, no async/timeout changes.

**Tech Stack:** Rust, tokio, parking_lot::Mutex, serde; `cargo test` in `src-tauri/`.

**Goal Amendment (recorded):** The Goal Contract assumed the UI clobbers the visible prompt. Exploration shows the frontend already queues; the backlog item is stale on that point. Pilot scope is therefore narrowed to the genuine remaining gap — **policy (urgency) ordering**. The other Goal outcomes — **late-answer storage** (`resolve()` drops late answers) and **eviction bound** — are real but touch the async resolve/timeout invariants; they are **deferred to backlog follow-ups** to keep the pilot a clean, low-risk slice. See `../factory/2026-06-16-concurrent-prompt-queueing/goal.md`.

---

### Task 1: Track the work in beads (the loop's epic+issues phase)

**Files:** none (creates `.beads/` store).

- [ ] **Step 1: Init the beads store**

Run:
```bash
cd /Users/glebkalinin/ai_projects/cenno
bd init
```
Expected: a `.beads/` database is created (`bd where` shows the resolved workspace).

- [ ] **Step 2: Create the epic + child issue**

Run:
```bash
EPIC=$(bd q "Feature-factory pilot: concurrent-prompt queueing per policy")
TASK=$(bd q "Urgency-priority ordering in PromptRegistry::pending()")
bd link "$TASK" "$EPIC" 2>/dev/null || bd note "$EPIC" "child: $TASK"
bd list
```
Expected: `bd list` shows both beads. (If `bd link`'s dependency semantics differ, the `bd note` fallback records the parent→child relationship; either is acceptable for the pilot.)

- [ ] **Step 3: Commit the beads store**

```bash
git add .beads
git commit -m "chore(factory): track concurrent-prompt-queueing pilot in beads"
```

---

### Task 2: Urgency-priority ordering in `pending()` (TDD)

**Files:**
- Modify: `src-tauri/src/registry.rs` (the `pending()` sort, ~line 165; imports; a new `urgency_rank` helper)
- Test: `src-tauri/src/registry.rs` `#[cfg(test)]` module (existing tests start ~line 171)

- [ ] **Step 1: Add the test helper + failing test**

In the `#[cfg(test)] mod tests` block of `src-tauri/src/registry.rs`, add a helper next to the existing `req()` (line ~174) and a new test. The helper builds an `AskRequest` at a given urgency (mirrors `req()`'s JSON style; `Urgency` is `rename_all = "lowercase"`):

```rust
fn req_urgency(u: &str) -> AskRequest {
    serde_json::from_str(&format!(
        r#"{{"title":"t","timeout_s":1,"urgency":"{u}"}}"#
    ))
    .unwrap()
}

/// Enqueue Normal, High, Low (in arrival order); pending() must return them
/// ordered by urgency (High → Normal → Low), not by arrival.
#[tokio::test]
async fn pending_orders_by_urgency_then_arrival() {
    let reg = PromptRegistry::new();
    for u in ["normal", "high", "low"] {
        let reg2 = reg.clone();
        let r = req_urgency(u);
        tokio::spawn(async move { reg2.ask(r, |_id, _req| {}).await });
        // small gap so ids are assigned in arrival order deterministically
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let order: Vec<String> = reg
        .pending()
        .into_iter()
        .map(|(_, r, _)| format!("{:?}", r.urgency))
        .collect();
    assert_eq!(order, vec!["High", "Normal", "Low"]);
}
```

- [ ] **Step 2: Run the test — verify it FAILS**

Run: `cd src-tauri && cargo test --lib pending_orders_by_urgency_then_arrival -- --nocapture`
Expected: FAIL — order is `["Normal", "High", "Low"]` (current FIFO-by-id sort), asserting against `["High", "Normal", "Low"]`.

- [ ] **Step 3: Implement urgency-priority ordering**

In `src-tauri/src/registry.rs`, ensure `Urgency` is imported (the `use crate::protocol::...` line already imports `AskRequest`, `Via`; add `Urgency`):

```rust
use crate::protocol::{AskRequest, Urgency, Via};
```

Add a free helper above `impl PromptRegistry` (or near `pending`):

```rust
/// Queue policy: lower rank surfaces first. High(0) → Normal(1) → Low(2).
fn urgency_rank(u: &Urgency) -> u8 {
    match u {
        Urgency::High => 0,
        Urgency::Normal => 1,
        Urgency::Low => 2,
    }
}
```

Replace the sort line (currently `v.sort_by_key(|(id, _, _)| id.strip_prefix("p_")...)`) with a composite key — urgency first, then arrival id:

```rust
v.sort_by_key(|(id, req, _)| {
    (
        urgency_rank(&req.urgency),
        id.strip_prefix("p_").and_then(|n| n.parse::<u64>().ok()),
    )
});
```

- [ ] **Step 4: Run the test — verify it PASSES**

Run: `cd src-tauri && cargo test --lib pending_orders_by_urgency_then_arrival`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/registry.rs
git commit -m "feat(registry): order pending prompts by urgency, then arrival"
```

---

### Task 3: Lock the "High interrupts older Normals" case + full verify

**Files:**
- Test: `src-tauri/src/registry.rs` `#[cfg(test)]`

- [ ] **Step 1: Add the interrupt-ordering test**

```rust
/// Two Normals already queued; a newly-arrived High must surface first.
#[tokio::test]
async fn high_urgency_surfaces_before_older_normals() {
    let reg = PromptRegistry::new();
    for u in ["normal", "normal", "high"] {
        let reg2 = reg.clone();
        let r = req_urgency(u);
        tokio::spawn(async move { reg2.ask(r, |_id, _req| {}).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let first = reg
        .pending()
        .into_iter()
        .next()
        .map(|(_, r, _)| format!("{:?}", r.urgency));
    assert_eq!(first, Some("High".to_string()));
}
```

- [ ] **Step 2: Run the new test — verify it PASSES**

Run: `cd src-tauri && cargo test --lib high_urgency_surfaces_before_older_normals`
Expected: PASS (implementation from Task 2 already satisfies it; this locks the behavior).

- [ ] **Step 3: Run the full registry + socket suites — verify NO regression**

Run: `cd src-tauri && cargo test --lib registry:: && cargo test --test mcp_socket`
Expected: PASS — especially `unshown_prompt_never_times_out_and_keeps_full_budget` and `resolve_completes_ask` (they assert single-prompt behavior, unaffected by ordering).

- [ ] **Step 4: Lint/format gate (`factory verify` equivalent)**

Run: `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: clean. (If `cargo clippy` flags pre-existing unrelated warnings, fix only what this change introduced; note any pre-existing ones.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/registry.rs
git commit -m "test(registry): lock high-urgency-interrupts-normals ordering"
```

---

## Evidence / Definition of Done
- [ ] `pending_orders_by_urgency_then_arrival` and `high_urgency_surfaces_before_older_normals` pass.
- [ ] Existing registry + `mcp_socket` tests still pass (no regression of the unshown/no-timeout invariant).
- [ ] `cargo fmt --check` + `cargo clippy -D warnings` clean for the changed file.
- [ ] Goal traceability: outcome #2 (resolve surfaces next *per policy*) is met; outcomes #1/#3 (no-drop, late-answer) tracked as deferred follow-ups.
- [ ] Commits on `feat/feature-factory`.

## Deferred to backlog (Goal Amendment)
- Late-answer storage so `resolve()`/retrieval doesn't drop answers for a timed-out prompt (touches async resolve path).
- Pending-map eviction bound (cap + evict resolved/expired).
- A richer `policy` enum (drop-on-conflict / LIFO) if a real need appears — YAGNI for now; urgency ordering covers the pilot.

## Self-review
- **Spec coverage:** spec's "single-focus TDD", "deterministic verify", "no regression" → Tasks 2–3. "epic+issues (beads, conditional)" → Task 1. Risk R1 recorded in goal. Late-answer/eviction explicitly deferred (documented, not silently dropped).
- **Placeholders:** none — all test/impl code is concrete; the one tolerant branch (`bd link` fallback) is justified inline.
- **Type consistency:** `urgency_rank(&Urgency)`, `req.urgency: Urgency`, `pending() -> Vec<(String, AskRequest, u64)>`, `req_urgency(&str) -> AskRequest` consistent across tasks.
