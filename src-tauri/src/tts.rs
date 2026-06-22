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
        let c = CString::new(text).map_err(|_| "speech text contains an interior NUL".to_string())?;
        // Keep the optional voice CString alive for the duration of the call.
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
#[tauri::command]
pub async fn tts_stop() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    unsafe {
        cenno_tts_stop()
    };
    Ok(())
}
