import SwiftUI
import CennoSharedWatch

/// Entry point on Watch — shows the next pending prompt, or an idle state.
struct PromptQueueView: View {
    @EnvironmentObject var relay: CloudKitRelay

    var body: some View {
        Group {
            // Only prompts routed to the Watch (watch:off never appears here);
            // unrouted/legacy records still show, as before.
            if let prompt = relay.pendingPrompts.first(where: { $0.isTargeted(at: .current) }) {
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
