# Diff re-audit — codex (gpt-5.5), 2026-06-24 (round 2)

Re-review of the round-1 fixes. Codex independently ran typecheck + vitest + `cargo test --lib` (all green) and reported:

- **#2 (empty-clear), #3 (keepalive-on-restore), #4 (startup ordering): confirmed fixed.**
- **#1 (cross-prompt leak): only partially fixed.** Content-only fingerprinting still leaks when a new launch reuses the same id AND identical title/body (e.g. a recurring "How are you feeling?" mood prompt restoring a days-old draft).
- **NEW MINOR:** a literal NUL byte in `App.tsx:108` (the fingerprint separator) — makes git/`rg` treat the file as binary.

## Resolutions (round 2)

| Finding | Fix |
|---------|-----|
| #1 still partial | **Session-namespaced draft keys.** A per-launch nonce (`DRAFT_SESSION = crypto.randomUUID()`) is baked into every draft key (`cenno-draft-<session>-<id>`). Within one app session prompt ids are unique, so restore is exact; across launches (where ids collide) prior-session drafts live under a different key and are **unmatchable**. A `sweepForeignDrafts()` at module load removes any draft key from a different session so stale text can never surface and storage can't accumulate. The content fingerprint is kept as defense-in-depth. |
| NEW NUL byte | Confirmed real (`od -c` showed `\0` at the separator — an earlier `grep -P` gave a false negative). Stripped the NUL and replaced the fingerprint with `JSON.stringify([title, body_md])` — collision-free, pure ASCII. |

Verified post-fix: `npx tsc --noEmit` clean · `npm run typecheck:tests` clean · `npx vitest run` 148 passed · `cargo test --lib` 108 passed. New tests cover the reused-id/fingerprint-mismatch and type-then-clear cases.
