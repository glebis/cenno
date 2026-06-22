# Goal Contract — sound-out (voice-out for cenno prompts)

> Source of truth. Agents may propose **Goal Amendments**; they may not silently rewrite this.
> JTBD bundle: `~/jtbd/sound-out/jtbd.json`.

## Current state (≤3)
- cenno prompts are screen+keyboard bound: STT dictation exists for *answers* (`useVoiceDictation` → Swift `SpeechTranscriber`), but nothing is ever **spoken aloud** — there is no TTS/audio-playback code in the repo.
- `AskRequest` already carries a 3-level `urgency` (`Low`/`Normal`/`High`) used **only for queue ordering**, not for any audio behavior.
- Away from the screen, a prompt is missed or silently stalls the agent until the human returns to the desk.

## Desired future state (≤3)
- When a prompt appears with voice-out enabled and its urgency meets the threshold, cenno **speaks the title + body aloud**; the user answers by voice in the existing dictation field — fully hands-free for a one-shot prompt.
- Urgency `High` always sounds + shows regardless of settings; `Normal`/`Low` are gated by a user-set threshold.
- Voice-out is opt-in and on-device by default; speech is intelligible (no raw markdown/syntax read aloud).

## Current constraint (Theory of Constraints)
The human is tethered to the screen to service agent prompts — agent throughput is bottlenecked on the human being at the desk to *see and type*. sound-out attacks that single bottleneck.

## Target user / job (JTBD)
Solo dev (Gleb) running agent sessions while away from the desk / headphones on; hires sound-out to **hear and answer a blocking prompt hands-free, and triage urgency by ear** so the agent keeps moving and only interrupts when it matters.

## Non-negotiable constraints (≤5)
1. **Opt-in, default off**; trivially silenceable (a stop/skip control + the config flag).
2. **Never lose or distort the prompt's substance** in speech — normalize markdown + code identifiers to speakable text, never truncate the body.
3. **Reuse existing plumbing** — the dictation window for answers and the existing `urgency` field for priority. No parallel `priority` field, no new answer UI.
4. **No prompt text leaves the machine** unless the user has explicitly configured a cloud provider + key. Default = on-device, zero network.
5. v1 is **one-shot, macOS cenno only** — no multi-turn dialog, no iOS.

## Desired outcomes (solution-independent, measurable; ≤5)
1. A prompt with urgency `High` is spoken aloud automatically when shown (evidence: TTS invoked with the prompt's normalized text).
2. Spoken text contains **no raw markdown tokens** (`**`, backticks) and code identifiers from the db-refactor example (`refactor/i5ly.4-split-db-rs`, `db.rs`, `dump_schema.rs`) are voiced intelligibly (evidence: normalizer unit tests).
3. A prompt **below** the configured threshold is **not** spoken (evidence: gating unit test — `Normal`/`Low` at default threshold → no TTS call).
4. With voice-out disabled (`tts.enabled=false`) there is **no audio and no network** (evidence: test/assertion + manual).
5. With the on-device provider, **zero outbound network calls** are made (evidence: assertion or manual network-off run).

## Smallest shippable slice   <!-- required -->
On-device macOS TTS (`AVSpeechSynthesizer` via the existing Swift FFI) reads `title + body_md` aloud when a prompt is shown **and** its `urgency` ≥ the configured threshold (default: `High` only). Includes the markdown/identifier **normalizer** and a **stop/skip** control in the panel. Config: `tts.enabled` (bool), `tts.min_urgency` (low|normal|high). **No** cloud providers, **no** earcons, **no** emotional markup yet.

## Stop condition   <!-- required -->
Stop and ask for human approval if any of these hold:
- Reusing the existing `urgency` field proves semantically wrong (agents need a TTS-specific signal independent of queue order) → would require a parallel field (a Goal Amendment).
- On-device `AVSpeechSynthesizer` can't produce acceptable speech for the normalized text → reconsider provider order.
- The "speak on prompt-shown" hook races with panel mount/resize or double-fires and can't be made deterministic in the single loop.

## Success evidence (≤5)
- Normalizer unit tests (markdown + identifier cases incl. the db-refactor example) → `evidence/verify.log`.
- Gating unit test: TTS invoked **iff** `enabled && urgency ≥ threshold` → `evidence/verify.log`.
- Integration/manual: prompt appears → speech plays → audible; stop control halts it → note + screenshot.
- `npm` typecheck / lint / build + `cargo` test/build green → `evidence/verify.log`.
- On-device run makes no outbound network call (assertion or manual note) → `evidence/verify.log`.

## Visual checkpoints
Advisory (not blocking): a small stop/skip affordance added to the panel chrome. One screenshot at the primary panel viewport.

## Risk classification
**R1 — internal dev-assist.** EU AI Act: Art 5 prohibited use? **no** · Art 50 labelling? **no**.
(Privacy note: v1 on-device avoids any cloud transmission. Revisit this classification + a data-handling note when cloud TTS providers land as follow-on.)

## Rollback note
Config toggle `tts.enabled=false` disables the feature entirely. Otherwise additive (new `tts.rs` module + Swift TTS entry point + one frontend hook) → revert the commit.

## Risks (≤5)
1. Speaking on prompt-shown races with panel mount/resize or double-fires.
2. Normalizer over-strips and drops a critical detail — the exact failure the user fears.
3. `AVSpeechSynthesizer` voice quality/latency disappoints and pulls cloud providers forward.
4. `urgency` reuse: its queue-order semantics may not perfectly match the P0/P1/P2 mental model.
5. Scope creep into cloud providers / earcons before the on-device slice ships.

## Non-goals (≤5)
- Cloud TTS providers (Groq/OpenAI/Cartesia/ElevenLabs) — **follow-on**.
- ElevenLabs sound/music-generation earcons — **follow-on**.
- Emotional/prosody markup — **follow-on**.
- Multi-turn dialog, iOS companion, a global task-priority taxonomy.

## Release constraints
- macOS 26+ (already required for `SpeechTranscriber`). On-device TTS needs no new entitlement.

## Tracker
**bd** — decomposes into >1 task (normalizer, config schema, Swift TTS bridge + Rust command, prompt-shown hook + gating, stop control). Cloud providers / earcons / markup filed as separate follow-on issues, not v1.

---
**Fail rule:** if a goal can't produce evidence, it's a wish with better formatting — it doesn't pass.
