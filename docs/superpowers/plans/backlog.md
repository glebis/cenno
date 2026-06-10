# Backlog (consolidated from walking-skeleton reviews)

Carried out of the plan-1 final review (2026-06-10). Each item names the plan that owns it.

## Plan 3 — voice
- Opt-in ambient noise sampling (dB estimate only, no recording stored) attached to response metadata — shares the audio capture stack and Microphone permission

## Plan 4 — surfaces, policy, history
- Observe `context.ct` (client cancellation) in ask_user so a dead agent unparks the prompt (TODO in mcp.rs)
- Single-instance enforcement (tauri-plugin-single-instance) vs the socket unlink race (TODO in mcp.rs)
- Pending-map eviction/persistence — grows unboundedly today (doc on PromptRegistry)
- Store late answers in `Pending` so `get_response` can return them (currently dropped; resolve() returns false)
- Concurrent-prompt queueing per policy — UI currently replaces the visible prompt; first prompt becomes unanswerable until timeout
- Real tray icon + popover inbox/history (the --tray flag is reserved and honored but adds no icon yet)
- Timed-out prompt lingers on screen — frontend gets no timeout signal yet
- Drag region for the panel (decorations:false means it can't be moved)
- Switch eprintln! sites to tracing once a subscriber exists
- Panel chrome (structural): every frame shows a `cenno` wordmark (top-left, caption style) and a close ✕ (top-right); ✕ should resolve the prompt as dismissed
- Content-driven window height: `set_size` from Rust before showing window so longer bodies don't scroll
- Mood choice treatment: mood frame uses bare oversized words in one row — needs a flow-aware ChoicePicker variant, not a CSS tweak
- EMA header caption: frame shows "CHECK-IN — 1 OF 3" top center; desugar could synthesize from `flow`+`progress` but wording needs a protocol decision
- Dots pinned to bottom edge: frame fixes pagination at bottom center; currently flows after content (needs surface column to fill panel height)
- Send placement: frame has a quiet bottom-right "Send"; ours is a primary pill bottom-left — revisit with the window-height work
- Rust-side navigation hardening: a rich a2ui payload could still inject an anchor via a future component; a WebView navigation handler denying non-app URLs would close the class
- Panel: drop title h1 vs fullscreen variants — revisit type roles when fullscreen surface exists (plan 4)

## Cleanup
- Drop the `test_support` alias in mcp.rs once live_socket_probe.rs imports `mcp::client`

## Known edge (panel hide/show)
- Cross-IPC ordering: a hide IPC landing strictly between Rust's order_front_regardless and JS event delivery could still bury a fresh prompt (JS generation guard can't see it). Fix candidate: Rust-side re-show on emit, or a frontend "shown" ack. Rare; revisit in plan 4.
