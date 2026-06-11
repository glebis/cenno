import SwiftUI
import CennoShared

/// Renders a single pending prompt on Watch, routing to the right input control.
struct PromptView: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @State private var answered = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                // cenno mark + flow color accent
                HStack {
                    CennoMark()
                    Spacer()
                    if let flow = prompt.payload.flow {
                        Circle()
                            .fill(FlowColor.color(for: flow))
                            .frame(width: 8, height: 8)
                    }
                }

                Text(prompt.payload.title)
                    .font(.headline)
                    .fixedSize(horizontal: false, vertical: true)

                if let body = prompt.payload.bodyMd {
                    Text(body)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }

                inputControl
            }
            .padding(.horizontal, 4)
        }
        .navigationTitle("cenno")
        .navigationBarTitleDisplayMode(.inline)
    }

    @ViewBuilder
    private var inputControl: some View {
        let kind = prompt.payload.input?.kind ?? "none"
        switch kind {
        case "confirm":
            ConfirmInput(prompt: prompt)
        case "choice":
            ChoiceInput(prompt: prompt)
        case "scale":
            ScaleInput(prompt: prompt)
        case "text", "voice_text":
            TextInput(prompt: prompt, voice: kind == "voice_text")
        default:
            // kind: none — auto-dismiss after showing
            Color.clear.onAppear {
                Task {
                    try? await Task.sleep(for: .seconds(3))
                    await relay.markTimedOut(promptID: prompt.id)
                }
            }
        }
    }
}

// MARK: - Input controls

private struct ConfirmInput: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay

    var body: some View {
        VStack(spacing: 8) {
            Button("Yes") { submit("yes") }
                .buttonStyle(.borderedProminent)
            Button("No") { submit("no") }
                .buttonStyle(.bordered)
        }
    }

    private func submit(_ value: String) {
        Task { await relay.submit(answer: .make(value, via: "choice", device: "watch"),
                                  for: prompt.id) }
    }
}

private struct ChoiceInput: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay

    var body: some View {
        let choices = prompt.payload.choices ?? []
        List(choices, id: \.self) { choice in
            Button(choice) {
                Task { await relay.submit(answer: .make(choice, via: "choice", device: "watch"),
                                          for: prompt.id) }
            }
        }
        .listStyle(.carousel)
    }
}

private struct ScaleInput: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @State private var value: Double = 4

    var body: some View {
        VStack(spacing: 8) {
            Text("\(Int(value))")
                .font(.title)
                .focusable()
                .digitalCrownRotation($value, from: 1, through: 7, by: 1,
                                       sensitivity: .medium, isContinuous: false)
            HStack {
                Text("1").font(.caption2).foregroundStyle(.secondary)
                Spacer()
                Text("7").font(.caption2).foregroundStyle(.secondary)
            }
            Button("Submit") {
                Task { await relay.submit(answer: .make("\(Int(value))", via: "scale", device: "watch"),
                                          for: prompt.id) }
            }
            .buttonStyle(.borderedProminent)
        }
    }
}

private struct TextInput: View {
    let prompt: PromptRecord
    let voice: Bool
    @EnvironmentObject var relay: CloudKitRelay
    @State private var text = ""

    var body: some View {
        VStack(spacing: 8) {
            // TextFieldLink opens system input screen (keyboard + dictation)
            TextFieldLink(prompt: Text(voice ? "Dictate or type…" : "Type your answer…")) { result in
                Task { await relay.submit(answer: .make(result, via: voice ? "voice_text" : "text",
                                                        device: "watch"),
                                          for: prompt.id) }
            }
            .buttonStyle(.borderedProminent)
        }
    }
}

// MARK: - Helpers

private struct CennoMark: View {
    var body: some View {
        Image(systemName: "questionmark.circle.fill")
            .font(.caption)
            .foregroundStyle(.secondary)
    }
}

private enum FlowColor {
    static func color(for flow: String) -> Color {
        switch flow {
        case "mood":     return Color(red: 1.0, green: 0.38, blue: 0.31)
        case "ema":      return Color(red: 0.0, green: 0.71, blue: 0.65)
        case "question": return Color(red: 0.22, green: 0.44, blue: 0.96)
        case "reminder": return Color(red: 0.43, green: 0.49, blue: 0.56)
        default:         return Color(red: 0.08, green: 0.09, blue: 0.10)
        }
    }
}

private extension PromptAnswer {
    static func make(_ answer: String, via: String, device: String) -> PromptAnswer {
        PromptAnswer(answer: answer, via: via, elapsedS: 0, device: device)
    }
}
