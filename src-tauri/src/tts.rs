//! tts.rs — on-device text-to-speech for spoken prompts ("sound-out").
//!
//! The mirror of voice.rs: where voice.rs transcribes the user, this speaks a
//! prompt aloud. The engine is a Swift module (`src-tauri/swift`, linked by
//! swift-rs) wrapping Apple's **AVSpeechSynthesizer** — on-device, no network,
//! no API key. The webview owns the policy (urgency gating + markdown/identifier
//! normalization); Rust just forwards already-speakable text across the FFI.

#[cfg(target_os = "macos")]
use std::ffi::{c_char, CString};

#[cfg(target_os = "macos")]
extern "C" {
    fn cenno_tts_speak(text: *const c_char, voice: *const c_char);
    fn cenno_tts_stop();
}

/// Speak `text` aloud. Called by the webview when a prompt is shown and passes
/// the urgency gate; `text` is already normalized (and may be the agent's `say`
/// summary rather than the body). `voice` is an optional AVSpeechSynthesisVoice
/// identifier from config; null/empty → Swift auto-picks the best installed
/// voice. A new call supersedes any utterance still in progress.
#[tauri::command]
pub async fn tts_speak(text: String, voice: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Engine choice lives in ~/.cenno (default "system"). When "supertonic"
        // is selected, try the on-device neural backend; on any failure fall
        // through to AVSpeech so the user still hears the prompt.
        let engine = crate::config::Config::load().tts.engine.unwrap_or_default();
        if engine == "supertonic" && crate::supertonic::assets_present() {
            let style = voice.clone().filter(|v| !v.is_empty()).unwrap_or_else(|| "F3".to_string());
            let t = text.clone();
            match tokio::task::spawn_blocking(move || crate::supertonic::speak_blocking(&t, &style)).await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(e)) => eprintln!("cenno: Supertonic failed ({e}); falling back to AVSpeech"),
                Err(e) => eprintln!("cenno: Supertonic task panicked ({e}); falling back to AVSpeech"),
            }
        }
        // AVSpeech (default engine, or Supertonic fallback).
        let c = CString::new(text).map_err(|_| "speech text contains an interior NUL".to_string())?;
        let voice_c = match voice {
            Some(v) if !v.is_empty() => Some(CString::new(v).map_err(|_| "voice id contains an interior NUL".to_string())?),
            _ => None,
        };
        let voice_ptr = voice_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        unsafe { cenno_tts_speak(c.as_ptr(), voice_ptr) };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, voice); // voice-out is macOS-only for now
    }
    Ok(())
}

/// Stop any in-progress speech (the user answered, dismissed, or hit mute).
/// Idempotent when nothing is speaking.
/// Status of the resolved Supertonic model dir (for the settings window).
#[tauri::command]
pub fn tts_model_status() -> crate::supertonic::ModelStatus {
    crate::supertonic::model_status()
}

/// Download the managed Supertonic model into the cache, emitting
/// `tts-download-progress` events ({pct, done, total, status[, message]}).
/// User-initiated from settings; the only network this feature touches.
#[tauri::command]
pub async fn tts_download_model(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    let app_evt = app.clone();
    let res = tokio::task::spawn_blocking(move || {
        let mut last_pct = u64::MAX;
        crate::supertonic::download_blocking(|done, total| {
            let pct = if total > 0 { done * 100 / total } else { 0 };
            if pct != last_pct {
                last_pct = pct;
                let _ = app_evt.emit(
                    "tts-download-progress",
                    serde_json::json!({"status": "downloading", "pct": pct, "done": done, "total": total}),
                );
            }
        })
    })
    .await;
    match res {
        Ok(Ok(())) => {
            let _ = app.emit("tts-download-progress", serde_json::json!({"status": "done", "pct": 100}));
            Ok(())
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            let _ = app.emit("tts-download-progress", serde_json::json!({"status": "error", "message": msg}));
            Err(e.to_string())
        }
        Err(e) => Err(format!("download task panicked: {e}")),
    }
}

#[tauri::command]
pub async fn tts_stop() -> Result<(), String> {
    // Stop whichever backend might be speaking.
    crate::supertonic::stop();
    #[cfg(target_os = "macos")]
    unsafe {
        cenno_tts_stop()
    };
    Ok(())
}
