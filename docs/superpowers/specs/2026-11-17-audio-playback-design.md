# Audio playback in the prompt panel

Status: design spec. Spec date: 2026-11-17. Path: architectural (new flow, no existing path).

## Intent

Let an agent attach a generated audio artifact — an ElevenLabs voice clip, a jingle, a music snippet, a voice memo, an ambient track — to an `ask_user` prompt. The user hears it when the panel opens (autoplay) and can pause/seek. Audio is the payload, not a side channel.

This is **not** a general TTS replacement; `say:` stays untouched. Audio playback handles *agent-supplied* files/data.

## User-visible behavior

1. Agent calls `ask_user` with a new `audio:` field — file path or data URI.
2. Panel opens with the audio UI **above the body text**, below the title.
3. Audio **autoplays** on open. While playing, the prompt's `timeout_s` countdown **pauses**; it resumes when audio ends or the user pauses.
4. Player controls: **Play ▶ / Pause ⏸**, a **scrubber bar** (draggable), and a **time display** (`0:23 / 1:47`).
5. `muted: true` on the request silences **both** TTS voice-out and audio playback. The panel's mute toggle continues to work for both.
6. Voice-in (`voice_text`, `voice`) stays enabled — audio and voice coexist (user can pause audio, then speak).
7. On oversized payloads (>5 MB), the panel renders text-only with an "Audio too large" note and an **Open in Finder** button (if the source was a file path and the file still exists). The prompt completes as an `Answered` — the audio-drop is visible to the user but not an error to the agent.

## Wire protocol

New optional field on `AskRequest`:

```rust
pub struct AskRequest {
    // ... existing fields ...
    /// Audio payload for the panel to play. Either:
    ///  - a path to a local file (mp3/wav/m4a/ogg — extension + magic bytes checked), or
    ///  - a data URI: `data:audio/mpeg;base64,iVBOR...`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AudioSource {
    /// Local file path. Bounded: only under allowed roots, extension + magic-byte validated, ≤ 5 MB.
    File(String),
    /// Base64 data URI. Magic-byte validated after decode, ≤ 5 MB decoded.
    Uri(String),
}
```

The discriminator is untagged: starts-with-`data:audio/` → data URI; otherwise treated as path. Paths beginning with `data:` that are **not** valid data URIs fail validation.

**Schemars / agent contract:** field shows up in the `ask_user` tool schema; `skip_serializing_if = None` keeps the wire clean when unused. Like `agent`, this field is *agent-writable* — unlike `agent` it is not server-only.

## Backend: new module `src-tauri/src/audio.rs`

Mirrors `image.rs` (extension + magic-byte checked, bounded size).

```rust
pub const MAX_AUDIO_BYTES: usize = 5 * 1024 * 1024; // 5 MB — about 20 min of speech at 128 kbps

pub enum AudioError {
    NotFound,
    NotAFile,
    PathDenied,
    Oversize { bytes: usize, max: usize },
    BadExtension,
    MagicMismatch,
    BadDataUri,
    DecodeError,
}

pub struct LoadedAudio {
    /// Always a data: URI with a real MIME type — the frontend never resolves a path itself.
    pub data_uri: String,
    pub mime: &'static str,   // "audio/mpeg" | "audio/wav" | "audio/mp4" | "audio/ogg"
    pub bytes: usize,         // decoded size
    /// Some(path) if the source was a file — used for "Open in Finder" when oversized.
    pub source_path: Option<PathBuf>,
}

pub fn load_audio(src: &AudioSource) -> Result<LoadedAudio, AudioError>;
```

Validation rules (same shape as `image.rs`):

- **File path:** must be an absolute path to a regular file. Resolve `..` against a configurable allowlist of roots — by default `~/.cenno/audio/`, the system temp dir, and any directory listed under `~/.cenno/audio_roots` (one per line, user-managed). Reject anything outside the allowlist with `AudioError::PathDenied`, which `mcp.rs` maps to the MCP `InvalidParams` surface (see the failure-mode table below).
- **Extension allowlist:** `.mp3`, `.wav`, `.m4a`, `.ogg` (case-insensitive).
- **Size:** `metadata.len()` ≤ 5 MB. Over → `Oversize`.
- **Magic bytes:**
  - mp3: `ID3` *or* `0xFF 0xFB` / `0xFF 0xFA` / `0xFF 0xF3` / `0xFF 0xF2` frame sync
  - wav: `RIFF` at 0..4, `WAVE` at 8..12
  - m4a: `....ftyp` at bytes 4..12, brand one of `M4A ` / `mp42` / `isom`
  - ogg: `OggS` at 0..4
- **Data URI:** must match `^data:audio/(mpeg|wav|mp4|ogg);base64,`. Decode with the same base64 engine `image.rs` uses, then run the same magic-byte checks against the decoded bytes. Decoded size ≤ 5 MB.

On `Oversize` when `src` was `File`, `mcp.rs` swallows the error and instead surfaces a panel note: the prompt is emitted **without** audio, with an additional flag (see protocol change below) so the frontend can render the "Audio too large → Open in Finder" affordance. The path is **not** re-read at click time; the panel just calls the existing `open_path` (or platform equivalent) helper.

## Protocol additions

Two small additions to `protocol.rs`:

```rust
pub struct AskRequest {
    // ...
    pub audio: Option<AudioSource>,
}

/// Per-prompt event payload sent from mcp.rs to the webview — existing `prompt` event grows one
/// optional field so the frontend knows why audio is missing when it would have been oversized.
#[derive(Clone, Serialize)]
pub struct PromptPayload /* existing shape */ {
    // ...
    /// Present only when the agent supplied audio but it was rejected as oversized.
    /// Frontend shows "Audio too large" + Open in Finder button for `path` payloads.
    pub audio_oversize: Option<AudioOversizeNote>,
}

#[derive(Clone, Serialize)]
pub struct AudioOversizeNote {
    pub bytes: usize,
    pub max: usize,
    /// Path when the source was a file; None for data URIs (nothing to reveal in Finder).
    pub source_path: Option<PathBuf>,
}
```

`SeqMeta`/`AskResponse`/`ask_sequence` are untouched. Each question in a sequence may carry its own `audio:` — they play one at a time as that question's panel renders.

## Frontend: `src/PromptPanel.tsx`

New component `src/audio/AudioPlayer.tsx`, mounted **above the body**, below the title:

```
┌───────────────────────────────────────┐
│           <title>                     │   (unchanged)
├───────────────────────────────────────┤
│ ▶/⏸  ═══════●═══════  0:23 / 1:47     │   ← AudioPlayer
├───────────────────────────────────────┤
│           <body_md>                   │   (unchanged)
│           <input widget>              │   (unchanged)
└───────────────────────────────────────┘
```

Behavior:

- **Autoplay** on mount, unless `muted: true` was set on the request or the panel's own mute toggle is on.
- **Play ⏸ toggle** single button — icon flips between ▶ and ⏸ as appropriate.
- **Scrubber:** a thin track; thumb follows playback; user can drag to seek. Implemented with `<input type="range">` + custom CSS so it stays tiny.
- **Time readout:** `currentTime / duration`, formats as `m:ss`; while duration is unknown show `-:--`.
- **While playing:** panel suppresses the timeout countdown (see next section).
- **Replay:** when `ended` fires, the play button turns into a ↻ replay (same slot, same size — not a new control).
- **Oversize fallback:** when `audio_oversize` is set and `source_path` is present, render a small notice row in place of the player: "Audio too large (7.2 MB > 5 MB) — [Open in Finder]". If `source_path` is `None` (data URI oversized), render "Audio too large (7.2 MB > 5 MB)" without button.
- **Voice-in unchanged.** Mic buttons stay.

Implementation notes:

- Use WebAudio (`AudioContext` + `decodeAudioData`) — *not* `<audio>` — because we need precise currentTime for the scrubber and consistent autoplay behavior given WKWebView's autoplay policies.
- AudioContext is created on first user gesture if autoplay is blocked by the OS; a tiny "▶ to enable audio" prompt sits where the player was (rare path — WKWebView usually allows autoplay for user-initiated panels).
- The data URI arrives as a string on the `prompt` event; we `fetch(data:…)` → `ArrayBuffer` → `decodeAudioData`. Memory cap mirrors MAX_AUDIO_BYTES — we reject and render nothing (with the oversize note) rather than crash the webview.

## Timeout interop

`registry.rs` currently owns the per-prompt countdown. Changes:

- Frontend emits a new event `audio-playback-state` on the panel window's event channel with payload `{ prompt_id, playing: bool }`.
- `registry.rs` listens; while `playing: true` for the active prompt, it **pauses** the timeout timer. When `playing: false` (paused, ended, errored, panel closed), the timer resumes with the remaining time.
- If the panel is dismissed, timer cleanup happens anyway (existing path). No leaked subscriptions.
- `ask_sequence` steps share the same mechanism — the most recent event wins; earlier prompt ids are ignored.

The pause is *playback-driven*, not "panel-open-driven" — a paused audio resumes the countdown.

## Mute interop

`muted: true` (request) OR the user-toggle mute both suppress:

- TTS voice-out (existing path — unchanged)
- Audio autoplay (new)

While muted, the player sits idle with ▶ visible. The user can still press play — mute silences *auto* playback, not user-initiated playback. This matches how muted notifications still show a badge.

## Voice interop

No coupling. `AudioPlayer` doesn't interact with the voice pipeline. If the user starts recording while audio plays, audio keeps playing (user can pause it). This keeps `voice.rs` untouched.

## DB & history

`db.rs` rows gain one nullable column:

```sql
audio_path TEXT NULL  -- file path when source was File; NULL for data URIs and absent audio
```

No base64 in the DB. `cenno export` includes the column in its JSON output. History rendering (if/when a UI for it exists) can show a "had audio" affordance but is out of scope for this spec.

## Security / trust boundary

Prompts are untrusted input. The new attack surface:

1. **Path traversal.** Mitigated by the root allowlist + absolute-path + path-canonicalization before compare.
2. **Format confusion / polyglot.** Mitigated by magic-byte checks after extension check.
3. **Zip bomb / oversized data URI.** Mitigated by 5 MB decoded cap, enforced *before* decode to the webview (Rust-side on data URIs, file-size check on paths).
4. **Autoplay loudness.** Mitigated by `muted:` and the user toggle.
5. **Finder exposure via "Open in Finder".** Path comes from `LoadedAudio::source_path`, which is the same path we already validated and (for oversize) only stat()'d. We do not canonicalize-and-reveal new paths; we reveal the path the agent supplied, validated and still on an allowed root.

No new network calls. Data URIs decode locally; file paths read locally. AGENTS.md's "no network calls" constraint stays intact.

## Failure modes and agent-visible errors

| Condition                          | Behavior                                                              |
|------------------------------------|-----------------------------------------------------------------------|
| Path missing                       | `InvalidParams` on `ask_user` — no panel opens                        |
| Path outside allowlist (`PathDenied` internal → mapped to `InvalidParams`) | `InvalidParams` at MCP boundary — no panel opens |
| Bad extension                      | `InvalidParams` — no panel opens                                      |
| Magic-byte mismatch                | `InvalidParams` — no panel opens                                      |
| Bad data URI (syntax/MIME)         | `InvalidParams` — no panel opens                                      |
| Oversized (file)                   | Panel opens **without** the player; `audio_oversize` note with path; answer flow unaffected |
| Oversized (data URI)               | Same, but note without path (nothing to reveal)                       |
| WebAudio decode error at runtime   | Player shows "Couldn't decode audio"; prompt remains answerable       |
| Timeout pause stream interrupted   | Countdown resumes with remaining time (fail-safe)                     |

## Testing

**Rust (`cargo test` in `src-tauri/`):**

- `audio.rs`: allowlist of magic bytes + extensions × {legit, wrong ext, wrong magic, empty file, oversize, data URI variants} — table-driven.
- Path allowlist: traversal attempts (`..`, symlinks, hardlinks) rejected.
- Wire: `AskRequest` schema exposes `audio`, `audio` survives serialize/deserialize roundtrips, schemars hides nothing new.
- Protocol: oversized file → `PromptPayload.audio_oversize` is populated; normal file → field is `None`.
- Timeout: simulated playing/paused events pause and resume the countdown.

**Frontend (`npx vitest run`):**

- `AudioPlayer` renders and toggles play/pause via mocked `AudioContext`.
- Scrubber drag calls `seek` and updates the time readout.
- Autoplay suppressed when `muted: true`.
- Replay button appears after `ended`.
- Oversize + path → "Open in Finder" affordance; oversize + data-URI → note without button.
- Emits `audio-playback-state` with `playing: true/false` at the right transitions (registry test relies on this).

**Smoke (`scripts/demo.sh`)** grows one scenario: a 3-second generated sine wav under 1 MB from the system temp dir, plus an oversize mp3, plus a data-URI wav. Each exercises one of the three primary paths.

## Out of scope (explicitly)

- No standalone `play_audio` MCP tool. Playback is always attached to a question.
- No streaming playback from HTTPS URLs.
- No volume / playback-speed controls.
- No "audio gallery" of past clips in history UI (DB column is enough).
- No recording/microphone integration. Voice stays in `voice.rs`.
- No change to `say:` TTS. Agents that want pure speech without panel controls keep using `say:`.

## Migration / backwards compat

- `audio` is optional. Old agents behave identically.
- `muted` semantic widens from "silence voice-out" to "silence voice-out *and* audio autoplay". Existing agents using `muted` for TTS reasons continue to behave reasonably — a muted panel stays silent.
- New DB column is additive; schema migration follows the existing pattern used for previous columns.
- Tool schema gains one optional field; MCP clients that cached the old schema will keep working.

## File touch list

- `src-tauri/src/protocol.rs` — `AudioSource`, `AskRequest.audio`, `PromptPayload.audio_oversize`, `AudioOversizeNote`.
- `src-tauri/src/audio.rs` — **new**.
- `src-tauri/src/mcp.rs` — load audio; populate `audio_oversize` on oversize.
- `src-tauri/src/registry.rs` — pause/resume the countdown on playback state; keep a suspended prompt answerable in `pending()`.
- `src-tauri/src/db.rs` — new `audio_path` column + insert/include in queries.
- `src/PromptPanel.tsx` — render `AudioPlayer` above body; handle oversize note; wire mute.
- `src/audio/AudioPlayer.tsx` — **new** component + helpers.
- `src/audio/useAudioPlayer.ts` — **new**, the WebAudio engine (kept out of the component so it can be tested directly).
- `src/audio/AudioPlayer.test.tsx` — **new**.
- `src/App.tsx`, `src/App.css` — plumb `audio_uri`/`audio_oversize`, suspend the frontend budget clock, player styles.
- `scripts/demo.sh` — `audio` and `audio-oversize` scenarios.
- `README.md`, `skills/cenno/SKILL.md` — one short section describing the new field.

### As built, two deviations from the text above

- **No `bridge.rs` touch.** `src-tauri/src/bridge.rs` is only the stdio↔socket pipe. Playback state reaches Rust over a Tauri command (`mark_audio_playing`), registered in `lib.rs` — the same channel every other panel→backend signal uses.
- **`AudioSource` is a transparent newtype over `String`, not a two-variant untagged enum.** Both variants would be a bare `String`, so serde put *every* value into the first variant (`"/tmp/x.mp3"` parsed as `Uri`, and `{"File": …}` failed to parse at all). The URI-vs-path distinction lives in the value (`data:` prefix, case-insensitive), matching the spec's original rule.

## Open questions

None at spec time. All decisions resolved during brainstorming:

- Path + data URI, no URL.
- All four formats (mp3/wav/m4a/ogg).
- 5 MB cap.
- Autoplay + pause/seek controls.
- Mute covers audio.
- Voice + audio coexist.
- Path-only persistence.
