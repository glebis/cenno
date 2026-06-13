import SwiftUI
import CennoShared

struct PhonePromptQueueView: View {
    @EnvironmentObject var relay: CloudKitRelay

    var body: some View {
        NavigationStack {
            Group {
                if relay.pendingPrompts.isEmpty {
                    ContentUnavailableView(
                        "Nothing pending",
                        systemImage: "checkmark.circle",
                        description: Text("Prompts from your agents will appear here.")
                    )
                } else {
                    List(relay.pendingPrompts) { prompt in
                        NavigationLink(value: prompt) {
                            PromptRowView(prompt: prompt)
                        }
                    }
                }
            }
            .navigationTitle("cenno")
            .navigationDestination(for: PromptRecord.self) { prompt in
                PhonePromptDetailView(prompt: prompt)
            }
            .refreshable { await relay.fetchPending() }
        }
    }
}

private struct PromptRowView: View {
    let prompt: PromptRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(prompt.payload.title)
                .font(.headline)
                .lineLimit(2)
            HStack {
                if let kind = prompt.payload.input?.kind {
                    Label(kind, systemImage: kindIcon(kind))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text(prompt.expiresAt, style: .relative)
                    .font(.caption)
                    .foregroundStyle(prompt.isExpired ? .red : .secondary)
            }
        }
        .padding(.vertical, 2)
    }

    private func kindIcon(_ kind: String) -> String {
        switch kind {
        case "confirm":    return "checkmark.circle"
        case "choice":     return "list.bullet"
        case "scale":      return "slider.horizontal.3"
        case "text":       return "keyboard"
        case "voice_text": return "mic"
        default:           return "info.circle"
        }
    }
}

