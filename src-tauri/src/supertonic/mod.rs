//! supertonic — on-device neural TTS backend for sound-out.
//!
//! Higher-quality alternative to AVSpeechSynthesizer, fully local: the
//! Supertonic ONNX models (`Supertone/supertonic-3`, MIT code / OpenRAIL-M
//! weights) run in-process via `ort`. The inference pipeline in `helper` is
//! vendored from supertone-inc/supertonic (MIT) — text preprocessing is pure
//! Rust (unicode-indexer tokenization, no g2p/network).
//!
//! Latency scales with text length × denoising steps, so sound-out speaks the
//! short `say` summary at a low step count (~5) to stay well under a second.
//! Assets are cached under `~/.cenno/models/supertonic-3/`; if they're missing
//! or anything fails, the caller falls back to AVSpeech (see tts.rs).

// Vendored verbatim from supertone-inc/supertonic (MIT); not all of its
// helpers are used here (e.g. batch synthesis, filename sanitizing).
#[allow(dead_code)]
mod helper;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};

use helper::{load_text_to_speech, load_voice_style, TextToSpeech};

/// Denoising steps for sound-out: low, because we speak short summaries and
/// want sub-second latency (quality 5 is fine for a one-line prompt).
const STEPS: usize = 5;
const SPEED: f32 = 1.05;
const PAUSE_S: f32 = 0.3;

/// Cached engine (4 ONNX sessions) keyed by the resolved model dir, so a
/// changed `model_path` or a freshly-downloaded model reloads sessions instead
/// of serving the stale one (audit: static cache wouldn't pick up changes).
static ENGINE: Mutex<Option<(PathBuf, TextToSpeech)>> = Mutex::new(None);
/// The currently-playing sink, so `stop()` can interrupt it (e.g. the mute
/// button or a superseding prompt). `Arc` because the playback thread also
/// holds it to `sleep_until_end`.
static SINK: Mutex<Option<Arc<Sink>>> = Mutex::new(None);

/// The default model cache, `~/.cenno/models/supertonic-3`.
fn default_model_dir() -> Option<PathBuf> {
    crate::config::cenno_dir().map(|d| d.join("models/supertonic-3"))
}

/// True if `dir` holds a usable model (a representative ONNX file + the indexer).
fn dir_has_model(dir: &std::path::Path) -> bool {
    dir.join("onnx/vocoder.onnx").exists() && dir.join("onnx/unicode_indexer.json").exists()
}

/// Pure resolution policy: a valid custom path wins; otherwise the default
/// cache. `custom_valid` lets callers (and tests) inject the validity check.
fn pick_model_dir(custom: Option<&str>, custom_valid: bool, default: PathBuf) -> PathBuf {
    match custom {
        Some(p) if !p.trim().is_empty() && custom_valid => PathBuf::from(p),
        _ => default,
    }
}

/// Resolve the model dir from config: a valid `tts.model_path` else the default
/// cache. An invalid custom path falls through to the default (and ultimately
/// to AVSpeech via `assets_present`), never crashes.
fn model_dir() -> Option<PathBuf> {
    let default = default_model_dir()?;
    let cfg = crate::config::Config::load();
    let custom = cfg.tts.model_path.clone();
    let custom_valid = custom
        .as_deref()
        .map(|p| dir_has_model(std::path::Path::new(p)))
        .unwrap_or(false);
    Some(pick_model_dir(custom.as_deref(), custom_valid, default))
}

/// True if the cached assets are present (so the caller can decide engine
/// without paying a load attempt).
pub fn assets_present() -> bool {
    model_dir().is_some_and(|d| dir_has_model(&d))
}

/// Synthesize `text` to mono f32 PCM at the engine's sample rate. Loads (and
/// caches) the engine on first call. `voice` is a style name like "F3".
fn synthesize(text: &str, voice: &str) -> Result<(Vec<f32>, i32)> {
    let dir = model_dir().context("no home dir for ~/.cenno")?;
    let onnx_dir = dir.join("onnx");
    let style_path = dir.join("voice_styles").join(format!("{voice}.json"));

    let mut guard = ENGINE.lock();
    let needs_reload = guard.as_ref().is_none_or(|(cached, _)| cached != &dir);
    if needs_reload {
        let tts = load_text_to_speech(&onnx_dir.to_string_lossy(), false)
            .context("loading Supertonic ONNX models")?;
        *guard = Some((dir.clone(), tts));
    }
    let (_, tts) = guard.as_mut().expect("engine just loaded");

    let style = load_voice_style(&[style_path.to_string_lossy().into_owned()], false)
        .with_context(|| format!("loading voice style {voice}"))?;
    let (wav, duration) = tts
        .call(text, "en", &style, STEPS, SPEED, PAUSE_S)
        .context("Supertonic synthesis")?;

    // Trim to the reported duration (the model pads).
    let len = ((tts.sample_rate as f32 * duration) as usize).min(wav.len());
    Ok((wav[..len].to_vec(), tts.sample_rate))
}

/// Synthesize and play `text` on a blocking thread, returning once playback has
/// *started* (not finished) so the async command doesn't block. Any superseding
/// call or `stop()` interrupts the previous utterance. Errors propagate so the
/// caller can fall back to AVSpeech.
pub fn speak_blocking(text: &str, voice: &str) -> Result<()> {
    let (samples, sample_rate) = synthesize(text, voice)?;
    // Stop a prior utterance before starting this one.
    if let Some(prev) = SINK.lock().take() {
        prev.stop();
    }
    // Own the output stream on a dedicated thread; it must outlive playback.
    std::thread::spawn(move || {
        let Ok((_stream, handle)) = OutputStream::try_default() else {
            return;
        };
        let Ok(sink) = Sink::try_new(&handle) else {
            return;
        };
        let sink = Arc::new(sink);
        *SINK.lock() = Some(sink.clone());
        sink.append(SamplesBuffer::new(1, sample_rate as u32, samples));
        sink.sleep_until_end(); // keeps _stream alive until done or stopped
        // Clear our slot if it's still us (a newer utterance may have replaced it).
        let mut slot = SINK.lock();
        if slot.as_ref().is_some_and(|s| Arc::ptr_eq(s, &sink)) {
            *slot = None;
        }
    });
    Ok(())
}

/// Stop any in-progress Supertonic playback. Idempotent.
pub fn stop() {
    if let Some(sink) = SINK.lock().as_ref() {
        sink.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_path_used_only_when_valid() {
        let default = PathBuf::from("/default/cache");
        // valid custom → used
        assert_eq!(
            pick_model_dir(Some("/custom/model"), true, default.clone()),
            PathBuf::from("/custom/model")
        );
        // invalid custom → default (never a broken path → AVSpeech fallback)
        assert_eq!(pick_model_dir(Some("/custom/model"), false, default.clone()), default);
        // empty / absent → default
        assert_eq!(pick_model_dir(Some("  "), true, default.clone()), default);
        assert_eq!(pick_model_dir(None, false, default.clone()), default);
    }
}
