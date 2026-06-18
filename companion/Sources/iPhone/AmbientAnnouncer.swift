import Foundation
import AVFoundation
import AudioToolbox

/// Plays the arrival sound and (optionally) speaks the prompt title aloud when a
/// prompt surfaces on the ambient second screen. All local — no network, no
/// model download (uses the system speech synthesizer).
@MainActor
final class AmbientAnnouncer {
    private let synthesizer = AVSpeechSynthesizer()

    /// Announce a newly-surfaced prompt.
    /// - sound: play the arrival chime (mandatory per design; toggle is the user's).
    /// - speak: read `title` aloud via the built-in synthesizer.
    func announce(title: String, sound: Bool, speak: Bool) {
        if sound { playArrivalSound() }
        if speak { speakAloud(title) }
    }

    private func playArrivalSound() {
        // A bundled chime is preferred; fall back to a system sound so the
        // mandatory "you have a prompt" cue always fires even without the asset.
        if let url = Bundle.main.url(forResource: "prompt_arrival", withExtension: "caf") {
            var soundID: SystemSoundID = 0
            AudioServicesCreateSystemSoundID(url as CFURL, &soundID)
            AudioServicesPlaySystemSound(soundID)
        } else {
            // 1007 = a short, attention-getting system alert tone.
            AudioServicesPlaySystemSound(1007)
        }
    }

    private func speakAloud(_ text: String) {
        // Strip simple Markdown emphasis so the synthesizer doesn't read asterisks.
        let plain = text.replacingOccurrences(of: "*", with: "")
                        .replacingOccurrences(of: "_", with: "")
                        .replacingOccurrences(of: "`", with: "")
        let utterance = AVSpeechUtterance(string: plain)
        utterance.rate = AVSpeechUtteranceDefaultSpeechRate
        synthesizer.speak(utterance)
    }
}
