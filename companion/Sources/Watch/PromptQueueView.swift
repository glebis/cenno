import SwiftUI
import CennoShared

/// Entry point on Watch — shows the next pending prompt, or an idle state.
struct PromptQueueView: View {
    @EnvironmentObject var relay: CloudKitRelay

    var body: some View {
        Group {
            if let prompt = relay.pendingPrompts.first {
                PromptView(prompt: prompt)
            } else {
                IdleView()
            }
        }
        .animation(.easeInOut, value: relay.pendingPrompts.map(\.id))
    }
}

private struct IdleView: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "checkmark.circle")
                .font(.title2)
                .foregroundStyle(.secondary)
            Text("Nothing pending")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
    }
}
