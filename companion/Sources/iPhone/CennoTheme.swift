import SwiftUI

/// cenno design tokens (docs/design + tokens/tokens.json): one saturated flow
/// hue per flow, white ink on top, 40%-white hairlines. Mirrors the tauri
/// panel's themed surface so the iPhone renderer looks like the desktop app.
enum CennoTheme {
    // Flow surface hues (tokens: color.flow.*)
    static func surface(for flow: String?) -> Color {
        switch flow {
        case "mood":     return Color(hex: 0xFF6250) // coral
        case "ema":      return Color(hex: 0x0E7C6B) // teal
        case "reminder": return Color(hex: 0x4A5568) // slate
        case "ambient":  return Color(hex: 0x14171A) // ink
        default:         return Color(hex: 0x1E4FD8) // cobalt — questions/choices
        }
    }

    // On-flow palette (tokens: color.text / color.line)
    static let ink = Color.white                       // primary text
    static let inkDim = Color.white.opacity(0.6)        // 60% white
    static let line = Color.white.opacity(0.4)          // 40% white — hairlines/outlines

    // Type scale (tokens: type.*) — SF system, semibold questions.
    static let questionM = Font.system(size: 22, weight: .semibold)
    static let questionL = Font.system(size: 34, weight: .semibold)
    static let body = Font.system(size: 17)
    static let caption = Font.system(size: 13)

    // Spacing (tokens: space.*) and radii (tokens: radius.*)
    static let space1: CGFloat = 8
    static let space2: CGFloat = 16
    static let space3: CGFloat = 24
}

/// The active flow surface color, injected by A2UIPromptView so leaf views can
/// paint "selected" states (e.g. a filled scale numeral flips to the surface hue).
private struct CennoSurfaceKey: EnvironmentKey {
    static let defaultValue: Color = CennoTheme.surface(for: nil)
}
extension EnvironmentValues {
    var cennoSurface: Color {
        get { self[CennoSurfaceKey.self] }
        set { self[CennoSurfaceKey.self] = newValue }
    }
}

extension Color {
    /// 0xRRGGBB convenience initializer.
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: 1)
    }
}

/// A simple wrapping flow layout (iOS 16+ Layout) for outline chips.
struct FlowLayout: Layout {
    var spacing: CGFloat = CennoTheme.space1

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout Void) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        var rowWidth: CGFloat = 0, rowHeight: CGFloat = 0
        var totalHeight: CGFloat = 0, totalWidth: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if rowWidth + size.width > maxWidth, rowWidth > 0 {
                totalHeight += rowHeight + spacing
                totalWidth = max(totalWidth, rowWidth - spacing)
                rowWidth = 0; rowHeight = 0
            }
            rowWidth += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
        totalHeight += rowHeight
        totalWidth = max(totalWidth, rowWidth - spacing)
        return CGSize(width: min(totalWidth, maxWidth), height: totalHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout Void) {
        var x = bounds.minX, y = bounds.minY, rowHeight: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x + size.width > bounds.maxX, x > bounds.minX {
                x = bounds.minX; y += rowHeight + spacing; rowHeight = 0
            }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}
