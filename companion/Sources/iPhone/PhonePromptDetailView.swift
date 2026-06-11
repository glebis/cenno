import SwiftUI
import CennoShared

struct PhonePromptDetailView: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss
    @State private var textInput = ""

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text(prompt.payload.title)
                    .font(.title2.bold())

                if let body = prompt.payload.bodyMd {
                    Text(body)
                        .foregroundStyle(.secondary)
                }

                inputControl
            }
            .padding()
        }
        .navigationTitle(flowLabel)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Skip") {
                    Task {
                        await relay.markTimedOut(promptID: prompt.id)
                        dismiss()
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var inputControl: some View {
        let kind = prompt.payload.input?.kind ?? "none"
        switch kind {
        case "confirm":    PhoneConfirmInput(prompt: prompt)
        case "choice":     PhoneChoiceInput(prompt: prompt)
        case "scale":      PhoneScaleInput(prompt: prompt)
        case "text":       PhoneTextInput(prompt: prompt, voice: false)
        case "voice_text": PhoneTextInput(prompt: prompt, voice: true)
        default:           Color.clear
        }
    }

    private var flowLabel: String {
        switch prompt.payload.flow ?? "question" {
        case "mood":     return "Mood"
        case "ema":      return "Check-in"
        case "reminder": return "Reminder"
        case "ambient":  return "Info"
        default:         return "Question"
        }
    }
}

// MARK: - Input controls

private struct PhoneConfirmInput: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        HStack(spacing: 12) {
            Button("Yes") { submit("yes") }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
            Button("No") { submit("no") }
                .buttonStyle(.bordered)
                .controlSize(.large)
        }
    }

    private func submit(_ value: String) {
        Task {
            await relay.submit(answer: PromptAnswer(answer: value, via: "choice",
                                                    elapsedS: 0, device: "iphone"),
                               for: prompt.id)
            dismiss()
        }
    }
}

private struct PhoneChoiceInput: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 10) {
            ForEach(prompt.payload.choices ?? [], id: \.self) { choice in
                Button(choice) {
                    Task {
                        await relay.submit(answer: PromptAnswer(answer: choice, via: "choice",
                                                                elapsedS: 0, device: "iphone"),
                                           for: prompt.id)
                        dismiss()
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
                .frame(maxWidth: .infinity)
            }
        }
    }
}

private struct PhoneScaleInput: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss
    @State private var value: Double = 4

    var body: some View {
        VStack(spacing: 16) {
            Text("\(Int(value))")
                .font(.system(size: 56, weight: .bold, design: .rounded))
            Slider(value: $value, in: 1...7, step: 1)
            HStack {
                Text("1").foregroundStyle(.secondary)
                Spacer()
                Text("7").foregroundStyle(.secondary)
            }
            .font(.caption)
            Button("Submit") {
                Task {
                    await relay.submit(answer: PromptAnswer(answer: "\(Int(value))", via: "scale",
                                                            elapsedS: 0, device: "iphone"),
                                       for: prompt.id)
                    dismiss()
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
    }
}

private struct PhoneTextInput: View {
    let prompt: PromptRecord
    let voice: Bool
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(spacing: 16) {
            TextField(voice ? "Speak or type…" : "Type your answer…", text: $text, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(4...)
                .focused($focused)
                .onAppear { focused = true }
            Button("Submit") {
                guard !text.isEmpty else { return }
                Task {
                    await relay.submit(answer: PromptAnswer(answer: text,
                                                            via: voice ? "voice_text" : "text",
                                                            elapsedS: 0, device: "iphone"),
                                       for: prompt.id)
                    dismiss()
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(text.isEmpty)
        }
    }
}
