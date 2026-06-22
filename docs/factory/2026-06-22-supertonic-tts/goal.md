# Goal Contract — Supertonic on-device TTS backend (sound-out)

> Source of truth. Agents may propose **Goal Amendments**; they may not silently rewrite this.
> Builds on shipped sound-out (docs/factory/2026-06-22-sound-out/). Tracker: cenno-0wc.

## Current state (≤3)
- sound-out speaks via on-device `AVSpeechSynthesizer` (shipped); even with a premium system voice it sounds plain.
- `tts_speak(text, voice)` is the single seam where speech is produced (Rust command → Swift `CennoTts`) — the backend can be swapped behind it without touching gating/normalize/say logic.
- No audio-**file** playback path exists: AVSpeech both synthesizes *and* plays. A WAV-producing model needs a new playback step.

## Desired future state (≤3)
- A higher-quality on-device voice (**Supertonic**, 99M ONNX, `Supertone/supertonic-3`) speaks prompts, fully local — no network at speak time, **R1 preserved**.
- `AVSpeechSynthesizer` remains the automatic fallback when the model/asset is unavailable or errors.
- Backend selectable via `~/.cenno` `tts.engine` (`system` | `supertonic`); default stays `system` until Supertonic is proven.

## Current constraint (Theory of Constraints)
Voice quality — the shipped on-device voice sounds robotic, undercutting sound-out's "pleasant enough to leave on" feel. A better *local* model is the lever, without the privacy cost of cloud.

## Target user / job (JTBD)
Solo dev hearing agent prompts hands-free wants the voice good enough to actually keep voice-out enabled.

## Non-negotiable constraints (≤5)
1. **Stay R1** — fully on-device; **no prompt text egresses the network at speak time**, no API keys. (Model asset may download once on explicit enable — see release constraints — never prompt content.)
2. **AVSpeech fallback must remain** — if Supertonic fails to load/synthesize, speech still happens.
3. **Don't bundle the ~100M model in git** — cache outside the repo (app data / `~/.cenno`).
4. **Reuse the existing `tts_speak` seam** + the shipped say/normalize/gating logic unchanged.
5. **Default engine stays `system`** until Supertonic is proven; Supertonic is opt-in via config.

## Desired outcomes (solution-independent, measurable; ≤5)
1. With `tts.engine=supertonic`, a prompt is spoken in the Supertonic voice, on-device (evidence: audible + no network during speak).
2. If the model asset is absent/unloadable, it falls back to AVSpeech and still speaks (evidence: test/manual with model removed).
3. No network egress at speak time (evidence: speak works with networking off).
4. First-audio latency for a short `say` line is usable (target ≤ ~1.5 s) (evidence: measured + recorded).
5. Existing suite stays green; gating/normalize/say behavior unchanged (evidence: verify.log).

## Smallest shippable slice   <!-- required -->
Supertonic speaks one **English** line in cenno via the chosen integration path, on-device, behind `tts.engine=supertonic`, with AVSpeech fallback on any load/synth error. Single default voice, default quality/speed. Model downloaded once to app cache. **No** multi-voice UI, language switching, or quality/speed config yet.

## Stop condition   <!-- required -->
Stop and ask for human approval if any hold:
- The integration path would require **bundling/distributing the ONNX runtime or model** in a way that materially bloats the app/build.
- A **Python sidecar** would be the *shipped* artifact (not acceptable — use the native Swift SDK or Rust `ort`/ONNX, or reconsider Kokoro). A throwaway Python spike to evaluate quality is fine.
- First-audio **latency** is too high to be usable, or quality doesn't beat premium AVSpeech enough to justify the weight.
- Supertonic's Swift/Rust SDK proves immature on macOS 26.

## Success evidence (≤5)
Maps to outcomes: audible Supertonic speak (manual/note) · fallback test with model removed · network-off speak · measured first-audio latency · `npx vitest` + `cargo` + build green → `evidence/verify.log`.

## Visual checkpoints
N/A — audio feature, no UI change beyond shipped sound-out.

## Risk classification
**R1 — internal dev-assist, on-device.** EU AI Act: Art 5 prohibited use? **no** · Art 50 labelling? **no** (assistive readout of the agent's own question to its user; not synthetic-media impersonation).

## Rollback note
`tts.engine=system` (the default) disables Supertonic entirely. Additive backend module → revert the commit.

## Risks (≤5)
1. Integration path heavier than expected (ONNX runtime bundling, model distribution) → build bloat. **(Riskiest unknown — resolve in the plan, possibly a small spike.)**
2. New audio-playback path for WAV (AVSpeech doesn't play files) — adds `AVAudioPlayer`/`rodio` surface.
3. Model download UX/size (~100M+) on first enable; offline-first users.
4. Latency/quality may not beat premium AVSpeech voices enough to justify.
5. Supertonic Swift/Rust SDK maturity on macOS 26 unverified.

## Non-goals (≤5)
- Multi-voice picker, language switching, quality/speed tuning UI.
- Cloud providers (cenno-fx5), Voice Builder / custom voices.
- Bundling the model in git.

## Release constraints
- macOS 26+. Model asset cached in app data; **first enable requires a one-time download (network); speak-time stays offline.**

## Tracker
**bd** — epic `cenno-0wc` + child tasks (integration-path decision/spike, model fetch+cache, synth→WAV, audio playback, engine switch + fallback, config).

---
**Fail rule:** if a goal can't produce evidence, it's a wish with better formatting — it doesn't pass.

## Size + risk triage
**Size: L** (new backend + ONNX/SDK dependency + new audio-playback path + model download/caching + fallback). **Plan approval REQUIRED** before code (new dependency, changes how speech is produced, model-asset distribution). The plan's first job is the **integration-path decision**: native Swift SDK (matches CennoTts) vs Rust `ort`/ONNX vs (spike-only) local HTTP server — chosen on build-weight + latency, with Kokoro as the named fallback model if Supertonic disappoints.
