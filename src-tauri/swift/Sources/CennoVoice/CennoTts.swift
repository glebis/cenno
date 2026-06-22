// CennoTts — on-device text-to-speech ("sound-out"), bridged to Rust.
//
// The other direction from CennoVoice: instead of transcribing the user, this
// speaks a prompt aloud via Apple's AVSpeechSynthesizer. On-device, no network,
// no API key — the system's installed voices. The webview has already gated on
// urgency and normalized markdown/identifiers into speakable text, so this is a
// thin wrapper: speak(text) and stop().
//
// FFI: Rust calls cenno_tts_speak(text) and cenno_tts_stop().

import AVFoundation
import Foundation

// The synthesizer must outlive the call that starts speaking, so it's held in a
// process-global. AVSpeechSynthesizer is not Sendable; all access is hopped to
// the main queue, which serializes it.
nonisolated(unsafe) private var gSynth: AVSpeechSynthesizer?

@_cdecl("cenno_tts_speak")
public func cenno_tts_speak(_ text: UnsafePointer<CChar>?, _ voiceId: UnsafePointer<CChar>?) {
    let str = text.map { String(cString: $0) } ?? ""
    guard !str.isEmpty else { return }
    let requestedVoice = voiceId.map { String(cString: $0) }
    DispatchQueue.main.async {
        let synth = gSynth ?? AVSpeechSynthesizer()
        gSynth = synth
        // A fresh prompt supersedes any prior one still being read.
        synth.stopSpeaking(at: .immediate)
        let utterance = AVSpeechUtterance(string: str)
        if let voice = chooseVoice(requestedVoice) { utterance.voice = voice }
        synth.speak(utterance)
    }
}

/// Pick the voice to speak with. A configured identifier wins if it resolves;
/// otherwise auto-select the best-quality installed English voice (premium >
/// enhanced) rather than the plain system default, which sounds robotic.
private func chooseVoice(_ requested: String?) -> AVSpeechSynthesisVoice? {
    if let id = requested, !id.isEmpty, let v = AVSpeechSynthesisVoice(identifier: id) {
        return v
    }
    let english = AVSpeechSynthesisVoice.speechVoices().filter { $0.language.hasPrefix("en") }
    return english.first { $0.quality == .premium }
        ?? english.first { $0.quality == .enhanced }
        ?? AVSpeechSynthesisVoice(language: "en-US")
}

@_cdecl("cenno_tts_stop")
public func cenno_tts_stop() {
    DispatchQueue.main.async {
        gSynth?.stopSpeaking(at: .immediate)
    }
}
