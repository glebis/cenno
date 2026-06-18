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
    @State private var showReply = false
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
        #if DEBUG
        .onAppear { if DemoHarness.replyOpen { showReply = true } }
        #endif
        // Reply-in-words is available on EVERY prompt — so a display-only or
        // unsupported control is never a dead end, and the user can always add
        // their own answer/context for the agent.
        .sheet(isPresented: $showReply) {
            ReplySheet(surfaceColor: surfaceColor, onSend: submitReply)
        }
    }

    /// Plain top bar: a text-only Skip, plus a reply affordance on the right.
    private var header: some View {
        HStack {
            Button("Skip") { skip() }
                .buttonStyle(.plain)
                .foregroundStyle(CennoTheme.inkDim)
            Spacer()
            Button { showReply = true } label: {
                Image(systemName: "bubble.and.pencil")
                    .font(.system(size: 18, weight: .regular))
            }
            .buttonStyle(.plain)
            .foregroundStyle(CennoTheme.ink)
            .accessibilityLabel("Reply in your own words")
        }
        .font(CennoTheme.body)
        .padding(.horizontal, CennoTheme.space3)
        .padding(.top, CennoTheme.space1)
        .padding(.bottom, CennoTheme.space5)   // breathing room before the title
    }

    @ViewBuilder private var content: some View {
        if let vm {
            // Pin-or-scroll: the surface renders with its OWN scrolling off
            // (`scrolls:false` → raw component tree), wrapped here in a
            // ScrollView whose content is floored at the viewport height.
            //   • Short content  → floor = viewport, the pinned envelope's
            //     weighted column fills it and pushes the action group to the
            //     bottom (no scroll, identical to before).
            //   • Tall content (long body, or a short landscape viewport) →
            //     content grows past the floor and the ScrollView scrolls,
            //     instead of clipping the last lines.
            // `catalog:` IS the custom component catalog; the core registry
            // lives on the SurfaceViewModel via `init(catalog:)`. scrolls:false
            // also drops the built-in .padding(), so re-apply cenno's gutters.
            GeometryReader { geo in
                ScrollView {
                    A2UISurfaceView(viewModel: vm, catalog: CennoComponentCatalog(), scrolls: false) { action in
                        handle(action)
                    }
                    .frame(maxWidth: .infinity,
                           minHeight: max(0, geo.size.height - CennoTheme.space3),
                           alignment: .top)
                    .padding(.horizontal, CennoTheme.space3)
                    .padding(.bottom, CennoTheme.space3)
                }
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
        // Rich `a2ui` passthrough surfaces own their own layout — render as-is.
        if prompt.payload.a2ui != nil,
           let model = makeSurface(try? A2UIMessageBuilder.messages(for: prompt.payload)) {
            vm = model; return
        }
        // Desugared prompts: pin the action group to the bottom (phone layout;
        // desktop does the equivalent in catalog.css). Also the passthrough
        // fallback (PromptPanel.tsx parity).
        if let model = makeSurface(pinnedDesugarMessages(for: prompt.payload)) {
            vm = model; return
        }
        buildError = "This prompt couldn't be rendered."
    }

    /// Desugar → cenno remap → pin-to-bottom (phone layout) → typed messages.
    private func pinnedDesugarMessages(for payload: PromptPayload) -> [A2uiMessage]? {
        let env = PromptLayout.pinActionsToBottom(
            CennoComponentRemap.apply(A2UIDesugar.messages(for: payload)))
        guard let data = try? JSONEncoder().encode(env) else { return nil }
        return try? JSONDecoder().decode([A2uiMessage].self, from: data)
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

    /// Free-text/dictated reply from the header bubble — submitted as the answer
    /// (via `text`), through the same path as a control answer.
    private func submitReply(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let answer = PromptAnswer(answer: trimmed, via: "text",
                                  elapsedS: Date().timeIntervalSince(shownAt), device: "iphone")
        if let onAnswer { onAnswer(answer); return }
        Task { await relay.submit(answer: answer, for: prompt.id); dismiss() }
    }

    private func skip() {
        if onAnswer != nil { dismiss(); return }
        Task { await relay.markTimedOut(promptID: prompt.id); dismiss() }
    }
}

/// Compact reply composer: type or dictate (the system keyboard's mic key — no
/// speech framework or mic permission needed) a free-text answer for the agent.
private struct ReplySheet: View {
    let surfaceColor: Color
    let onSend: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @FocusState private var focused: Bool

    private var canSend: Bool {
        !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        NavigationStack {
            ZStack {
                surfaceColor.ignoresSafeArea()
                VStack(alignment: .leading, spacing: CennoTheme.space2) {
                    Text("Reply in your own words")
                        .font(CennoTheme.questionM)
                        .foregroundStyle(CennoTheme.ink)
                    VStack(spacing: 4) {
                        TextField("Type, or tap the mic to dictate…", text: $text, axis: .vertical)
                            .font(CennoTheme.body)
                            .foregroundStyle(CennoTheme.ink)
                            .tint(CennoTheme.ink)
                            .lineLimit(3...8)
                            .focused($focused)
                        Rectangle().fill(focused ? CennoTheme.ink : CennoTheme.line).frame(height: 1)
                    }
                    Spacer()
                }
                .padding(CennoTheme.space3)
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }.foregroundStyle(CennoTheme.inkDim)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Send") { onSend(text); dismiss() }
                        .foregroundStyle(CennoTheme.ink)
                        .disabled(!canSend)
                }
            }
            .toolbarBackground(surfaceColor, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .onAppear { focused = true }
        }
        .presentationDetents([.medium])
        .tint(CennoTheme.ink)
    }
}
