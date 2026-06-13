import SwiftUI
import A2UISwiftCore
import A2UISwiftUI

/// Renders cenno's leaf components (remapped to Cenno* typeNames) natively.
///
/// Output is pinned to `AnyView`: the protocol declares `associatedtype Output:
/// View`, and an opaque `some View` from a switch can fail to infer one concrete
/// `Output`. The `default` branch never fires in practice — the remap only ever
/// produces the six Cenno* typeNames.
struct CennoComponentCatalog: CustomComponentCatalog {
    typealias Output = AnyView
    @MainActor
    func build(typeName: String, node: ComponentNode, surface: SurfaceModel) -> AnyView {
        switch typeName {
        case "CennoText":         return AnyView(CennoTextView(node: node, surface: surface))
        case "CennoTextField":    return AnyView(CennoTextFieldView(node: node, surface: surface))
        case "CennoChoicePicker": return AnyView(CennoChoicePickerView(node: node, surface: surface))
        case "CennoSlider":       return AnyView(CennoSliderView(node: node, surface: surface))
        case "CennoScale":        return AnyView(CennoScaleView(node: node, surface: surface))
        case "CennoDots":         return AnyView(CennoDotsView(node: node, surface: surface))
        default:                  return AnyView(EmptyView())
        }
    }
}

// MARK: - Action firing (mirrors the built-in A2UIButton)

@MainActor
private func fire(_ action: Action?, node: ComponentNode, surface: SurfaceModel,
                  handler: (@Sendable (ResolvedAction) -> Void)?) {
    guard case .event(let name, let ctx)? = action else { return }
    let dc = DataContext(surface: surface, path: node.dataContextPath)
    var resolved: [String: AnyCodable] = [:]
    ctx?.forEach { resolved[$0.key] = dc.resolveDynamicValue($0.value) ?? .null }
    surface.dispatchAction(name: name, sourceComponentId: node.id, context: resolved)
    handler?(ResolvedAction(name: name, sourceComponentId: node.id, context: resolved))
}

// MARK: - Text (Markdown + variant sizing)

private struct CennoTextView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable { var text: String?; var variant: String? }
    var body: some View {
        let _ = node.instance   // register @Observable tracking
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        // desugar emits `text` as a literal string; no data-binding resolve needed.
        Text(markdown(p.text ?? "")).font(font(for: p.variant))
            .frame(maxWidth: .infinity, alignment: .leading)
    }
    private func markdown(_ s: String) -> AttributedString {
        (try? AttributedString(markdown: s,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
    }
    private func font(for variant: String?) -> Font {
        switch variant {
        case "h1": return .system(size: 34, weight: .bold)
        case "h2", "h3", "h4", "h5": return .title2.bold()
        case "caption": return .caption
        default: return .body
        }
    }
}

// MARK: - TextField (voice flag + submitAction)

private struct CennoTextFieldView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    @FocusState private var focused: Bool
    private struct Props: Codable { var label: String?; var value: DynamicString?
                                    var voice: Bool?; var submitAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = DataContext(surface: surface, path: node.dataContextPath)
        // No Send button here: the desugar emits a separate `send` Button sibling
        // (rendered by the basic catalog) that fires the same submitAction. We
        // only wire keyboard-return submit, so raw a2ui TextFields without a
        // paired button still submit. `voice: true` uses the system keyboard
        // dictation mic for MVP (on-device push-to-talk is tauri-only today).
        TextField(p.label ?? "Your reply", text: dc.stringBinding(for: p.value), axis: .vertical)
            .textFieldStyle(.roundedBorder).lineLimit(3...).focused($focused)
            .onAppear { focused = true }
            .onSubmit { fire(p.submitAction, node: node, surface: surface, handler: handler) }
    }
}

// MARK: - ChoicePicker (chips + words variant + selectAction)

private struct CennoChoicePickerView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    private struct Option: Codable { var label: String; var value: String }
    private struct Props: Codable { var options: [Option]?; var value: DynamicStringList?
                                    var variant: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = DataContext(surface: surface, path: node.dataContextPath)
        VStack(spacing: 10) {
            ForEach(p.options ?? [], id: \.value) { opt in
                Button(opt.label) {
                    try? dc.set(bindingPath(p.value), value: .array([.string(opt.value)]))
                    fire(p.selectAction, node: node, surface: surface, handler: handler)
                }
                .font(p.variant == "words" ? .title2 : .body)
                .buttonStyle(.bordered).frame(maxWidth: .infinity)
            }
        }
    }
    private func bindingPath(_ v: DynamicStringList?) -> String {
        if case .dataBinding(let path)? = v { return path }; return "/choice"
    }
}

// MARK: - Slider (min/max labels + selectAction on commit)

private struct CennoSliderView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    private struct Props: Codable { var min: Double?; var max: Double?; var value: DynamicNumber?
                                    var minLabel: String?; var maxLabel: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = DataContext(surface: surface, path: node.dataContextPath)
        let lo = p.min ?? 0, hi = p.max ?? 10
        VStack {
            Slider(value: dc.doubleBinding(for: p.value ?? .literal(lo), fallback: lo), in: lo...hi) { editing in
                if !editing { fire(p.selectAction, node: node, surface: surface, handler: handler) }
            }
            HStack { Text(p.minLabel ?? "").font(.caption); Spacer(); Text(p.maxLabel ?? "").font(.caption) }
        }
    }
}

// MARK: - Scale (discrete numeral row + selectAction)

private struct CennoScaleView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    private struct Props: Codable { var min: Double?; var max: Double?; var value: DynamicNumber?
                                    var minLabel: String?; var maxLabel: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = DataContext(surface: surface, path: node.dataContextPath)
        let lo = Int(p.min ?? 1), hi = Int(p.max ?? 7)
        VStack(spacing: 8) {
            HStack { ForEach(lo...hi, id: \.self) { n in
                Button("\(n)") {
                    try? dc.set(bindingPath(p.value), value: .number(Double(n)))
                    fire(p.selectAction, node: node, surface: surface, handler: handler)
                }.buttonStyle(.bordered).frame(maxWidth: .infinity)
            } }
            HStack { Text(p.minLabel ?? "").font(.caption); Spacer(); Text(p.maxLabel ?? "").font(.caption) }
        }
    }
    private func bindingPath(_ v: DynamicNumber?) -> String {
        if case .dataBinding(let path)? = v { return path }; return "/scale"
    }
}

// MARK: - Dots (step pagination)

private struct CennoDotsView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable { var step: Double?; var total: Double? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let total = Int(p.total ?? 1), step = Int(p.step ?? 1)
        HStack(spacing: 6) {
            ForEach(1...max(total, 1), id: \.self) { i in
                Circle().fill(i == step ? Color.primary : Color.secondary.opacity(0.3))
                    .frame(width: 7, height: 7)
            }
        }
    }
}
