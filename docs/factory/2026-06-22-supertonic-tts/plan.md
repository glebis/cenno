# Plan — Supertonic backend (L, plan-approval gate)

Goal: `docs/factory/2026-06-22-supertonic-tts/goal.md`. Decided by spike.

## Integration path (spike outcome)
**Rust `ort` crate** loads the `supertonic-3` ONNX model in-process; **`rodio`** plays the PCM output. No Swift SDK, no Python sidecar. The existing Swift `CennoTts`/AVSpeech stays as the automatic fallback. Rationale: cenno's backend is Rust, Supertonic ships no SwiftPM package (only ONNX-C example code), and the Rust example already uses `ort` — least new surface, no Swift work.

## Biggest remaining unknown
Supertonic's **text preprocessing** (g2p/tokenization → model inputs). The ONNX model doesn't take raw text; the `rust/` example handles text→tokens. That porting is the real work and the main risk.

## Build sequence
0. **De-risk first (standalone):** clone + `cargo build --release` the upstream `rust/` example, download `supertonic-3` assets, synthesize one WAV from the CLI, and *listen*. Confirms `ort`+model+preprocessing work on this macOS arm64 before touching cenno. If this balloons (ORT build, preprocessing) → STOP per goal stop-condition (reconsider Kokoro / Swift).
1. Add deps: `ort` (decide download-binaries vs brew system ORT), `rodio`. Pin ORT version.
2. `supertonic.rs`: lazy-load the ONNX session once; `synthesize(text, lang="en", default voice, steps, speed) -> Vec<f32>` (44.1kHz). Port preprocessing from the example.
3. Model fetch + cache: ensure `~/.cenno/models/supertonic-3/` (or app data); one-time download on enable; never at speak time.
4. Playback via `rodio` sink; wire `tts_stop` to stop the sink (so the mute button works for Supertonic too).
5. Dispatch in `tts_speak`: if `tts.engine==supertonic` and session loads → Supertonic; on any error → AVSpeech fallback. Add `tts.engine` to config (`system` default).

## Verify
Standalone WAV audible (step 0) · cenno speaks via Supertonic (manual) · model-removed → AVSpeech fallback (test/manual) · network-off speak works · first-audio latency measured · `cargo`+`vitest`+build green → `evidence/verify.log`.

## Out of scope (this slice)
Multi-voice, language switching, quality/speed UI, CoreML/ANE tuning, bundling the model.
