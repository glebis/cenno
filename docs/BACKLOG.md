# Cenno backlog

Idea inbox — not commitments. During revenue focus (through Sep 2026), items here get logged, not built.

## 2026-07-07 — Predefined actions / deep links (from Gleb, voice)

Agents should be able to attach predefined actions to a prompt, e.g.:
- return a link or deep link into an app (example: open generated text in a text editor like Zed)
- optionally auto-launch that action when the user answers

Shape sketch: an `actions: [{label, url_or_command, auto?: bool}]` field on AskRequest; answered prompt fires the action. Also fixes a real UX gap observed today: agent had to round-trip a whole extra prompt just to learn "open it in Zed".

Known timing bug (same session): a second ask was sent before the user's typed follow-up to the first arrived — answers can race the next prompt. Consider draining/attaching late input to the previous prompt_id.

## 2026-07-13 — General voice assistant direction (from Gleb, voice)

Evolve cenno from "agents ask questions" into a general voice surface for agents: pass
information out and collect it in, integrated with MCPs. Spokenly-style dictation chains
already work (panels accept any input; native push-to-talk exists via voice.rs).

Architecture decision sketch — keep cenno a *peripheral*, not a brain:
- Cenno stays local-only (no LLM, no API keys — preserves the AGENTS.md privacy contract).
- A separate always-on agent daemon (Claude Agent SDK / persistent Claude Code session) is
  the brain; it holds keys and connects to other MCPs (mail, calendar, etc.).
- Same shape as the telegram-telethon daemon: surface ↔ daemon, agent in the middle.

Missing halves, in wedge order:
1. `tell_user` MCP tool — one-way outbound (voice/panel, no answer expected). Mostly reuse
   of ask_user minus answer plumbing; composes with urgency/quiet-mode/TTS as-is.
2. User-initiated direction — global hotkey → dictate (voice.rs) → utterance queued on a
   local socket / `wait_for_user_message` long-poll tool → daemon picks it up and responds
   via tell_user/ask_user. First daemon can literally be `claude -p` per utterance.

Everything past the wedge (session continuity, multi-agent routing, "universal app")
waits for evidence of daily use. Revenue-focus rule applies: logged, not built.

## 2026-07-08 — Media display / player in prompts (from Gleb, voice)

Cenno should be able to present media files inline in a prompt — audio, video, images, animations — instead of the agent shelling out to `afplay`. Motivating case (HeyClicky video edit): the agent generated several candidate title SFX and had to play them via `afplay` on a loop while the user picked blind by ear, with no way to replay a specific one on demand.

Shape sketch: a prompt can attach a list of media items, each rendered with an inline play/pause toggle (for audio/video) or preview (image/animation). The user can replay any item individually, and select or react to any one — or all at once (e.g. "pick A", "reject all", per-item thumbs). Part of a larger media-display capability, not audio-only.

Fields sketch: `media: [{id, kind: audio|video|image|animation, url_or_path, label}]` on AskRequest; answer carries which item(s) were selected/reacted to. Composes with `choices` (each choice could bind to a media item) and with the `actions` idea above.

Addendum 2026-07-13 (from Gleb, voice): images should come with *controls*, not just display —
per-image select/react, zoom/compare, and pairing with input widgets (e.g. rate each image on
a slider). Current state: A2UI catalog already has a display-only `Image` component plus
`Scale`/`Slider`; what's missing is (a) interactive image affordances (select, zoom, compare
grid), and (b) serving *local* image files into the webview (agents have paths, not URLs —
needs Tauri asset-protocol scoping, a security surface to design deliberately).

Descope 2026-07-13 (Gleb): adjustable/interactive image controls are NOT a priority. Fixed
scaling stands as decided: existing variant caps (icon 24 / avatar 44 / smallFeature 80 /
mediumFeature 120 / largeFeature+header 160, max-width 100%), recommended default
`fit: contain` + `mediumFeature`, `largeFeature` when the image is the subject of the
question. Document this in the widget-advisor guide (cenno-bee). The only remaining app-code
piece under this idea is local-file image serving (scoped asset protocol) — kept on cenno-88n.

## 2026-07-13 — Widget-advisor guidance in the cenno skill (from Gleb, voice)

The `skills/cenno/SKILL.md` teaches mechanics (kinds, A2UI payloads) but not *judgment*:
which widget fits which question. Add a decision guide — confirm for yes/no, choice ≤5
options, scale for intensity/satisfaction, slider for continuous ranges, voice_text for
open-ended, media grid for "pick one of these artifacts", sequences for check-ins.
Docs-only, no app code — buildable during revenue focus without breaking the freeze.
