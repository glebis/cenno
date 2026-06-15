import SwiftUI
import CennoShared

struct PhonePromptQueueView: View {
    @EnvironmentObject var relay: CloudKitRelay
    @ObservedObject var secondScreen: SecondScreenSettings
    @State private var showingSettings = false

    var body: some View {
        NavigationStack {
            // Always a List so pull-to-refresh works even when empty (an
            // overlaid ContentUnavailableView isn't scrollable on its own).
            List(relay.pendingPrompts) { prompt in
                NavigationLink(value: prompt) {
                    PromptRowView(prompt: prompt)
                }
            }
            .overlay {
                if relay.pendingPrompts.isEmpty {
                    ContentUnavailableView(
                        "Nothing pending",
                        systemImage: "checkmark.circle",
                        description: Text("Prompts from your agents will appear here.\nPull down to refresh.")
                    )
                    .allowsHitTesting(false)   // let the pull gesture reach the List
                }
            }
            .navigationTitle("cenno")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showingSettings = true } label: {
                        Image(systemName: "rectangle.on.rectangle.angled")
                    }
                    .accessibilityLabel("Second screen settings")
                }
            }
            .navigationDestination(for: PromptRecord.self) { prompt in
                PhonePromptDetailView(prompt: prompt)
            }
            .refreshable { await relay.fetchPending() }
            .sheet(isPresented: $showingSettings) {
                SecondScreenSettingsView(settings: secondScreen)
            }
        }
    }
}

private struct PromptRowView: View {
    let prompt: PromptRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(Self.plainTitle(prompt.payload.title))
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

    /// Strip Markdown markers from the title for the list row (the detail view
    /// renders full Markdown; rows want a clean single line).
    static func plainTitle(_ md: String) -> String {
        (try? AttributedString(markdown: md,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            .map { String($0.characters) } ?? md
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

