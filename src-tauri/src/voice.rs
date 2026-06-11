//! voice.rs — push-to-talk dictation for `voice_text` prompts.
//!
//! The transcription engine is a Swift module (`src-tauri/swift`, linked by
//! swift-rs) wrapping Apple's **SpeechAnalyzer / SpeechTranscriber** (macOS
//! 26+): long-form, on-device, no network, no API key. It natively separates
//! VOLATILE (live progress) from FINALIZED (locked) results, so Rust does no
//! transcript stitching — it forwards whatever Swift reports.
//!
//! Flow: the webview calls `voice_start` / `voice_stop`. Swift streams results
//! back through a C callback, which this module re-emits as `voice-event`s:
//!   {type:"state", state:"recording"|"stopped"}
//!   {type:"partial", text}   — full current transcript (finalized + volatile)
//!   {type:"error", message}  — graceful error (permissions, locale, <macOS 26)

use std::ffi::{c_char, c_void, CStr};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

pub const EVENT: &str = "voice-event";

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceEvent {
    State { state: &'static str },
    Partial { text: String },
    Error { message: String },
}

/// The app handle the Swift callback emits through. Set on `voice_start`; the
/// callback reads it, so no raw pointer crosses the FFI boundary for context.
static APP: Mutex<Option<AppHandle>> = Mutex::new(None);

fn emit(app: &AppHandle, ev: VoiceEvent) {
    let _ = app.emit(EVENT, ev);
}

// Kind tags, kept in sync with CennoVoice.swift.
const KIND_TRANSCRIPT: i32 = 0;
const KIND_RECORDING: i32 = 1;
const KIND_STOPPED: i32 = 2;

/// Invoked by the Swift transcriber for every result/state change. Runs on
/// Swift's main actor; `AppHandle::emit` is thread-safe.
extern "C" fn voice_callback(_ctx: *mut c_void, kind: i32, text: *const c_char) {
    let Some(app) = APP.lock().unwrap().clone() else {
        return;
    };
    let s = if text.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    };
    let ev = match kind {
        KIND_TRANSCRIPT => VoiceEvent::Partial { text: s },
        KIND_RECORDING => VoiceEvent::State { state: "recording" },
        KIND_STOPPED => VoiceEvent::State { state: "stopped" },
        _ => VoiceEvent::Error { message: s }, // KIND_ERROR (3) or unknown
    };
    emit(&app, ev);
}

type VoiceCallback = extern "C" fn(*mut c_void, i32, *const c_char);

#[cfg(target_os = "macos")]
extern "C" {
    fn cenno_voice_start(ctx: *mut c_void, cb: VoiceCallback) -> i32;
    fn cenno_voice_stop();
}

/// Start push-to-talk dictation. Idempotent while recording (Swift restarts a
/// clean session). Permission / availability failures arrive as `voice-event`
/// errors via the callback.
#[tauri::command]
pub async fn voice_start(app: AppHandle) -> Result<(), String> {
    *APP.lock().unwrap() = Some(app.clone());
    #[cfg(target_os = "macos")]
    unsafe {
        cenno_voice_start(std::ptr::null_mut(), voice_callback);
    }
    #[cfg(not(target_os = "macos"))]
    emit(
        &app,
        VoiceEvent::Error { message: "voice input is only available on macOS".into() },
    );
    Ok(())
}

/// Stop dictation. Idempotent when idle.
#[tauri::command]
pub async fn voice_stop(_app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    unsafe {
        cenno_voice_stop();
    }
    Ok(())
}
