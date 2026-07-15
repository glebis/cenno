# L0 — Screen-capture security & threat model

**Date:** 2026-07-15
**Status:** Implemented and verified
**Bead:** `cenno-jc6.7` (blocks `cenno-jc6.1` / L1a)
**Scope:** Threat-model, consent, and data-handling policy for the whole
context-awareness epic (`cenno-jc6`). No feature code ships here except the
capture indicator + kill switch and the denylist/redaction library that L1a/L1b
call. Everything in L1a/L1b/L2 is built *on top of* the decisions in this doc.

## Goal

Make screen awareness safe to ship without breaking the promises cenno is
built on: **screen capture is processed and stored locally by cenno, adds no
new network path, and remains under user control.** Screen capture is a
categorical expansion of cenno's data surface —
it turns a Q&A relay into something that can read your documents, messages, and
URLs — so the safety story must land *before* the first capture tool, not after.

Concretely, L0 delivers: (1) an updated threat model covering the two new risks
screen capture introduces, (2) a denylist + redaction library that L1a/L1b must
call before returning or storing any captured content, (3) a visible
capture/sampling indicator and a global kill switch, and (4) a precise,
non-overclaimed privacy statement in `README.md` and `SECURITY.md`.

## Background / current state

`SECURITY.md` today models three things as the attack surface: the local
machine, the MCP socket, and **agent-supplied prompt content coming *in***
(`a2ui` payloads, validated at `a2ui_guard.rs`). Its load-bearing sentence is:

> cenno is a local-first Tauri 2 menu-bar app. All answer history stays on disk.

The current document also says cenno makes no network connections except the
updater. That is already stale: the source contains a CloudKit relay and an
optional SpeechTranscriber model download. Screen awareness adds a different
data flow the current model does not cover: **captured screen content flowing
*out* to the calling agent.** Two consequences:

1. The existing model treats agent content as untrusted input to cenno. It has
   no concept of cenno handing untrusted *screen* content to an agent.
2. The capture subsystem can make no network calls and still hand content to
   an agent whose model is remote. If we let anyone read the privacy story as
   "your screen never leaves your machine," that is the exact overclaim we documented Hey Clicky making
   (its "Privacy-first — no passive recording" marketing versus the
   `activity-timeline.sqlite` its binary ships). We must not repeat it.

cenno is **non-sandboxed** (hardened runtime, Developer-ID; confirmed in
`src-tauri/Entitlements.plist`), so it is eligible to be an Accessibility client
and to use ScreenCaptureKit once the user grants the (separate) TCC permissions.

## Threats introduced by screen awareness

### T1 — Prompt injection via captured screen content (new, serious)

`get_screen_context` and the L2 activity timeline read text from whatever is on
screen (a webpage, an email, a chat, a PDF) and return it to the calling agent.
On-screen text is **attacker-controllable**: a web page or message can contain
`"ignore your previous instructions and …"`, and that text now arrives inside a
tool result the agent treats as trustworthy. This is a genuine
injection/exfiltration path that today's threat model (which only guards content
coming *into* cenno) does not address. The agent, not cenno, is the target, but
cenno is the conduit and must not launder attacker text as authoritative.

### T2 — Secret & sensitive-content capture (new)

The Accessibility API will not return secure text fields (macOS blanks password
fields by design), but everything else is fair game: visible API tokens, private
DMs, medical/financial info, a screenshot of an open password-manager window.
Captured content may also be *persisted* (L2/L4). We need to bound what is
capturable and scrub obvious secrets before anything is returned or stored.

### T3 — Invisible capture / no consent signal (new)

ScreenCaptureKit trips the macOS purple recording indicator, but **AX reading
has no OS-level indicator at all.** L2's passive sampler could read screen text
continuously with nothing on screen to say so. That is both a trust problem and,
again, the Clicky trap. cenno must provide its own signal and an off switch.

## Decisions

| Decision | Choice |
|---|---|
| Injection framing (T1) | Captured content is **untrusted data, never instructions.** Tool results wrap it in a typed, clearly-delimited field (e.g. `captured_content` with a `source` + `untrusted: true` marker), and the `cenno` skill teaches agents to treat it as quoted data. cenno does **not** try to sanitize natural language — it labels provenance and delegates trust to the agent. |
| Capture policy (T2) | **Denylist + redaction, applied in one shared library** that L1a/L1b/L2 all call before returning or storing content. Denylist keys on `bundle_id` and URL `host`; ships with sensible defaults (known password managers, `NSSecureTextField` contexts, banking hosts) and is user-extensible via `~/.cenno/config.json`. Redaction runs regex scrubbers for high-confidence secret shapes (private keys, `sk-`/`AKIA` tokens, JWTs) on any text before it leaves the library. |
| Consent (T3) | A **visible capture/sampling indicator** owned by cenno (menu-bar state change + an optional on-screen dot while actively reading/sampling) and a **global kill switch** in the tray that hard-stops all capture and sampling. Kill-switch state is persisted and defaults to **capture allowed but passive sampling OFF** (L2/L4 are opt-in per their own beads). |
| Privacy claim | The claim is **"cenno processes and stores captured context locally; screen capture adds no cenno network path."** We explicitly do **not** claim either "cenno makes no network calls" (CloudKit relay, the updater, and optional model downloads are separate documented paths) or "your screen never leaves your machine" (the requesting agent may send tool results to its model provider). `README.md` + the skill state that captured context is sent onward by whatever agent reads it. |
| Permissions | Accessibility and Screen Recording are **separate TCC gates**, requested **lazily** (first real use), each with graceful typed-denial handling. No entitlement changes beyond what capture requires; keep the sandbox off (already the case) — do not add restricted entitlements that AMFI would SIGKILL a Developer-ID build for (see the 0.3.0 CloudKit incident in `Entitlements.plist`). |
| Enforcement point | The denylist/redaction library is called **inside the Rust MCP boundary** (alongside `a2ui_guard`), not in the Swift capture layer — so every path (AX text, OCR text, image return, stored samples) passes through one audited chokepoint. |

## Architecture

```
Swift capture (AX / ScreenCaptureKit / Vision)
        │  raw {app, bundle_id, host, text?, image?}
        ▼
Rust MCP boundary ──►  capture_guard.rs   ◄── ~/.cenno/config.json (denylist)
        │                 │  1. kill-switch check → drop if capture disabled
        │                 │  2. denylist match (bundle_id | host) → return `blocked`
        │                 │  3. redact secret patterns in text
        │                 │  4. wrap as untrusted: {source, untrusted:true, ...}
        ▼                 ▼
   tool result  ◄──  clean payload (or typed blocked/denied/redacted marker)
        │
        └──►  capture indicator (tray + optional on-screen dot) reflects "reading now"
```

- **`capture_guard.rs`** (new): the single chokepoint. Pure, unit-testable
  functions: `is_capture_enabled()`, `is_denied(bundle_id, host)`,
  `redact(text) -> (text, redaction_count)`, `wrap_untrusted(payload)`. L1a/L1b
  and L2 call these; none of them return content without going through it.
- **Config** (`~/.cenno/config.json`, loader `config.rs`): new
  `capture` block — `enabled`, `passive_sampling`, `denylist_bundles`,
  `denylist_hosts`, `redaction` toggle. Documented in `docs/CONFIG.md`.
- **Indicator + kill switch** (`tray.rs`): a tray item showing capture state
  and a toggle; an optional lightweight on-screen indicator while a capture or
  sample is actively in flight (reuse the non-activating panel machinery).
- **Docs**: new `SECURITY.md` "Screen capture & context" section (T1–T3 + the
  precise claim); `README.md` privacy paragraph corrected; `skills/cenno/SKILL.md`
  gains a "captured content is untrusted data" rule for agents.

## Out of scope

- The capture tools themselves (L1a/L1b) and the sampler (L2) — L0 only ships
  the guard library, indicator, kill switch, and docs they depend on.
- Encryption-at-rest beyond the existing `0600` + FileVault posture.
- Network egress controls on the *agent* — cenno cannot police what the agent
  does with content; it can only label provenance and let the user say no.

## Resolved questions

1. **Redaction aggressiveness:** high-confidence patterns only: PEM private
   keys, AWS access-key ids, JWTs, and long `sk-` tokens. Redaction can be
   disabled explicitly; entropy heuristics are out of scope.
2. **Default denylist:** 1Password, Bitwarden, KeePassXC, and Keychain Access
   bundle identifiers. Hosts are user-owned configuration; cenno does not ship
   a brittle or implicitly comprehensive banking-host list.
3. **Indicator:** a checked tray item is steady while capture is allowed and
   changes to `Reading screen context…` while one or more leases are active.
   A floating dot remains a future UX option, not an L0 requirement.
