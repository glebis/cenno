# Cenno — Design Spec

*2026-06-10. Status: approved pending user review.*

**Cenno** (Italian: *fare un cenno* — to give a small sign, to beckon) is a macOS desktop runtime that lets AI agents interact with the user on the OS level in a time-sensitive manner. Agents summon UI surfaces — floating panels, fullscreen takeovers, tray popovers — through MCP tools or a CLI, and receive the user's answer (voice, text, or choice) as the tool result.

The first consumer is quantified-self check-ins triggered by app context, but cenno itself is a generic runtime: any agent that speaks MCP can use it.

## Goals

- Any MCP-capable agent (Claude Code, Claude Desktop, scheduled agents) can ask the user a question on screen with zero custom integration — one MCP config entry.
- Voice answers work out of the box with **no external apps**: local whisper.cpp by default, BYOK (Groq, OpenAI) configurable.
- Surfaces are beautiful and consistent: one token-driven component catalog, themable via design tokens.
- The user stays in control: urgency→surface mapping is runtime policy the user configures, not something agents dictate.

## Non-goals (v1)

- Windows/Linux support (Tauri keeps the door open; nspanel behavior is macOS-only).
- Native notifications tier (later; cheapest escalation rung).
- Arbitrary HTML surfaces (declarative-only by design — security and consistency).
- Context *sensing* (frontmost-app tracking lives in agents/scripts that call cenno, not in cenno itself — cenno is the output/input runtime, not the tracker).

## Architecture

The cull pattern: **one Tauri 2 process, no separate daemon.**

| Mode | Invocation | Behavior |
|---|---|---|
| App | `cenno` | Main window (settings, history) + socket server |
| Tray | `cenno --tray` | Headless, surfaces ready, menubar icon |
| MCP stdio bridge | `cenno --mcp-stdio` | If socket missing, auto-launches app in tray mode, waits, then proxies stdin/stdout ↔ Unix socket |
| CLI | `cenno ask "..." --surface panel` | Direct tool execution, prints result as JSON |

- **MCP server**: `rmcp` crate on a Unix socket at `{app_data_dir}/mcp.sock`. Optional token-gated HTTP/SSE server (off by default) for remote agents later.
- **Storage**: SQLite — interaction history, pending prompts, settings, tokens.
- **Frontend**: React in the Tauri webview (chosen over Svelte because the A2UI React renderer is stable; see Rendering).
- **Rust core**: window/surface management, whisper.cpp binding, socket server, job/timeout handling.

## MCP contract

Four tools in v1:

### `ask_user`

```jsonc
{
  "title": "Quick check-in",
  "body_md": "You've been in **Figma** for 2h. What are you working on?",
  "input": { "kind": "voice+text" },        // text | voice | voice+text | choice | scale | confirm | none
  "choices": null,                           // for kind: choice
  "urgency": "normal",                       // low | normal | high
  "timeout_s": 120,
  "a2ui": null                               // optional full A2UI component list
}
```

Blocks until answered or timed out. Returns `{ answer, via: "voice"|"text"|"choice", elapsed_s }` or `{ answered: false, prompt_id }`. Timed-out prompts land in the tray inbox; a late answer is retrievable via `get_response(prompt_id)`.

### `show_surface`

Fire-and-forget display (status, ambient info). Same content fields, no input. Returns `surface_id` for later dismissal. (In-place updating of a live surface is deferred to post-v1; agents re-show instead.)

### `dismiss_surface(surface_id)`

### `get_response(prompt_id)`

Retrieves a late answer for a prompt that timed out into the tray inbox. Returns the same shape as `ask_user`, or `{ answered: false }` if still pending.

### Content contract — the hedge

The simple fields (`title`, `body_md`, `input`, `choices`) are the default path; agents never need A2UI knowledge. The optional `a2ui` field accepts a full A2UI v0.9 component list for rich layouts. **Internally the simple form desugars into A2UI** and everything renders through one path. The pre-1.0 A2UI spec dependency stays contained in one field and one transform.

## Surfaces

| Surface | Mechanism | When |
|---|---|---|
| Floating panel | `tauri-plugin-nspanel` non-activating, always-on-top; never steals keyboard focus; voice/text reply inline | Default for normal urgency |
| Fullscreen takeover | Separate window, `fullscreen: true` | High urgency only |
| Tray popover | Menubar popover: pending prompts + history | Inbox for missed/low urgency |

**Escalation is runtime policy, not agent choice.** Agents declare `urgency`; the user-configurable policy maps urgency → surface (defaults: low → tray badge, normal → panel, high → fullscreen). Rationale: a fullscreen takeover the user didn't sanction feels hostile; policy keeps trust.

macOS platform facts the design relies on:
- Own-window fullscreen/always-on-top/tray need **no OS permission**.
- Focus-stealing from background is restricted (Sonoma+ cooperative activation) — the non-activating panel sidesteps it entirely.
- Microphone permission prompts on first voice use (standard, unavoidable).

## Visual design direction (added 2026-06-10, user)

**Not a cull clone.** cull is the *process architecture* reference only (socket/CLI/bridge); cenno shares zero UI with it. The visual reference is **Reporter App** (Nicholas Felton, [App Store](https://apps.apple.com/de/app/reporter-app/id779697486)): full-bleed solid color, one question per screen, large quiet typography, minimal chrome, dot pagination for multi-step flows, instant tap-to-answer.

Primary planned experiences (these drive the component catalog in plan 2):
- **EMA questionnaires** (ecological momentary assessment): short multi-step flows — scale, choice chips, free text/voice — with dot progress
- **Mood check-ins**: single-screen, one tap or one sentence, optionally voice
- **Reminders**: glanceable, dismiss/snooze/done, no typing required

**Optional ambient noise sampling**: like Reporter's "SILENCE 18.71 dB" metric — when the user opts in, a check-in may sample background audio level (dB estimate only, no recording stored) and attach it to the response metadata. Requires the same Microphone permission voice input already needs; strictly opt-in per policy settings. Deferred to the voice plan (plan 3) since it shares the audio capture stack.

## Rendering & design primitives

Three independently replaceable layers:

1. **Protocol** — A2UI JSON (what agents say).
2. **Component catalog** — our own React components registered as the A2UI renderer's trusted catalog (Card, Text, Button, TextField, Choice, Scale, VoiceInput, layout containers). The stable A2UI React renderer hosts them; we do not ship Google's default catalog.
3. **Design tokens** — DTCG `tokens.json` compiled by Style Dictionary → CSS custom properties. Per-surface themes (panel/fullscreen/tray) are token-set overrides on shared primitives.

**Spike (first task of implementation):** half a day validating the React renderer's custom-catalog + theming API against A2UI v0.9 before committing the catalog design. Fallback if it disappoints: render the desugared component list with our own thin interpreter (the flat-list format is simple enough to walk directly).

## Voice

- **Local default**: whisper.cpp via Rust binding, Metal-accelerated; small model bundled, larger models downloadable in settings.
- **BYOK**: provider trait with implementations for Groq and OpenAI (and room for more); selected in settings, keys in macOS Keychain.
- Interaction: tap-to-talk button in any surface whose `input.kind` includes voice; transcription shown live for confirmation before submit (configurable auto-submit).

## Data & history

SQLite tables: `prompts` (full interaction log: agent, content, answer, via, timestamps), `surfaces` (active fire-and-forget surfaces), `settings`, `mcp_tokens` (for the HTTP server, cull-style: SHA256 secret hash, role, scopes, audit log).

History is the quantified-self payoff: every answered check-in is a timestamped record queryable by agents via a future `query_history` tool (post-v1).

## Error handling

- Agent disconnects mid-prompt → prompt auto-dismisses (configurable: keep in inbox).
- Timeout → `{ answered: false, prompt_id }`, prompt moves to tray inbox.
- Whisper failure / no mic permission → surface falls back to text input with an inline notice; error returned in `via` metadata.
- Malformed `a2ui` payload → tool error with validation details (agent can retry); never a blank surface.
- Concurrent prompts → queued per policy (one panel at a time; queue visible in tray badge).

## Testing

- Rust: unit tests for desugaring transform (simple form → A2UI), policy mapping, timeout/queue logic.
- Contract tests: golden JSON fixtures for each tool's request/response.
- React catalog: component tests with token themes applied (panel vs fullscreen snapshots).
- E2E smoke: `cenno ask` CLI → socket → render → scripted answer → JSON result.

## Naming in docs

The fullscreen takeover mode is nicknamed **mano a borsa** (🤌, "che vuoi?!") in docs and code comments; the panel is *un cenno*. The 🤌 hand is the logo brief.

## Open questions (deferred, not blocking)

- `query_history` tool shape (post-v1).
- HTTP/SSE remote access UX (token issuance flow) — schema is ready, surface later.
- Multi-display behavior for the panel (default: display with cursor).
