import SwiftUI
import A2UISwiftCore
import A2UISwiftUI
import CennoShared

/// Renders a PromptRecord through the A2UI runtime and bridges submit actions
/// back to CloudKitRelay. Replaces the hand-rolled input switch.
struct A2UIPromptView: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss
    @State private var vm: SurfaceViewModel?
    @State private var buildError: String?
    private let shownAt = Date()

    var body: some View {
        Group {
            if let vm {
                // `catalog:` IS the custom component catalog (generic
                // `where Catalog: CustomComponentCatalog`); the core component
                // registry lives on the SurfaceViewModel via `init(catalog:)`.
                A2UISurfaceView(viewModel: vm, catalog: CennoComponentCatalog(), scrolls: true) { action in
                    handle(action)
                }
            } else if let buildError {
                ContentUnavailableView("Couldn't render", systemImage: "exclamationmark.triangle",
                                       description: Text(buildError))
            } else {
                ProgressView()
            }
        }
        .padding()
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Skip") { Task { await relay.markTimedOut(promptID: prompt.id); dismiss() } }
            }
        }
        .onAppear(perform: buildSurface)
    }

    private func buildSurface() {
        guard vm == nil else { return }
        // Try the primary path (passthrough if present, else desugar). If a
        // passthrough payload fails to build or process, fall back to the
        // desugared prompt — parity with PromptPanel.tsx's error boundary.
        if let model = makeSurface(try? A2UIMessageBuilder.messages(for: prompt.payload)) {
            vm = model; return
        }
        if prompt.payload.a2ui != nil,
           let model = makeSurface(try? A2UIMessageBuilder.desugarMessages(for: prompt.payload)) {
            vm = model; return
        }
        buildError = "This prompt couldn't be rendered."
    }

    /// Process messages into a SurfaceViewModel, or nil if they're absent,
    /// errored, or produced no component tree.
    private func makeSurface(_ messages: [A2uiMessage]?) -> SurfaceViewModel? {
        guard let messages else { return nil }
        let model = SurfaceViewModel(catalog: basicCatalog)
        guard model.processMessages(messages).isEmpty, model.componentTree != nil else { return nil }
        return model
    }

    private func handle(_ action: ResolvedAction) {
        // Convert a2ui-swift's [String: AnyCodable] context → [String: JSONValue].
        let context: [String: JSONValue] = (try? JSONDecoder().decode(
            [String: JSONValue].self, from: JSONEncoder().encode(action.context))) ?? [:]
        let elapsed = Date().timeIntervalSince(shownAt)
        guard let answer = A2UIAnswerBridge.answer(name: action.name, context: context,
                                                   elapsedS: elapsed, device: "iphone") else { return }
        Task { await relay.submit(answer: answer, for: prompt.id); dismiss() }
    }
}
