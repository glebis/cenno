import SwiftUI
import A2UISwiftCore
import A2UISwiftUI
import CennoShared

/// Renders a PromptRecord through the A2UI runtime in the cenno design language
/// (white ink on the flow's saturated surface) and bridges submit actions back
/// to CloudKitRelay. Replaces the hand-rolled input switch.
struct A2UIPromptView: View {
    let prompt: PromptRecord
    /// When set (DEBUG demo), receives the answer instead of CloudKit — so taps
    /// are visibly verifiable without a live relay.
    var onAnswer: ((PromptAnswer) -> Void)? = nil

    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss
    @State private var vm: SurfaceViewModel?
    @State private var buildError: String?
    private let shownAt = Date()

    private var surfaceColor: Color { CennoTheme.surface(for: prompt.payload.flow) }

    var body: some View {
        ZStack {
            surfaceColor.ignoresSafeArea()
            VStack(spacing: 0) {
                header
                content
            }
        }
        .environment(\.cennoSurface, surfaceColor)
        .foregroundStyle(CennoTheme.ink)
        .tint(CennoTheme.ink)
        // cenno draws its own chrome — hide the glassy iOS toolbar.
        .toolbar(.hidden, for: .navigationBar)
        .onAppear(perform: buildSurface)
    }

    /// Plain top bar: a text-only Skip, no capsule background.
    private var header: some View {
        HStack {
            Button("Skip") { skip() }
                .buttonStyle(.plain)
                .foregroundStyle(CennoTheme.inkDim)
            Spacer()
        }
        .font(CennoTheme.body)
        .padding(.horizontal, CennoTheme.space3)
        .padding(.vertical, CennoTheme.space1)
    }

    @ViewBuilder private var content: some View {
        if let vm {
            // `catalog:` IS the custom component catalog; the core registry lives
            // on the SurfaceViewModel via `init(catalog:)`.
            A2UISurfaceView(viewModel: vm, catalog: CennoComponentCatalog(), scrolls: true) { action in
                handle(action)
            }
        } else if let buildError {
            ContentUnavailableView("Couldn't render", systemImage: "exclamationmark.triangle",
                                   description: Text(buildError))
        } else {
            ProgressView().tint(CennoTheme.ink)
        }
    }

    private func buildSurface() {
        guard vm == nil else { return }
        // Primary path (passthrough if present, else desugar); on passthrough
        // failure fall back to the desugared prompt (PromptPanel.tsx parity).
        if let model = makeSurface(try? A2UIMessageBuilder.messages(for: prompt.payload)) {
            vm = model; return
        }
        if prompt.payload.a2ui != nil,
           let model = makeSurface(try? A2UIMessageBuilder.desugarMessages(for: prompt.payload)) {
            vm = model; return
        }
        buildError = "This prompt couldn't be rendered."
    }

    private func makeSurface(_ messages: [A2uiMessage]?) -> SurfaceViewModel? {
        guard let messages else { return nil }
        let model = SurfaceViewModel(catalog: basicCatalog)
        guard model.processMessages(messages).isEmpty, model.componentTree != nil else { return nil }
        model.a2uiStyle = cennoStyle()
        return model
    }

    /// Restyle the basic Row/Column/Button via A2UIStyle so the desugared
    /// confirm Yes/No and quiet Send match cenno's pill/text treatments.
    private func cennoStyle() -> A2UIStyle {
        A2UIStyle(
            primaryColor: CennoTheme.ink,
            buttonStyles: [
                "primary":    .init(foregroundColor: surfaceColor, backgroundColor: CennoTheme.ink,
                                    cornerRadius: 22, horizontalPadding: CennoTheme.space3, verticalPadding: 12),
                "borderless": .init(foregroundColor: CennoTheme.inkDim, backgroundColor: .clear,
                                    cornerRadius: 0, horizontalPadding: CennoTheme.space1, verticalPadding: 12),
                "quiet":      .init(foregroundColor: CennoTheme.ink, backgroundColor: .clear,
                                    cornerRadius: 0, horizontalPadding: 0, verticalPadding: 12),
            ]
        )
    }

    private func handle(_ action: ResolvedAction) {
        let context: [String: JSONValue] = (try? JSONDecoder().decode(
            [String: JSONValue].self, from: JSONEncoder().encode(action.context))) ?? [:]
        let elapsed = Date().timeIntervalSince(shownAt)
        guard let answer = A2UIAnswerBridge.answer(name: action.name, context: context,
                                                   elapsedS: elapsed, device: "iphone") else { return }
        if let onAnswer { onAnswer(answer); return }
        Task { await relay.submit(answer: answer, for: prompt.id); dismiss() }
    }

    private func skip() {
        if onAnswer != nil { dismiss(); return }
        Task { await relay.markTimedOut(promptID: prompt.id); dismiss() }
    }
}
