# Cross-device prompt routing ("second screens")

**Status:** accepted (2026-06-15)
**Branch:** `feat/cross-device-routing`

## Problem

`ask_user` prompts already reach the Watch/iPhone companion over CloudKit
(`mcp.rs:126` → `relay::write_prompt`, verified on a physical device). But the
behavior is unconditional and undirected: every prompt is published with
`device_hint: "any"` and shown everywhere, "first-to-answer-wins". There is no
way to say *which* device a prompt should reach, or to use a phone/iPad as a
deliberate **second screen** that mirrors agent prompts while you work at the
Mac.

We want two behaviors on top of the existing transport:

- **Presence-based fallback** — a device shows a prompt only when the Mac
  hasn't dealt with it (overflow).
- **Explicit second screen** — a device mirrors prompts live, even while you're
  at the Mac.

## Decisions (from brainstorming)

- **A1 — Mac resolves policy; devices self-filter.** No presence/heartbeat
  subsystem. The Mac stamps a resolved routing decision onto each prompt;
  devices read it and decide whether/when to display.
- **Agent proposes, user disposes.** The agent's `device_hint` is a proposal;
  the user's global policy is the final authority. The hint can only *narrow*
  within what the user allows — it can never enable an `off` device, nor
  escalate a `fallback` device to `mirror`.
- **Liveness via foreground polling, not APNs.** The "second screen" is a
  dedicated, app-foreground, fullscreen ambient display. It polls every
  15–30 s. No `CKQuerySubscription`/APNs in this MVP.
- **Sound mandatory, TTS optional.** On a prompt surfacing, a bundled sound
  plays; optionally the title is spoken via `AVSpeechSynthesizer` (no network).

## Architecture

### 1. Routing model

The Mac performs all policy resolution and stamps two new fields onto the
`Prompt` CloudKit record (the existing `device_hint` is retained as the agent's
raw proposal, for debugging):

- **`route`** — `"mirror"` | `"fallback"`
  - `mirror`: eligible devices surface immediately.
  - `fallback`: eligible devices surface only after a grace delay —
    `now ≥ created_at + fallback_grace_s` and still `pending`.
- **`targets`** — comma-joined eligible device classes, e.g. `"iphone,ipad"`.
  A device displays a prompt iff its own class ∈ `targets`.

Device classes: `mac`, `iphone`, `ipad`, `watch`. The Mac always shows its
local panel as today; `route`/`targets` govern only the companion devices.

### 2. Policy resolution (Mac)

User global policy lives in `~/.cenno/config.json` under `"routing"`:

```jsonc
{
  "iphone": "off" | "fallback" | "mirror",   // default: fallback
  "ipad":   "off" | "fallback" | "mirror",   // default: off
  "watch":  "off" | "fallback" | "mirror",   // default: off
  "fallback_grace_s": 20,                      // default: 20
  "allow_agent_hint": true                     // default: true
}
```

Pure function `resolve_route(policy, agent_hint) -> (route, targets)`:

1. Eligible classes = those whose mode ≠ `off`. **An `off` class is a hard
   gate — never enabled by an agent.**
2. If `allow_agent_hint` and `agent_hint` names an *eligible* class, narrow
   `targets` to just that class. The hint filters within what's allowed; it
   never widens, never promotes.
3. `route` = `mirror` if any surviving target class has mode `mirror`, else
   `fallback`.

Example: iphone=`fallback`, ipad=`mirror`. Hint `"ipad"` → `route=mirror`,
`targets="ipad"`. No hint → `route=mirror`, `targets="iphone,ipad"` (iphone
still obeys its own fallback grace — see note). Hint `"phone"` when
iphone=`off` → ignored.

> Note: when `targets` mixes a `mirror` class and a `fallback` class, `route`
> is set to `mirror` (the louder wins) but each device additionally consults
> its *own* class mode via the per-class map carried in `targets`. To keep the
> record flat, we encode per-class mode directly: `targets` becomes a
> comma-joined list of `class:mode` pairs, e.g. `"iphone:fallback,ipad:mirror"`.
> Devices parse their own entry. `route` is dropped in favor of this; the
> single `targets` field is authoritative.

**Revised record shape (final):** one field, `targets`, e.g.
`"iphone:fallback,ipad:mirror"`. `fallback_grace_s` is carried as its own field
`grace_s`. This removes the `mirror`-vs-`fallback` ambiguity for mixed targets.

### 3. Device side — ambient "second screen" mode

A per-device toggle (local `UserDefaults`, independent of Mac policy): *"Act as
a second screen."* When ON:

- App shows a fullscreen **ambient view**: idle = clock or smiley face.
- A foreground **poll loop** runs every `pollIntervalS` (default 20 s, range
  15–30) calling the existing `CloudKitRelay.fetchPending()`.
- Fetched prompts are filtered by pure function
  `shouldSurface(record, deviceClass, now) -> Bool`:
  - parse this device's `class:mode` entry from `record.targets`; absent →
    `false`.
  - `mode == mirror` → surface now.
  - `mode == fallback` → surface iff `now ≥ created_at + record.graceS`.
- On a prompt **first** surfacing: play a bundled sound (mandatory); optionally
  speak `record.title` via `AVSpeechSynthesizer` (TTS toggle, default off).
- Answering is unchanged (writes `state=answered`); other surfaces drop it on
  their next poll (≤ one interval).

When OFF: today's behavior exactly (manual pull-to-refresh, no polling).

Per-device local settings: `secondScreenEnabled`, `pollIntervalS`,
`soundEnabled` (default on), `ttsEnabled` (default off), `ambientFace`
(clock | smiley).

### 4. Dedup, testing, scope

- **Race window:** answering on the Mac may leave a prompt visible on a second
  screen for up to one poll interval before it clears. Acceptable for MVP.
- **Testing:**
  - Rust: table tests for `resolve_route` (policy × hint → targets+grace).
  - Swift: table tests for `shouldSurface` (class × mode × grace × now),
    mirroring the existing `A2UIRouter` test style.
  - Manual: ambient mode on a simulator/device, fire `demo.sh` under each
    policy, confirm grace timing + sound + TTS.
- **Out of scope (YAGNI):** APNs/`CKQuerySubscription` (deferred — only needed
  to wake a *backgrounded* device), presence/heartbeat, a Mac-side
  "push this one to phone" button.
- **iPad:** the iOS app running on iPad, self-reporting class `ipad` via
  `UIDevice` idiom. No separate iPad target.

## Build sequence

1. Rust `resolve_route` + `RoutingPolicy` config type (TDD).
2. Wire resolver into `relay::write_prompt` / `mcp.rs` (pass real targets+grace
   instead of `"any"`).
3. Mac writer `CennoRelay.swift`: write `targets` + `grace_s` fields.
4. `PromptRecord`: add `targets` (parsed `[class:mode]`) + `graceS`; CloudKit
   read/write.
5. Swift `shouldSurface` pure function (TDD).
6. Device ambient mode: view, poll loop, sound, TTS, per-device settings.
7. Build, test, screenshot ambient mode, import to Cull.
