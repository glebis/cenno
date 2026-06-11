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
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use objc2_avf_audio::{AVAudioEngine, AVAudioFormat, AVAudioPCMBuffer, AVAudioTime};
    use objc2_foundation::NSError;
    use objc2_speech::{
        SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult,
        SFSpeechRecognitionTask, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
    };
    use tauri::AppHandle;

    use super::{emit, VoiceEvent};

    const MAX_RECORDING_SECS: u64 = 600; // 10 minutes
    /// Apple Speech finalizes a recognition request after ~1 minute (or at a
    /// speech pause), resetting its transcript to the new segment. To dictate
    /// for longer without losing a word, each finalized segment is committed
    /// and a fresh request is started. This bounds consecutive EMPTY restarts
    /// (silence, or a genuinely failing recognizer) so it can't spin forever —
    /// any real recognized speech resets the count.
    const MAX_EMPTY_RESTARTS: u32 = 4;

    /// Live recording session. Main-thread only (see module docs). The engine
    /// and recognizer persist across segments; `request`/`task` are swapped each
    /// time the recognizer finalizes. `committed` accumulates every finalized
    /// segment so no dictated word is ever dropped on a segment rollover.
    struct Session {
        engine: Retained<AVAudioEngine>,
        recognizer: Retained<SFSpeechRecognizer>,
        format: Retained<AVAudioFormat>,
        request: Retained<SFSpeechAudioBufferRecognitionRequest>,
        task: Retained<SFSpeechRecognitionTask>,
        /// Finalized segments so far. `Arc<Mutex<_>>` because the result
        /// handler runs off the main thread. Does NOT include the field text
        /// the user already had — the frontend prepends that.
        committed: Arc<Mutex<String>>,
    }

    thread_local! {
        static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    }

    /// Consecutive segment restarts that produced no new committed text.
    static EMPTY_RESTARTS: AtomicU32 = AtomicU32::new(0);

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

    /// Append `seg` to the committed transcript with single-space joins.
    fn commit_segment(committed: &Arc<Mutex<String>>, seg: &str) {
        if seg.is_empty() {
            return;
        }
        let mut c = committed.lock().unwrap();
        if !c.is_empty() && !c.ends_with(' ') {
            c.push(' ');
        }
        c.push_str(seg);
        c.push(' ');
    }

    /// Full transcript to display: everything committed plus the live segment.
    fn full_transcript(committed: &Arc<Mutex<String>>, seg: &str) -> String {
        let c = committed.lock().unwrap();
        if c.is_empty() {
            seg.to_string()
        } else if c.ends_with(' ') || seg.is_empty() {
            format!("{c}{seg}")
        } else {
            format!("{c} {seg}")
        }
    }

    /// Create a fresh recognition request + task on the existing engine and
    /// repoint the mic tap at it. Main thread only; reads the live `Session`.
    ///
    /// Called once at session start and again after every finalization, so a
    /// segment boundary (the recognizer's ~1-min / pause limit) is invisible:
    /// the committed text already holds the prior segments, and the new request
    /// picks up where the audio left off.
    fn start_segment(app: &AppHandle, generation: u64) {
        SESSION.with(|s| {
            let mut slot = s.borrow_mut();
            let Some(sess) = slot.as_mut() else { return };

            let request = unsafe { SFSpeechAudioBufferRecognitionRequest::new() };
            unsafe {
                request.setShouldReportPartialResults(true);
                request.setRequiresOnDeviceRecognition(true);
            }

            // Repoint the mic tap at the new request (no audible gap — segment
            // boundaries land on the silence that triggered finalization).
            let input = unsafe { sess.engine.inputNode() };
            unsafe { input.removeTapOnBus(0) };
            let tap_request = request.clone();
            let tap = RcBlock::new(
                move |buffer: NonNull<AVAudioPCMBuffer>, _when: NonNull<AVAudioTime>| unsafe {
                    tap_request.appendAudioPCMBuffer(buffer.as_ref());
                },
            );
            let tap_ptr = &*tap as *const block2::DynBlock<_> as *mut block2::DynBlock<_>;
            unsafe {
                input.installTapOnBus_bufferSize_format_block(0, 1024, Some(&sess.format), tap_ptr)
            };

            let cb_app = app.clone();
            let committed = sess.committed.clone();
            let handler = RcBlock::new(
                move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                    if GENERATION.load(Ordering::SeqCst) != generation {
                        return; // stale session (user stopped, restarted, or timed out)
                    }
                    if !result.is_null() {
                        let seg = unsafe { (*result).bestTranscription().formattedString() }
                            .to_string();
                        // Live view never shrinks: committed segments + this one.
                        emit(&cb_app, VoiceEvent::Partial { text: full_transcript(&committed, &seg) });
                        if unsafe { (*result).isFinal() } {
                            // Commit this segment and continue dictating with a
                            // fresh request — survives the recognizer's limit.
                            if seg.is_empty() {
                                if EMPTY_RESTARTS.fetch_add(1, Ordering::SeqCst) + 1
                                    >= MAX_EMPTY_RESTARTS
                                {
                                    return; // recognizer producing nothing; let it idle
                                }
                            } else {
                                EMPTY_RESTARTS.store(0, Ordering::SeqCst);
                                commit_segment(&committed, &seg);
                            }
                            let app2 = cb_app.clone();
                            let _ = app2.clone().run_on_main_thread(move || {
                                if GENERATION.load(Ordering::SeqCst) == generation {
                                    start_segment(&app2, generation);
                                }
                            });
                        }
                    } else if !error.is_null() {
                        // The ~1-min limit can arrive as an error rather than a
                        // final result. Treat it as a segment boundary and keep
                        // going (bounded by EMPTY_RESTARTS); only surface an
                        // error if nothing has been recognized at all.
                        if EMPTY_RESTARTS.fetch_add(1, Ordering::SeqCst) + 1 >= MAX_EMPTY_RESTARTS {
                            if committed.lock().unwrap().is_empty() {
                                let message =
                                    unsafe { (*error).localizedDescription() }.to_string();
                                emit(&cb_app, VoiceEvent::Error { message });
                                emit(&cb_app, VoiceEvent::State { state: "stopped" });
                            }
                            return;
                        }
                        let app2 = cb_app.clone();
                        let _ = app2.clone().run_on_main_thread(move || {
                            if GENERATION.load(Ordering::SeqCst) == generation {
                                start_segment(&app2, generation);
                            }
                        });
                    }
                },
            );
            let task = unsafe {
                sess.recognizer
                    .recognitionTaskWithRequest_resultHandler(&request, &handler)
            };

            // Finalize the previous segment's task (if any) and swap in the new.
            unsafe { sess.task.finish() };
            sess.request = request;
            sess.task = task;
        });
    }

    /// Build the engine + recognizer and start the first segment. Main thread.
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

        let engine = unsafe { AVAudioEngine::new() };
        let input = unsafe { engine.inputNode() };
        let format = unsafe { input.outputFormatForBus(0) };

        // A throwaway request/task seed the Session; start_segment immediately
        // replaces them with the real first segment (and installs the tap).
        let request = unsafe { SFSpeechAudioBufferRecognitionRequest::new() };
        let seed_handler = RcBlock::new(
            |_r: *mut SFSpeechRecognitionResult, _e: *mut NSError| {},
        );
        let task = unsafe {
            recognizer.recognitionTaskWithRequest_resultHandler(&request, &seed_handler)
        };

        unsafe { engine.prepare() };
        if let Err(e) = unsafe { engine.startAndReturnError() } {
            unsafe { task.cancel() };
            return Err(format!(
                "Could not start the microphone: {}",
                e.localizedDescription()
            ));
        }

        EMPTY_RESTARTS.store(0, Ordering::SeqCst);
        SESSION.with(|s| {
            *s.borrow_mut() = Some(Session {
                engine,
                recognizer,
                format,
                request,
                task,
                committed: Arc::new(Mutex::new(String::new())),
            })
        });
        // Install the real first segment (tap + result handler) over the seed.
        start_segment(&app, generation);
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

    #[cfg(test)]
    mod tests {
        use super::{commit_segment, full_transcript};
        use std::sync::{Arc, Mutex};

        fn buf() -> Arc<Mutex<String>> {
            Arc::new(Mutex::new(String::new()))
        }

        #[test]
        fn full_transcript_joins_committed_and_live_segment() {
            let c = buf();
            // Nothing committed yet: the live segment stands alone.
            assert_eq!(full_transcript(&c, "hello"), "hello");
            commit_segment(&c, "hello world");
            // Committed + live, single-spaced.
            assert_eq!(full_transcript(&c, "how are"), "hello world how are");
            // Empty live segment between utterances: just the committed text.
            assert_eq!(full_transcript(&c, ""), "hello world ");
        }

        #[test]
        fn segments_accumulate_and_never_drop_earlier_text() {
            // The exact failure the user hit: a later segment must never
            // overwrite earlier ones.
            let c = buf();
            commit_segment(&c, "the first thing I said");
            commit_segment(&c, "the second thing");
            commit_segment(&c, "and the third");
            // A fresh segment's live partial still shows everything before it.
            assert_eq!(
                full_transcript(&c, "plus a fourth"),
                "the first thing I said the second thing and the third plus a fourth"
            );
        }

        #[test]
        fn commit_segment_ignores_empty_and_normalizes_spacing() {
            let c = buf();
            commit_segment(&c, "");
            assert_eq!(*c.lock().unwrap(), "");
            commit_segment(&c, "one");
            commit_segment(&c, "two");
            // Exactly one space between committed segments, trailing space kept
            // so the next live segment joins cleanly.
            assert_eq!(*c.lock().unwrap(), "one two ");
        }
    }
}
