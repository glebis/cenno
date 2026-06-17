# Goal Brief — Progress clamping (S-tier)

Tier: **S** · Risk: **R1** (internal robustness) · Date: 2026-06-17 · Tracker: none

**Intent:** Make cenno's MCP `Progress` robust to nonsensical agent values
(`step>total`, `step=0`, `total=0`) by clamping on deserialize, so the panel's
dot pagination can never render wrong.

**Success evidence:** `cargo test --lib` green with protocol cases
`{5,3}→{3,3}` · `{0,5}→{1,5}` · `{0,0}→{1,1}`; existing tests still pass.
→ **Met:** 103/103 lib tests green.

**Stop condition:** if clamping can't be isolated to the protocol deserialize
layer (forces touching call-sites / UI render) → stop, re-scope to M.
→ **Held:** isolated to `src-tauri/src/protocol.rs` only.
