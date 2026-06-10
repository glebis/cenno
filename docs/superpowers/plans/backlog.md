# Backlog (consolidated from walking-skeleton reviews)

Carried out of the plan-1 final review (2026-06-10). Each item names the plan that owns it.

## Plan 2 — rendering & tokens
- Validate the A2UI `version` field at the MCP boundary (the renderer silently ignores it — spike finding)
- Stock A2UI `Text` needs a `MarkdownContext` provider (or use our own Text component)
- Tighten `csp: null` in tauri.conf.json before exposing any remote content
- Visual direction: Reporter-app minimalism (see spec "Visual design direction") — full-bleed color, one question per screen, dot pagination; EMA / mood check-in / reminder flows drive the catalog
- Extract DTCG tokens.json from the design work (palette, type scale, spacing, radius)

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
- Markdown links in prompts should open via tauri-plugin-opener, not navigate the panel webview
- Drag region for the panel (decorations:false means it can't be moved)
- Switch eprintln! sites to tracing once a subscriber exists

## Cleanup
- Drop the `test_support` alias in mcp.rs once live_socket_probe.rs imports `mcp::client`
