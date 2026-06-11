//! voice.rs — push-to-talk dictation for `voice_text` prompts.
//!
//! Apple Speech (`SFSpeechRecognizer`) fed by an `AVAudioEngine` mic tap,
//! strictly on-device (`requiresOnDeviceRecognition`) — no network call, no
//! API key. The webview toggles recording with the `voice_start` /
//! `voice_stop` commands and receives `voice-event` emissions:
//!
//!   {type:"state", state:"recording"|"stopped"}
//!   {type:"partial", text}          — full session transcript so far
//!   {type:"error", message}         — graceful error state (permissions,
//!                                     locale without on-device support, …)
//!
//! Threading model: every ObjC object lives on the main thread only (none of
//! them are `Send`); commands hop there via `run_on_main_thread`. Callback
//! blocks (recognition results, audio taps) run on framework-owned queues and
//! touch nothing but the `AppHandle` (Send + Sync) and a generation counter
//! that makes stale callbacks inert. Recording is capped at
//! `MAX_RECORDING_SECS` by a watchdog; `timeout_s` on the prompt keeps
//! governing the panel lifetime, not the recording.

use tauri::{AppHandle, Emitter};

pub const EVENT: &str = "voice-event";

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceEvent {
    State { state: &'static str },
    Partial { text: String },
    Error { message: String },
}

fn emit(app: &AppHandle, ev: VoiceEvent) {
    let _ = app.emit(EVENT, ev);
}

/// Start push-to-talk dictation. Idempotent while recording (restarts the
/// session). Permission and availability failures surface BOTH as the command
/// error and as a `voice-event` error so the panel can show its graceful
/// error state.
#[tauri::command]
pub async fn voice_start(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let outcome = mac::start(app.clone()).await;
        if let Err(ref message) = outcome {
            emit(&app, VoiceEvent::Error { message: message.clone() });
            emit(&app, VoiceEvent::State { state: "stopped" });
        }
        outcome
    }
    #[cfg(not(target_os = "macos"))]
    {
        let message = "voice input is only available on macOS".to_string();
        emit(&app, VoiceEvent::Error { message: message.clone() });
        Err(message)
    }
}

/// Stop dictation. Idempotent when idle.
#[tauri::command]
pub async fn voice_stop(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        mac::stop(app).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use objc2_avf_audio::{AVAudioEngine, AVAudioPCMBuffer, AVAudioTime};
    use objc2_foundation::NSError;
    use objc2_speech::{
        SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult,
        SFSpeechRecognitionTask, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
    };
    use tauri::AppHandle;

    use super::{emit, VoiceEvent};

    const MAX_RECORDING_SECS: u64 = 600; // 10 minutes

    /// Live recording session. Main-thread only (see module docs).
    struct Session {
        engine: Retained<AVAudioEngine>,
        request: Retained<SFSpeechAudioBufferRecognitionRequest>,
        task: Retained<SFSpeechRecognitionTask>,
        // The recognizer must outlive its task.
        _recognizer: Retained<SFSpeechRecognizer>,
    }

    thread_local! {
        static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    }

    /// Bumped on every start/stop; callbacks capture the generation at start
    /// and go inert when it moves on.
    static GENERATION: AtomicU64 = AtomicU64::new(0);

    /// Ask for speech-recognition authorization if undetermined.
    async fn speech_authorized() -> Result<(), String> {
        const DENIED: &str = "Speech recognition permission denied — enable it in System Settings → Privacy & Security → Speech Recognition.";
        let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
        if status == SFSpeechRecognizerAuthorizationStatus::Authorized {
            return Ok(());
        }
        if status != SFSpeechRecognizerAuthorizationStatus::NotDetermined {
            return Err(DENIED.into());
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<SFSpeechRecognizerAuthorizationStatus>();
        {
            // Scoped so the (non-Send) block is dropped before the await; the
            // runtime copies escaping blocks, so ours need not outlive the call.
            let tx = Mutex::new(Some(tx));
            let handler = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(status);
                }
            });
            unsafe { SFSpeechRecognizer::requestAuthorization(&handler) };
        }
        match rx.await {
            Ok(SFSpeechRecognizerAuthorizationStatus::Authorized) => Ok(()),
            _ => Err(DENIED.into()),
        }
    }

    /// Ask for microphone authorization if undetermined.
    async fn mic_authorized() -> Result<(), String> {
        const DENIED: &str = "Microphone permission denied — enable it in System Settings → Privacy & Security → Microphone.";
        let status = {
            let media = unsafe { AVMediaTypeAudio }.ok_or("AVMediaTypeAudio unavailable")?;
            unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) }
        };
        if status == AVAuthorizationStatus::Authorized {
            return Ok(());
        }
        if status != AVAuthorizationStatus::NotDetermined {
            return Err(DENIED.into());
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        {
            // Scoped: neither the block nor the AVMediaType ref may cross the
            // await (they are not Send); the runtime copies escaping blocks.
            let media = unsafe { AVMediaTypeAudio }.ok_or("AVMediaTypeAudio unavailable")?;
            let tx = Mutex::new(Some(tx));
            let handler = RcBlock::new(move |granted: objc2::runtime::Bool| {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(granted.as_bool());
                }
            });
            unsafe {
                AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &handler)
            };
        }
        match rx.await {
            Ok(true) => Ok(()),
            _ => Err(DENIED.into()),
        }
    }

    /// Build and start the whole pipeline. Main thread only.
    fn start_session(app: AppHandle, generation: u64) -> Result<(), String> {
        let recognizer = unsafe { SFSpeechRecognizer::new() };
        if !unsafe { recognizer.isAvailable() } {
            return Err("Speech recognition is currently unavailable.".into());
        }
        if !unsafe { recognizer.supportsOnDeviceRecognition() } {
            return Err(
                "On-device speech recognition is not available for your locale — cenno never sends audio to the network, so dictation is disabled.".into(),
            );
        }

        let request = unsafe { SFSpeechAudioBufferRecognitionRequest::new() };
        unsafe {
            request.setShouldReportPartialResults(true);
            request.setRequiresOnDeviceRecognition(true);
        }

        let engine = unsafe { AVAudioEngine::new() };
        let input = unsafe { engine.inputNode() };
        let format = unsafe { input.outputFormatForBus(0) };

        // Feed mic buffers to the recognition request. The tap block runs on
        // the audio thread; appending to a buffer recognition request is the
        // documented streaming pattern.
        let tap_request = request.clone();
        let tap = RcBlock::new(
            move |buffer: NonNull<AVAudioPCMBuffer>, _when: NonNull<AVAudioTime>| unsafe {
                tap_request.appendAudioPCMBuffer(buffer.as_ref());
            },
        );
        // The generated binding takes the tap block as a raw pointer
        // (AVAudioNodeTapBlock typedef); the engine copies it on install.
        let tap_ptr = &*tap as *const block2::DynBlock<_> as *mut block2::DynBlock<_>;
        unsafe { input.installTapOnBus_bufferSize_format_block(0, 1024, Some(&format), tap_ptr) };

        unsafe { engine.prepare() };
        if let Err(e) = unsafe { engine.startAndReturnError() } {
            unsafe { input.removeTapOnBus(0) };
            return Err(format!(
                "Could not start the microphone: {}",
                e.localizedDescription()
            ));
        }

        // Stream results back. Runs on a framework queue; generation-guarded.
        let cb_app = app.clone();
        let handler = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                if GENERATION.load(Ordering::SeqCst) != generation {
                    return; // stale session
                }
                if !result.is_null() {
                    let text =
                        unsafe { (*result).bestTranscription().formattedString() }.to_string();
                    emit(&cb_app, VoiceEvent::Partial { text });
                    if unsafe { (*result).isFinal() } {
                        emit(&cb_app, VoiceEvent::State { state: "stopped" });
                    }
                } else if !error.is_null() {
                    let message = unsafe { (*error).localizedDescription() }.to_string();
                    emit(&cb_app, VoiceEvent::Error { message });
                    emit(&cb_app, VoiceEvent::State { state: "stopped" });
                }
            },
        );
        let task =
            unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };

        SESSION.with(|s| {
            *s.borrow_mut() = Some(Session {
                engine,
                request,
                task,
                _recognizer: recognizer,
            })
        });
        emit(&app, VoiceEvent::State { state: "recording" });
        Ok(())
    }

    /// Tear down the live session, if any. Main thread only.
    fn teardown() -> bool {
        let session = SESSION.with(|s| s.borrow_mut().take());
        match session {
            Some(session) => {
                unsafe {
                    session.engine.stop();
                    session.engine.inputNode().removeTapOnBus(0);
                    session.request.endAudio();
                    session.task.finish();
                }
                true
            }
            None => false,
        }
    }

    /// Run `f` on the main thread and await its result.
    async fn on_main<T: Send + 'static>(
        app: &AppHandle,
        f: impl FnOnce(AppHandle) -> T + Send + 'static,
    ) -> Result<T, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let app2 = app.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(f(app2));
        })
        .map_err(|e| e.to_string())?;
        rx.await.map_err(|e| e.to_string())
    }

    pub async fn start(app: AppHandle) -> Result<(), String> {
        speech_authorized().await?;
        mic_authorized().await?;
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        on_main(&app, move |app| {
            // A start while recording restarts cleanly (the generation moved
            // on, so the old session's callbacks are already inert).
            teardown();
            start_session(app, generation)
        })
        .await??;

        // Watchdog: hard cap on recording length. The prompt's timeout_s
        // governs the panel; this governs only the mic.
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(MAX_RECORDING_SECS)).await;
            if GENERATION.load(Ordering::SeqCst) == generation {
                let _ = on_main(&app2, |app| {
                    GENERATION.fetch_add(1, Ordering::SeqCst);
                    if teardown() {
                        emit(&app, VoiceEvent::State { state: "stopped" });
                    }
                })
                .await;
            }
        });
        Ok(())
    }

    pub async fn stop(app: AppHandle) -> Result<(), String> {
        on_main(&app, |app| {
            GENERATION.fetch_add(1, Ordering::SeqCst);
            if teardown() {
                emit(&app, VoiceEvent::State { state: "stopped" });
            }
        })
        .await
    }
}
