import SwiftUI
import CennoShared

/// Ambient "second screen" mode: a dedicated, fullscreen display that polls for
/// prompts routed to this device and surfaces them with a sound (and optional
/// spoken title). While nothing is pending it shows a calm clock or smiley.
///
/// Liveness is foreground polling on the existing CloudKit reader — no APNs.
struct SecondScreenView: View {
    @EnvironmentObject var relay: CloudKitRelay
    @ObservedObject var settings: SecondScreenSettings

    /// Disable the network poll loop in demo/screenshot mode (seeded data).
    var poll: Bool = true

    @State private var announcer = AmbientAnnouncer()
    @State private var announced: Set<String> = []

    /// The first pending prompt that should surface on THIS device right now.
    private var surfaced: PromptRecord? {
        relay.pendingPrompts.first { $0.shouldSurface(on: .current) }
    }

    var body: some View {
        ZStack {
            CennoTheme.surface(for: "ambient").ignoresSafeArea()
            if let prompt = surfaced {
                A2UIPromptView(prompt: prompt)
                    .transition(.opacity.combined(with: .scale(scale: 0.98)))
            } else {
                AmbientIdleView(face: settings.ambientFace)
                    .transition(.opacity)
            }
        }
        .animation(.easeInOut(duration: 0.25), value: surfaced?.id)
        .onChange(of: surfaced?.id) { _, id in announceIfNew(id) }
        .task(id: poll) { await pollLoop() }
    }

    /// Announce a prompt the first time it surfaces (sound + optional TTS).
    private func announceIfNew(_ id: String?) {
        guard let id, !announced.contains(id), let prompt = surfaced else { return }
        announced.insert(id)
        announcer.announce(
            title: prompt.payload.title,
            sound: settings.soundEnabled,
            speak: settings.ttsEnabled
        )
    }

    private func pollLoop() async {
        guard poll else { return }
        // Surface anything already pending immediately on entry.
        announceIfNew(surfaced?.id)
        while !Task.isCancelled {
            let seconds = UInt64(settings.pollIntervalS) * 1_000_000_000
            try? await Task.sleep(nanoseconds: seconds)
            if Task.isCancelled { break }
            await relay.fetchPending()
        }
    }
}

/// The calm idle state — a live clock or a friendly face.
struct AmbientIdleView: View {
    let face: AmbientFace

    var body: some View {
        VStack(spacing: 16) {
            switch face {
            case .clock:
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    Text(context.date, format: .dateTime.hour().minute())
                        .font(.system(size: 88, weight: .thin, design: .rounded))
                        .monospacedDigit()
                        .foregroundStyle(CennoTheme.ink)
                }
            case .smiley:
                Text(":)")
                    .font(.system(size: 120, weight: .bold, design: .rounded))
                    .foregroundStyle(CennoTheme.ink)
            }
            Text("Listening for your agents")
                .font(.footnote)
                .foregroundStyle(CennoTheme.inkDim)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Settings for the ambient second screen, presented as a sheet from the queue.
struct SecondScreenSettingsView: View {
    @ObservedObject var settings: SecondScreenSettings
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Toggle("Act as a second screen", isOn: $settings.enabled)
                } footer: {
                    Text("Show a fullscreen ambient display that surfaces prompts routed here, even while you work elsewhere.")
                }
                Section("When a prompt arrives") {
                    Toggle("Play a sound", isOn: $settings.soundEnabled)
                    Toggle("Speak the question aloud", isOn: $settings.ttsEnabled)
                }
                Section("Idle display") {
                    Picker("Show", selection: $settings.ambientFace) {
                        ForEach(AmbientFace.allCases) { Text($0.label).tag($0) }
                    }
                }
                Section {
                    Stepper("Check every \(settings.pollIntervalS)s",
                            value: $settings.pollIntervalS,
                            in: SecondScreenSettings.pollRange)
                } footer: {
                    Text("How often this device polls for new prompts (\(SecondScreenSettings.pollRange.lowerBound)–\(SecondScreenSettings.pollRange.upperBound)s).")
                }
            }
            .navigationTitle("Second screen")
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}
