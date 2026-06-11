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
        /// Accumulated dictation. `Arc<Mutex<_>>` because the result handler
        /// runs off the main thread. Does NOT include the field text the user
        /// already had — the frontend prepends that.
        transcript: Arc<Mutex<Transcript>>,
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

    /// Accumulated dictation. `committed` holds utterances the recognizer has
    /// moved past; `segment` is the current live utterance (its high-water
    /// text). Apple's on-device recognizer auto-segments on a speech pause and
    /// RESETS `formattedString` to the new utterance WITHOUT an isFinal — so we
    /// detect the rollback ourselves and fold the prior utterance into
    /// `committed` before adopting the new one. Nothing is ever dropped.
    #[derive(Default)]
    struct Transcript {
        committed: String,
        segment: String,
    }

    /// Length of the shared leading run of two strings, in chars.
    fn common_prefix_len(a: &str, b: &str) -> usize {
        a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
    }

    /// Is `new` a continuation of the live `segment`, or a fresh utterance?
    ///
    /// Within one utterance the recognizer's text either grows (`new` extends
    /// `segment`), trims its tail while revising (`segment` extends `new`), or
    /// revises a word in place (most of the leading text is preserved). A
    /// pause-reset starts a new utterance that shares little with what came
    /// before. We deliberately bias toward "reset": a false reset only
    /// duplicates a little text (the user can edit), whereas a false
    /// continuation would OVERWRITE — losing dictated words, which is the one
    /// outcome we must never allow.
    fn is_continuation(segment: &str, new: &str) -> bool {
        if segment.is_empty() {
            return true;
        }
        if new.starts_with(segment) || segment.starts_with(new) {
            return true; // clean growth or tail-trim revision
        }
        // Word-level revision keeps most of the previous text as a shared
        // prefix; require >= 60% of the previous segment preserved.
        common_prefix_len(segment, new) * 5 >= segment.chars().count() * 3
    }

    impl Transcript {
        /// Fold `seg` into the committed text with single-space joins.
        fn commit(&mut self, seg: &str) {
            if seg.is_empty() {
                return;
            }
            if !self.committed.is_empty() && !self.committed.ends_with(' ') {
                self.committed.push(' ');
            }
            self.committed.push_str(seg);
            self.committed.push(' ');
        }

        /// Apply a partial transcript; returns the full text to display. Commits
        /// the prior utterance first if the recognizer rolled over to a new one.
        fn apply(&mut self, new: &str) -> String {
            if self.segment.is_empty() {
                self.segment = new.to_string();
            } else if is_continuation(&self.segment, new) {
                // Keep the high-water text so a transient shorter revision never
                // loses the tail.
                if new.chars().count() >= self.segment.chars().count() {
                    self.segment = new.to_string();
                }
            } else {
                // Pause-reset: the prior utterance is done — bank it, start anew.
                let prior = std::mem::take(&mut self.segment);
                self.commit(&prior);
                self.segment = new.to_string();
            }
            self.display()
        }

        /// Finalize the live utterance (isFinal / stop): bank it and clear.
        fn finalize(&mut self) {
            let seg = std::mem::take(&mut self.segment);
            self.commit(&seg);
        }

        /// committed + live segment, single-spaced.
        fn display(&self) -> String {
            if self.committed.is_empty() {
                self.segment.clone()
            } else if self.committed.ends_with(' ') || self.segment.is_empty() {
                format!("{}{}", self.committed, self.segment)
            } else {
                format!("{} {}", self.committed, self.segment)
            }
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
            let transcript = sess.transcript.clone();
            let handler = RcBlock::new(
                move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                    if GENERATION.load(Ordering::SeqCst) != generation {
                        return; // stale session (user stopped, restarted, or timed out)
                    }
                    if !result.is_null() {
                        let seg = unsafe { (*result).bestTranscription().formattedString() }
                            .to_string();
                        let is_final = unsafe { (*result).isFinal() };
                        // apply() handles pause-resets (no isFinal) by banking the
                        // prior utterance; the live view therefore never shrinks.
                        let display = {
                            let mut t = transcript.lock().unwrap();
                            let d = t.apply(&seg);
                            if is_final {
                                t.finalize();
                            }
                            d
                        };
                        emit(&cb_app, VoiceEvent::Partial { text: display });
                        if !seg.is_empty() {
                            EMPTY_RESTARTS.store(0, Ordering::SeqCst);
                        }
                        if is_final {
                            // The ~1-min hard limit: continue dictating on a fresh
                            // request unless the recognizer has gone silent.
                            if seg.is_empty()
                                && EMPTY_RESTARTS.fetch_add(1, Ordering::SeqCst) + 1
                                    >= MAX_EMPTY_RESTARTS
                            {
                                return;
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
                        // final result. Bank the live utterance, then continue on
                        // a fresh request (bounded by EMPTY_RESTARTS); surface an
                        // error only if nothing has been recognized at all.
                        transcript.lock().unwrap().finalize();
                        if EMPTY_RESTARTS.fetch_add(1, Ordering::SeqCst) + 1 >= MAX_EMPTY_RESTARTS {
                            if transcript.lock().unwrap().committed.is_empty() {
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
                transcript: Arc::new(Mutex::new(Transcript::default())),
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
        use super::Transcript;

        #[test]
        fn growing_partials_within_one_utterance_just_extend() {
            let mut t = Transcript::default();
            assert_eq!(t.apply("I"), "I");
            assert_eq!(t.apply("I went"), "I went");
            assert_eq!(t.apply("I went to"), "I went to");
            assert_eq!(t.apply("I went to the store"), "I went to the store");
            // Nothing committed yet — it's all one live utterance.
            assert_eq!(t.committed, "");
        }

        #[test]
        fn tail_trim_revision_keeps_the_high_water_text() {
            let mut t = Transcript::default();
            t.apply("I went to the store");
            // Recognizer briefly trims its tail while revising — must not lose it.
            assert_eq!(t.apply("I went to the"), "I went to the store");
        }

        #[test]
        fn pause_reset_without_final_banks_the_prior_utterance() {
            // THE bug the user hit: after a pause the recognizer resets
            // formattedString to the new utterance with NO isFinal. Earlier
            // text must be banked, not lost.
            let mut t = Transcript::default();
            t.apply("the first long thing I said before pausing");
            // New short utterance that is not a continuation → reset.
            assert_eq!(
                t.apply("But"),
                "the first long thing I said before pausing But"
            );
            assert_eq!(t.apply("But maybe"), "the first long thing I said before pausing But maybe");
            assert_eq!(
                t.apply("But maybe it's fine"),
                "the first long thing I said before pausing But maybe it's fine"
            );
        }

        #[test]
        fn many_pause_resets_accumulate_everything() {
            let mut t = Transcript::default();
            t.apply("first sentence here");
            t.apply("a second sentence"); // reset (not a continuation)
            t.apply("and a third one entirely"); // reset
            t.apply("tiny"); // reset
            assert_eq!(
                t.apply("tiny tail"),
                "first sentence here a second sentence and a third one entirely tiny tail"
            );
        }

        #[test]
        fn word_revision_mid_utterance_is_not_a_reset() {
            // "stork" → "store": neither is a prefix of the other, but they
            // share most of the leading text — a revision, not a new utterance.
            let mut t = Transcript::default();
            t.apply("I went to the stork");
            assert_eq!(t.apply("I went to the store"), "I went to the store");
            assert_eq!(t.committed, "", "a revision must not bank anything");
        }

        #[test]
        fn finalize_banks_the_live_utterance() {
            let mut t = Transcript::default();
            t.apply("hello world");
            t.finalize();
            assert_eq!(t.committed, "hello world ");
            assert_eq!(t.segment, "");
            // A following utterance continues after the banked text.
            assert_eq!(t.apply("again"), "hello world again");
        }
    }
}
