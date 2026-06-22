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

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
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

/// List the names of available audio output devices, for the settings picker.
/// Best-effort: returns an empty list if the host can't be queried. Names are
/// what `speak_blocking`'s `device` argument matches against.
pub fn output_device_names() -> Vec<String> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let host = rodio::cpal::default_host();
    match host.output_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Open an output stream on the named device, falling back to the default when
/// the name is absent or no longer matches a present device (e.g. unplugged).
fn open_output(device: Option<&str>) -> Option<(OutputStream, rodio::OutputStreamHandle)> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    if let Some(name) = device.filter(|n| !n.is_empty()) {
        let host = rodio::cpal::default_host();
        if let Ok(mut devices) = host.output_devices() {
            if let Some(dev) = devices.find(|d| d.name().map(|n| n == name).unwrap_or(false)) {
                if let Ok(pair) = OutputStream::try_from_device(&dev) {
                    return Some(pair);
                }
            }
        }
    }
    OutputStream::try_default().ok()
}

/// Synthesize and play `text` on a blocking thread, returning once playback has
/// *started* (not finished) so the async command doesn't block. Any superseding
/// call or `stop()` interrupts the previous utterance. Errors propagate so the
/// caller can fall back to AVSpeech. `device` names the output to play through
/// (None → system default).
pub fn speak_blocking(text: &str, voice: &str, device: Option<String>) -> Result<()> {
    let (samples, sample_rate) = synthesize(text, voice)?;
    // Stop a prior utterance before starting this one.
    if let Some(prev) = SINK.lock().take() {
        prev.stop();
    }
    // Own the output stream on a dedicated thread; it must outlive playback.
    std::thread::spawn(move || {
        let Some((_stream, handle)) = open_output(device.as_deref()) else {
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

/// Delete the managed model cache (`~/.cenno/models/supertonic-3`) so it can be
/// re-downloaded. Only ever touches the default cache — never a user's custom
/// `tts.model_path`. Drops the in-memory engine too, so the next synthesis (or
/// re-download) starts from a clean slate. No-op if the cache is already gone.
pub fn delete_model() -> Result<()> {
    *ENGINE.lock() = None;
    let dir = default_model_dir().ok_or_else(|| anyhow!("no home dir for ~/.cenno"))?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    Ok(())
}

/// Stop any in-progress Supertonic playback. Idempotent.
pub fn stop() {
    if let Some(sink) = SINK.lock().as_ref() {
        sink.stop();
    }
}

// ─────────────────────────── model download ───────────────────────────

/// Pinned source. `resolve/main` is mutable upstream; we guard integrity by
/// verifying each file's exact byte size against the manifest below, so a
/// changed/truncated download is rejected rather than silently used.
const HF_BASE: &str = "https://huggingface.co/Supertone/supertonic-3/resolve/main/";

/// Every file the engine needs, with its exact size (bytes). Loading needs all
/// of `onnx/` plus the voice styles; `assets_present` and the download both
/// validate against this list (audit: the old 2-file check was insufficient).
const REQUIRED: &[(&str, u64)] = &[
    ("onnx/duration_predictor.onnx", 3_700_147),
    ("onnx/text_encoder.onnx", 36_416_150),
    ("onnx/tts.json", 8_253),
    ("onnx/unicode_indexer.json", 277_676),
    ("onnx/vector_estimator.onnx", 256_534_781),
    ("onnx/vocoder.onnx", 101_424_195),
    ("voice_styles/F1.json", 292_046),
    ("voice_styles/F2.json", 292_423),
    ("voice_styles/F3.json", 290_794),
    ("voice_styles/F4.json", 291_808),
    ("voice_styles/F5.json", 291_479),
    ("voice_styles/M1.json", 291_748),
    ("voice_styles/M2.json", 292_055),
    ("voice_styles/M3.json", 290_198),
    ("voice_styles/M4.json", 291_522),
    ("voice_styles/M5.json", 291_469),
];

fn required_total() -> u64 {
    REQUIRED.iter().map(|(_, s)| s).sum()
}

/// Status of the resolved model dir, for the settings UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelStatus {
    /// True when every required file is present (existence check — a custom
    /// model dir need not byte-match the managed manifest).
    pub present: bool,
    /// The resolved dir (custom path or default cache).
    pub dir: String,
    /// Whether `dir` is a user-set custom path vs the managed cache.
    pub custom: bool,
    /// Required files that are missing from `dir`.
    pub missing: Vec<String>,
    /// Total download size for the managed model (bytes).
    pub total_bytes: u64,
}

pub fn model_status() -> ModelStatus {
    let cfg = crate::config::Config::load();
    let custom = cfg.tts.model_path.as_deref().is_some_and(|p| !p.trim().is_empty());
    let dir = model_dir();
    let Some(dir) = dir else {
        return ModelStatus {
            present: false,
            dir: String::new(),
            custom,
            missing: REQUIRED.iter().map(|(p, _)| (*p).to_string()).collect(),
            total_bytes: required_total(),
        };
    };
    let missing: Vec<String> = REQUIRED
        .iter()
        .filter(|(rel, _)| !dir.join(rel).exists())
        .map(|(rel, _)| (*rel).to_string())
        .collect();
    ModelStatus {
        present: missing.is_empty(),
        dir: dir.to_string_lossy().into_owned(),
        custom,
        missing,
        total_bytes: required_total(),
    }
}

/// Download the managed model into the default cache, verifying each file's
/// size, then publish atomically (temp dir → swap). Always targets the cache,
/// not a custom path. `progress(done, total)` is called as bytes arrive. Runs
/// on a blocking thread (caller uses spawn_blocking).
pub fn download_blocking(mut progress: impl FnMut(u64, u64)) -> Result<()> {
    let dest = default_model_dir().context("no ~/.cenno for model cache")?;
    let parent = dest.parent().context("model cache has no parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(".supertonic-3.download");
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }

    let total = required_total();
    let mut done = 0u64;
    let agent = ureq::AgentBuilder::new().build();

    for (rel, expected) in REQUIRED {
        let url = format!("{HF_BASE}{rel}");
        let out = tmp.join(rel);
        if let Some(d) = out.parent() {
            std::fs::create_dir_all(d)?;
        }
        let resp = agent.get(&url).call().map_err(|e| anyhow!("GET {rel}: {e}"))?;
        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(&out)?;
        let mut buf = [0u8; 65_536];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            done += n as u64;
            progress(done, total);
        }
        file.flush()?;
        let got = std::fs::metadata(&out)?.len();
        if got != *expected {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(anyhow!("{rel}: downloaded {got} bytes, expected {expected} (integrity check failed)"));
        }
    }

    // Publish: swap temp → dest with a backup so a failed rename rolls back.
    if dest.exists() {
        let bak = parent.join(".supertonic-3.bak");
        if bak.exists() {
            std::fs::remove_dir_all(&bak)?;
        }
        std::fs::rename(&dest, &bak)?;
        match std::fs::rename(&tmp, &dest) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&bak);
            }
            Err(e) => {
                let _ = std::fs::rename(&bak, &dest);
                return Err(anyhow!("publishing model: {e}"));
            }
        }
    } else {
        std::fs::rename(&tmp, &dest)?;
    }

    // Fresh model on disk → drop the cached engine so the next speak reloads it.
    *ENGINE.lock() = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_total_is_about_398mb() {
        // Guards against a typo'd size silently weakening integrity checks.
        let total = required_total();
        assert!(total > 395_000_000 && total < 410_000_000, "unexpected total {total}");
        assert_eq!(REQUIRED.len(), 16);
    }

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
