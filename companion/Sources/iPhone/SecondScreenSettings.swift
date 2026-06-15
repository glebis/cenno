import Foundation
import Combine

/// What the ambient second screen shows while idle (no prompt to surface).
public enum AmbientFace: String, CaseIterable, Identifiable {
    case clock, smiley
    public var id: String { rawValue }
    public var label: String { self == .clock ? "Clock" : "Smiley" }
}

/// Per-device "second screen" settings, persisted locally in UserDefaults.
/// Independent of the Mac's routing policy: the Mac decides whether a prompt is
/// *eligible* to reach this device; these settings decide how this device, when
/// acting as an ambient display, surfaces it.
@MainActor
final class SecondScreenSettings: ObservableObject {
    private let defaults: UserDefaults
    private enum Key {
        static let enabled = "secondScreen.enabled"
        static let poll = "secondScreen.pollIntervalS"
        static let sound = "secondScreen.soundEnabled"
        static let tts = "secondScreen.ttsEnabled"
        static let face = "secondScreen.ambientFace"
    }

    /// Poll cadence is clamped to a sane 15–30s window (battery vs. liveness).
    static let pollRange: ClosedRange<Int> = 15...30

    @Published var enabled: Bool { didSet { defaults.set(enabled, forKey: Key.enabled) } }
    @Published var pollIntervalS: Int { didSet {
        pollIntervalS = min(max(pollIntervalS, Self.pollRange.lowerBound), Self.pollRange.upperBound)
        defaults.set(pollIntervalS, forKey: Key.poll)
    } }
    @Published var soundEnabled: Bool { didSet { defaults.set(soundEnabled, forKey: Key.sound) } }
    @Published var ttsEnabled: Bool { didSet { defaults.set(ttsEnabled, forKey: Key.tts) } }
    @Published var ambientFace: AmbientFace { didSet { defaults.set(ambientFace.rawValue, forKey: Key.face) } }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        self.enabled = defaults.bool(forKey: Key.enabled)
        let storedPoll = defaults.object(forKey: Key.poll) as? Int ?? 20
        self.pollIntervalS = min(max(storedPoll, Self.pollRange.lowerBound), Self.pollRange.upperBound)
        // Sound defaults ON, TTS defaults OFF (absent key → default).
        self.soundEnabled = defaults.object(forKey: Key.sound) as? Bool ?? true
        self.ttsEnabled = defaults.bool(forKey: Key.tts)
        self.ambientFace = AmbientFace(rawValue: defaults.string(forKey: Key.face) ?? "") ?? .clock
    }
}
