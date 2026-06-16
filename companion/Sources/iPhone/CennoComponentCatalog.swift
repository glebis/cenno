import SwiftUI
import A2UISwiftCore
import A2UISwiftUI

/// Renders cenno's leaf components (remapped to Cenno* typeNames) natively, in
/// the cenno design language (white ink on a saturated flow surface, outline
/// pills, bare numerals) — matching the tauri panel. Structural components
/// (Row/Column/Button) stay basic; A2UIPromptView restyles the basic buttons
/// via A2UIStyle.buttonStyles.
///
/// Output is pinned to `AnyView`: the protocol declares `associatedtype Output:
/// View`, and an opaque `some View` from a switch can fail to infer one concrete
/// `Output`. The `default` branch never fires — the remap only ever produces
/// the six Cenno* typeNames.
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
        case "CennoDateTimeInput": return AnyView(CennoDateTimeInputView(node: node, surface: surface))
        case "CennoScoreMatrix":  return AnyView(CennoUnsupportedView(control: "ScoreMatrix"))
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

// MARK: - Text (Markdown + variant sizing). Inherits foreground color so primary
// button labels can flip to the surface hue (cenno's `.cenno-button .cenno-text`).

private struct CennoTextView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable { var text: String?; var variant: String? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        if p.variant == "caption" {
            Text(p.text ?? "").font(CennoTheme.caption).foregroundStyle(CennoTheme.inkDim)
                .textCase(.uppercase).tracking(1)
                // Keep full intrinsic height: never let a short (landscape)
                // viewport compress + truncate the text — overflow scrolls instead.
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            Text(markdown(p.text ?? "")).font(font(for: p.variant))
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
    private func markdown(_ s: String) -> AttributedString {
        (try? AttributedString(markdown: s,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
    }
    private func font(for variant: String?) -> Font {
        switch variant {
        case "h1": return CennoTheme.questionL
        case "h2", "h3", "h4", "h5": return CennoTheme.questionM
        default: return CennoTheme.body
        }
    }
}

// MARK: - TextField (underline only + voice flag + submitAction)

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
        // Bottom-border underline only (cenno-field). No Send button here — the
        // desugar emits a separate `send` Button; we wire keyboard-return submit.
        VStack(spacing: 4) {
            TextField("", text: dc.stringBinding(for: p.value), axis: .vertical)
                .font(CennoTheme.body).foregroundStyle(CennoTheme.ink).tint(CennoTheme.ink)
                .lineLimit(1...6).focused($focused)
                .onAppear { focused = true }
                .onSubmit { fire(p.submitAction, node: node, surface: surface, handler: handler) }
            Rectangle().fill(focused ? CennoTheme.ink : CennoTheme.line).frame(height: 1)
        }
    }
}

// MARK: - ChoicePicker (outline pills, wrapping row + words variant + selectAction)

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
        let words = (p.variant == "words")
        FlowLayout(spacing: words ? CennoTheme.space3 : CennoTheme.space1) {
            ForEach(p.options ?? [], id: \.value) { opt in
                Button {
                    try? dc.set(bindingPath(p.value), value: .array([.string(opt.value)]))
                    fire(p.selectAction, node: node, surface: surface, handler: handler)
                } label: {
                    if words {
                        Text(opt.label).font(CennoTheme.questionM).foregroundStyle(CennoTheme.ink)
                            .padding(.vertical, CennoTheme.space2)
                    } else {
                        Text(opt.label).font(CennoTheme.body).foregroundStyle(CennoTheme.ink)
                            .padding(.vertical, CennoTheme.space1).padding(.horizontal, CennoTheme.space3)
                            .frame(minHeight: 44)
                            .overlay(Capsule().stroke(CennoTheme.line, lineWidth: 1))
                    }
                }
                .buttonStyle(.plain)
            }
        }
        .frame(maxWidth: .infinity, alignment: words ? .center : .leading)
    }
    private func bindingPath(_ v: DynamicStringList?) -> String {
        if case .dataBinding(let path)? = v { return path }; return "/choice"
    }
}

// MARK: - Slider (hairline track, white thumb, caption labels)

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
        VStack(spacing: CennoTheme.space1) {
            Slider(value: dc.doubleBinding(for: p.value ?? .literal(lo), fallback: lo), in: lo...hi) { editing in
                if !editing { fire(p.selectAction, node: node, surface: surface, handler: handler) }
            }
            .tint(CennoTheme.ink)
            labels(p.minLabel, p.maxLabel)
        }
    }
    @ViewBuilder private func labels(_ lo: String?, _ hi: String?) -> some View {
        if lo != nil || hi != nil {
            HStack { Text(lo ?? "").font(CennoTheme.caption); Spacer(); Text(hi ?? "").font(CennoTheme.caption) }
                .foregroundStyle(CennoTheme.inkDim)
        }
    }
}

// MARK: - Scale (outline-circle numerals; selected = filled, numeral flips to surface)

private struct CennoScaleView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    @Environment(\.cennoSurface) private var surfaceColor
    private struct Props: Codable { var min: Double?; var max: Double?; var value: DynamicNumber?
                                    var minLabel: String?; var maxLabel: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = DataContext(surface: surface, path: node.dataContextPath)
        let lo = Int(p.min ?? 1), hi = Int(p.max ?? 7)
        let selected = dc.resolve(p.value ?? .literal(.nan)).map { Int($0) }
        VStack(alignment: .leading, spacing: CennoTheme.space1) {
            HStack(spacing: CennoTheme.space1) {
                ForEach(lo...hi, id: \.self) { n in
                    let isSel = (selected == n)
                    Button {
                        try? dc.set(bindingPath(p.value), value: .number(Double(n)))
                        fire(p.selectAction, node: node, surface: surface, handler: handler)
                    } label: {
                        Text("\(n)").font(CennoTheme.body)
                            .foregroundStyle(isSel ? surfaceColor : CennoTheme.ink)
                            .frame(width: 44, height: 44)
                            .background(Circle().fill(isSel ? CennoTheme.ink : Color.clear))
                            .overlay(Circle().stroke(isSel ? CennoTheme.ink : CennoTheme.line, lineWidth: 1))
                    }
                    .buttonStyle(.plain).frame(maxWidth: .infinity)
                }
            }
            labels(p.minLabel, p.maxLabel)
        }
    }
    @ViewBuilder private func labels(_ lo: String?, _ hi: String?) -> some View {
        if lo != nil || hi != nil {
            HStack { Text(lo ?? "").font(CennoTheme.caption); Spacer(); Text(hi ?? "").font(CennoTheme.caption) }
                .foregroundStyle(CennoTheme.inkDim)
        }
    }
    private func bindingPath(_ v: DynamicNumber?) -> String {
        if case .dataBinding(let path)? = v { return path }; return "/scale"
    }
}

// MARK: - Dots (6px dots, active full, inactive 40%)

private struct CennoDotsView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable { var step: Double?; var total: Double? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let total = Int(p.total ?? 1), step = Int(p.step ?? 1)
        HStack(spacing: CennoTheme.space1) {
            ForEach(1...max(total, 1), id: \.self) { i in
                Circle().fill(CennoTheme.ink).opacity(i == step ? 1 : 0.4)
                    .frame(width: 6, height: 6)
            }
        }
        .frame(maxWidth: .infinity)
    }
}

// MARK: - DateTimeInput (native compact picker, cenno-tinted: white ink, dark
// scheme so the pill reads on the saturated flow surface — vs. a2ui-swift's
// standard view, which renders black-on-cobalt). Updates the bound value; an
// explicit submit is left to a paired Button (no premature answer on scroll).

private struct CennoDateTimeInputView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable {
        var label: String?; var value: DynamicString?
        var enableDate: Bool?; var enableTime: Bool?
    }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = DataContext(surface: surface, path: node.dataContextPath)
        let date = p.enableDate ?? true, time = p.enableTime ?? false
        let comps: DatePickerComponents = (date && time) ? [.date, .hourAndMinute]
            : time ? [.hourAndMinute] : [.date]
        let str = dc.stringBinding(for: p.value)
        let selection = Binding<Date>(
            get: { Self.parse(str.wrappedValue, date: date, time: time) ?? Date() },
            set: { str.wrappedValue = Self.format($0, date: date, time: time) }
        )
        HStack {
            if let label = p.label, !label.isEmpty {
                Text(label).font(CennoTheme.body).foregroundStyle(CennoTheme.ink)
            }
            Spacer()
            DatePicker("", selection: selection, displayedComponents: comps)
                .labelsHidden()
                .datePickerStyle(.compact)
                .tint(CennoTheme.ink)
                .environment(\.colorScheme, .dark)   // light numerals on the pill
        }
        .frame(maxWidth: .infinity)
    }
    private static func fmt(_ date: Bool, _ time: Bool) -> DateFormatter {
        let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = (date && time) ? "yyyy-MM-dd'T'HH:mm" : time ? "HH:mm" : "yyyy-MM-dd"
        return f
    }
    private static func parse(_ s: String, date: Bool, time: Bool) -> Date? { fmt(date, time).date(from: s) }
    private static func format(_ d: Date, date: Bool, time: Bool) -> String { fmt(date, time).string(from: d) }
}

// MARK: - Unsupported control fallback. Some desktop controls (e.g. ScoreMatrix,
// a composite multi-value scorer) aren't yet implemented on the companion.
// Render a visible notice rather than a silent blank panel so the user knows to
// answer on the Mac instead of staring at an empty surface.

private struct CennoUnsupportedView: View {
    let control: String
    var body: some View {
        HStack(spacing: CennoTheme.space1) {
            Image(systemName: "desktopcomputer")
            Text("“\(control)” isn't supported here yet — answer on your Mac.")
                .font(CennoTheme.caption)
        }
        .foregroundStyle(CennoTheme.inkDim)
        .padding(CennoTheme.space2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(CennoTheme.line, lineWidth: 1))
    }
}
