// CennoVoice — Apple SpeechAnalyzer/SpeechTranscriber dictation, bridged to Rust.
//
// Why this exists: SFSpeechRecognizer (the Speech framework's old API) caps at
// ~1 minute and streams volatile, self-revising partials that are impossible to
// stitch into a stable transcript. SpeechTranscriber (macOS 26+) is built for
// long-form on-device dictation and separates VOLATILE (live progress) from
// FINALIZED (locked) results natively — so we just forward its text.
//
// FFI: Rust calls cenno_voice_start(ctx, callback) and cenno_voice_stop().
// `callback` is a C function pointer invoked with the full current transcript
// (finalized + volatile) and a kind tag.

import AVFoundation
import Foundation
import Speech

// Callback kinds (kept in sync with voice.rs).
private let KIND_TRANSCRIPT: Int32 = 0
private let KIND_RECORDING: Int32 = 1
private let KIND_STOPPED: Int32 = 2
private let KIND_ERROR: Int32 = 3

public typealias VoiceCallback = @convention(c) (
    UnsafeMutableRawPointer?, Int32, UnsafePointer<CChar>?
) -> Void

/// 10-minute hard cap on a single recording (product limit, not an API one).
private let MAX_RECORDING_SECS: UInt64 = 600

private struct VoiceError: Error {
    let message: String
}

/// Holds the live session. Untyped global so the file's top level needs no
/// availability annotation; the @_cdecl entry points gate on macOS 26.
nonisolated(unsafe) private var gManager: Any?

@available(macOS 26.0, *)
@MainActor
private final class VoiceManager {
    private let ctx: UnsafeMutableRawPointer?
    private let cb: VoiceCallback

    private var transcriber: SpeechTranscriber?
    private var analyzer: SpeechAnalyzer?
    private var analyzerFormat: AVAudioFormat?
    private let audioEngine = AVAudioEngine()
    private let converter = BufferConverter()
    private var inputBuilder: AsyncStream<AnalyzerInput>.Continuation?
    private var resultsTask: Task<Void, Never>?
    private var watchdog: Task<Void, Never>?

    private var finalized = ""
    private var volatileText = ""

    private static let locale = Locale(identifier: "en-US")

    init(ctx: UnsafeMutableRawPointer?, cb: @escaping VoiceCallback) {
        self.ctx = ctx
        self.cb = cb
    }

    private func emit(_ kind: Int32, _ text: String) {
        text.withCString { cb(ctx, kind, $0) }
    }

    func start() {
        Task { @MainActor in
            do {
                try await requestPermissions()
                try await setUp()
                try startAudio()
                emit(KIND_RECORDING, "")
                watchdog = Task { @MainActor in
                    try? await Task.sleep(nanoseconds: MAX_RECORDING_SECS * 1_000_000_000)
                    if !Task.isCancelled { self.stop() }
                }
            } catch let e as VoiceError {
                emit(KIND_ERROR, e.message)
                emit(KIND_STOPPED, "")
            } catch {
                emit(KIND_ERROR, error.localizedDescription)
                emit(KIND_STOPPED, "")
            }
        }
    }

    func stop() {
        watchdog?.cancel()
        watchdog = nil
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        inputBuilder?.finish()
        let analyzer = self.analyzer
        let task = self.resultsTask
        self.resultsTask = nil
        Task { @MainActor in
            try? await analyzer?.finalizeAndFinishThroughEndOfInput()
            task?.cancel()
            emit(KIND_STOPPED, "")
        }
    }

    private func requestPermissions() async throws {
        let speech = await withCheckedContinuation { (c: CheckedContinuation<SFSpeechRecognizerAuthorizationStatus, Never>) in
            SFSpeechRecognizer.requestAuthorization { c.resume(returning: $0) }
        }
        guard speech == .authorized else {
            throw VoiceError(
                message:
                    "Speech recognition permission denied — enable it in System Settings › Privacy & Security › Speech Recognition.")
        }
        let mic = await withCheckedContinuation { (c: CheckedContinuation<Bool, Never>) in
            AVCaptureDevice.requestAccess(for: .audio) { c.resume(returning: $0) }
        }
        guard mic else {
            throw VoiceError(
                message:
                    "Microphone permission denied — enable it in System Settings › Privacy & Security › Microphone.")
        }
    }

    private func setUp() async throws {
        let transcriber = SpeechTranscriber(
            locale: Self.locale,
            transcriptionOptions: [],
            reportingOptions: [.volatileResults],
            attributeOptions: [])
        self.transcriber = transcriber

        let analyzer = SpeechAnalyzer(modules: [transcriber])
        self.analyzer = analyzer

        try await ensureModel(transcriber: transcriber, locale: Self.locale)

        guard
            let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber])
        else {
            throw VoiceError(message: "No compatible audio format for on-device transcription.")
        }
        self.analyzerFormat = format

        let (stream, continuation) = AsyncStream<AnalyzerInput>.makeStream()
        self.inputBuilder = continuation

        // Stream results: finalized accumulates, volatile is the live tail.
        resultsTask = Task { @MainActor in
            do {
                for try await result in transcriber.results {
                    let text = String(result.text.characters)
                    if result.isFinal {
                        if !self.finalized.isEmpty && !self.finalized.hasSuffix(" ") {
                            self.finalized += " "
                        }
                        self.finalized += text
                        self.volatileText = ""
                    } else {
                        self.volatileText = text
                    }
                    self.emit(KIND_TRANSCRIPT, self.fullText())
                }
            } catch {
                self.emit(KIND_ERROR, error.localizedDescription)
            }
        }

        try await analyzer.start(inputSequence: stream)
    }

    private func fullText() -> String {
        if volatileText.isEmpty { return finalized }
        if finalized.isEmpty { return volatileText }
        if finalized.hasSuffix(" ") { return finalized + volatileText }
        return finalized + " " + volatileText
    }

    private func startAudio() throws {
        let input = audioEngine.inputNode
        let micFormat = input.outputFormat(forBus: 0)
        guard let analyzerFormat else {
            throw VoiceError(message: "Transcriber audio format unavailable.")
        }
        input.installTap(onBus: 0, bufferSize: 4096, format: micFormat) { [weak self] buffer, _ in
            guard let self else { return }
            if let converted = try? self.converter.convertBuffer(buffer, to: analyzerFormat) {
                self.inputBuilder?.yield(AnalyzerInput(buffer: converted))
            }
        }
        audioEngine.prepare()
        try audioEngine.start()
    }

    // MARK: - Model assets

    private func ensureModel(transcriber: SpeechTranscriber, locale: Locale) async throws {
        if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            try await request.downloadAndInstall()
        }
        let reserved = await AssetInventory.reservedLocales
        if !reserved.contains(where: { $0.identifier(.bcp47) == locale.identifier(.bcp47) }) {
            try await AssetInventory.reserve(locale: locale)
        }
    }
}

// MARK: - C ABI entry points

@_cdecl("cenno_voice_start")
public func cenno_voice_start(
    _ ctx: UnsafeMutableRawPointer?, _ cb: VoiceCallback
) -> Int32 {
    if #available(macOS 26.0, *) {
        // Hop to the main actor: VoiceManager and the audio/speech stack are
        // MainActor-isolated. Errors flow back via the callback, so the sync
        // return value is just "dispatched ok".
        Task { @MainActor in
            let m = VoiceManager(ctx: ctx, cb: cb)
            gManager = m
            m.start()
        }
        return 0
    } else {
        let msg =
            "Voice dictation needs macOS 26 or later (on-device SpeechTranscriber)."
        msg.withCString { cb(ctx, KIND_ERROR, $0) }
        return 1
    }
}

@_cdecl("cenno_voice_stop")
public func cenno_voice_stop() {
    Task { @MainActor in
        if #available(macOS 26.0, *) {
            (gManager as? VoiceManager)?.stop()
        }
        gManager = nil
    }
}

// MARK: - Buffer conversion (mic format → analyzer format)

private final class BufferConverter {
    private var converter: AVAudioConverter?

    func convertBuffer(_ buffer: AVAudioPCMBuffer, to format: AVAudioFormat) throws
        -> AVAudioPCMBuffer
    {
        let inputFormat = buffer.format
        guard inputFormat != format else { return buffer }

        if converter == nil || converter?.outputFormat != format {
            converter = AVAudioConverter(from: inputFormat, to: format)
            converter?.primeMethod = .none
        }
        guard let converter else { throw VoiceError(message: "Could not create audio converter.") }

        let ratio = converter.outputFormat.sampleRate / converter.inputFormat.sampleRate
        let capacity = AVAudioFrameCount((Double(buffer.frameLength) * ratio).rounded(.up))
        guard
            let out = AVAudioPCMBuffer(pcmFormat: converter.outputFormat, frameCapacity: capacity)
        else {
            throw VoiceError(message: "Could not create conversion buffer.")
        }

        var nsError: NSError?
        var fed = false
        let status = converter.convert(to: out, error: &nsError) { _, inputStatus in
            if fed {
                inputStatus.pointee = .noDataNow
                return nil
            }
            fed = true
            inputStatus.pointee = .haveData
            return buffer
        }
        if status == .error { throw VoiceError(message: nsError?.localizedDescription ?? "Audio conversion failed.") }
        return out
    }
}
