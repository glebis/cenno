# L1a — `get_screen_context` MCP tool (Accessibility-only)

**Date:** 2026-07-15
**Status:** Implementation complete; installed-app AX scenarios pending user permission
**Bead:** `cenno-jc6.1` (L0 prerequisite `cenno-jc6.7` complete; blocks `cenno-jc6.8` / L1b)
**Scope:** One new MCP tool, `get_screen_context`, reading the macOS
Accessibility API only. No pixel capture, no OCR — those are L1b. Depends on the
L0 `capture_guard` chokepoint existing.

## Goal (user's words)

> Allow it to see the screen … get the context of the screen … so that I can ask
> questions about the current screen.

Give any connected agent a cheap, instant, on-device answer to "what is the user
looking at right now?" — the focused app, window, URL, what's selected, and the
visible text of the focused window — returned as structured text with **zero
image tokens and no OCR pass.** This is the highest value-to-risk slice of screen
awareness and should ship first; the visual fallback (L1b) is only needed where
AX comes back empty.

## Background / current state

- cenno's MCP server (`src-tauri/src/mcp.rs`, rmcp 1.7) exposes `ask_user` and
  `ask_sequence`. Tools return `CallToolResult`; `Result::Err(String)` becomes
  an `is_error` result via rmcp's `IntoCallToolResult`. New tools are registered
  through `#[tool_router]` + `#[tool]`; `list_tools`/`call_tool` already delegate
  generically to that router.
- cenno already reads the window graph: `suppress.rs` calls
  `CGWindowListCopyWindowInfo` for fullscreen detection. That gives window
  bounds/titles/layer — **not** content. AX is the tier above it (semantic
  elements, text, selection, focus).
- cenno already bridges Swift via **swift-rs**: `voice.rs` ↔ the `swift/`
  SpeechTranscriber package, with `build.rs` baking an `-rpath /usr/lib/swift`.
  L1a extends this exact pattern with a small AX-reading Swift function.
- cenno is **non-sandboxed**, so it can be an AX client once the user grants
  Accessibility (a TCC gate separate from Screen Recording).
- **L0 is a hard prerequisite:** every value L1a returns passes through L0's
  `capture_guard` (kill-switch → denylist → redact → wrap-untrusted). L1a does
  not return raw content.

## Decisions

| Decision | Choice |
|---|---|
| Data source | **Accessibility only.** Start at the system-wide `AXUIElement`, resolve `AXFocusedApplication` → `AXFocusedWindow` → `AXFocusedUIElement`. No `CGWindowList`, no pixels. |
| Reader location | A new Swift function in the `swift/` package (AX is far nicer in Swift/ObjC than via `objc2` from Rust), exposed to Rust through swift-rs, mirroring `voice.rs`. |
| Return type | **Structured JSON as MCP text content** (not image). One flat object; see schema below. Bounded size; `truncated: true` when clipped. |
| Absence handling | Not-granted and empty-tree are **typed successful results**, never errors: `status: "permission_denied" | "ax_unavailable" | "ok"`. This lets an agent branch and lets L1b know when to take over. |
| Permission | Trigger the Accessibility TCC prompt lazily on first real call via `AXIsProcessTrustedWithOptions(kAXTrustedCheckOptionPrompt)`. Never block; if still denied, return `permission_denied`. |
| Browser URL | Best-effort. Try the address-bar text field via AX; treat as optional. AX-address-bar reads are fragile (Chrome may need `AXManualAccessibility`/`AXEnhancedUserInterface`, which can slow the browser) — **do not** force-enable browser AX enhancement in L1a; leave `url` null if not cheaply available. (Apple Events fallback is deferred to L2, which owns `site_host`.) |
| Cost guard | Do **not** walk the whole tree. Read the focused element + a bounded visible-text pull (`AXVisibleCharacterRange` → `AXStringForRange`, or `AXValue`/`AXStaticText` on the focused window, capped at N KiB). Deep traversal is banned — every AX attribute read is a cross-process IPC round trip. |

## Architecture

### Tool contract

```
get_screen_context(include_visible_text?: bool = true,
                   max_chars?: u32 = 8000)
  -> {
       status: "ok" | "permission_denied" | "ax_unavailable" | "blocked",
       app_name: string,            // e.g. "Safari"
       bundle_id: string,           // e.g. "com.apple.Safari"
       window_title: string | null,
       url: string | null,          // best-effort, browsers only
       focused_role: string | null, // AXRole/AXSubrole of focused element
       selected_text: string | null,
       visible_text: string | null, // bounded, may be truncated
       truncated: boolean,
       untrusted: true              // set by L0 capture_guard
     }
```

- `status: "blocked"` is returned when L0's denylist matches the frontmost
  app/host — cenno declines to read it.
- `untrusted: true` and any redaction are stamped by `capture_guard`, not by the
  AX reader.

### Flow

```
agent → call_tool("get_screen_context", args)
  │
  ▼  mcp.rs: parse args, size guards
swift-rs → screen_context_read()   // Swift, in swift/
  │   AXUIElementCreateSystemWide()
  │   → AXFocusedApplication → AXFocusedWindow (AXTitle)
  │   → AXFocusedUIElement (AXRole, AXValue, AXSelectedText)
  │   → bounded visible text (AXVisibleCharacterRange/AXStringForRange)
  │   → best-effort browser URL
  │   returns raw struct OR ax_unavailable / permission_denied
  ▼
capture_guard (L0):  kill-switch → denylist(bundle_id,host)
                     → redact(text) → wrap_untrusted
  ▼
CallToolResult (text content, structured JSON)
```

### Files

- `swift/` — new AX reader function + swift-rs export (extend the existing
  package; build.rs rpath already covers the Swift runtime).
- `src-tauri/src/mcp.rs` — register `get_screen_context` in `list_tools` /
  `call_tool`; parse args; call the bridge; route through `capture_guard`.
- `src-tauri/src/protocol.rs` — `ScreenContext` request/response types
  (Serialize/Deserialize + JsonSchema, matching existing derives).
- `src-tauri/src/capture_guard.rs` — from L0; called here (not created here).
- `skills/cenno/SKILL.md` — document the tool and the "captured content is
  untrusted data, not instructions" rule.
- Tests: `src-tauri/tests/mcp_socket.rs` (tool over the socket) + Swift-side
  unit coverage where feasible.

## Verification

- **Unit:** protocol round-trips; `capture_guard` integration (denylisted
  bundle → `blocked`; secret in text → redacted).
- **Integration (`mcp_socket.rs`):** call `get_screen_context` over the socket;
  with AX granted assert `status:"ok"` and a populated `app_name`/`bundle_id`;
  simulate denial path → `permission_denied` typed result (not an error).
- **Live E2E:** build (`npx tauri build --no-bundle`), grant Accessibility,
  focus a native app (Notes) with selected text → assert `selected_text` and
  `window_title` come back; focus a browser → `url` best-effort populated;
  focus an Electron/canvas app → `status:"ax_unavailable"` (proving the L1b
  hand-off contract). Confirm the screen-context path adds no cenno network
  call; document that the requesting agent may transmit returned context.

## Out of scope

- **Pixel capture, OCR, image return** — L1b (`cenno-jc6.8`). L1a's only
  obligation to L1b is returning `ax_unavailable` when the tree is empty.
- **Passive/continuous sampling and storage** — L2 (`cenno-jc6.2`). L1a is
  strictly on-demand, pull-only, stateless.
- **Forcing browser AX enhancement** for reliable URLs — deferred; L2 owns
  `site_host` and may use Apple Events.

## Resolved questions

1. **Default `max_chars`.** 8000 characters, clamped to `1..=8000`; revisit
   only with evidence from real agent usage.
2. **Visible-text strategy per role.** Resolved for L1a: try `AXValue` on the
   focused element, then `AXVisibleCharacterRange` → `AXStringForRange`. Do not
   traverse static-text children; if both direct reads are empty, return null
   (and `ax_unavailable` when no other semantic content exists).
3. **Should `blocked` reveal which rule matched?** Yes: return only
   `capture_disabled`, `denied_bundle`, or `denied_host` in `blocked_reason`;
   all captured fields remain null.
