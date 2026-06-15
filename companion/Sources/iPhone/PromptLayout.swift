import Foundation
import CennoShared

/// Phone-only layout pass over the desugared A2UI envelope: pins the prompt's
/// action group (choices / text field+send / confirm buttons, plus any
/// progress dots) to the bottom of the screen while the title+body stay at the
/// top. This is the iOS analog of the desktop panel's `margin-top:auto`
/// trailing row (catalog.css) — the desktop window hugs its content, but the
/// phone is a fixed full screen, so without this the content strands itself at
/// the top over a tall empty surface.
///
/// Mechanism (mirrors the desktop's flex-fill + auto-margin):
///   1. give the `col` Column `weight` so it fills the viewport height, and
///   2. insert a weighted spacer Column right after the body text — a weighted
///      child renders as `.frame(maxHeight:.infinity)` (WeightModifier), so it
///      absorbs the slack and pushes everything below it to the bottom.
///
/// Scoped to the desugar path only; rich `a2ui` passthrough surfaces own their
/// own layout and are left untouched (parity with how the desktop scopes its
/// CSS to the desugared component ids).
enum PromptLayout {
    static let spacerID = "cenno-action-spacer"

    static func pinActionsToBottom(_ messages: [JSONValue]) -> [JSONValue] {
        messages.map { message in
            guard case .object(var msg) = message,
                  case .object(var uc)? = msg["updateComponents"],
                  case .array(var comps)? = uc["components"]
            else { return message }

            comps = comps.map { comp in
                guard case .object(var c) = comp,
                      c["id"]?.stringValue == "col",
                      case .array(var children)? = c["children"]
                else { return comp }
                // Insert after the body (or after the title when there's no
                // body) so the text block stays grouped at the top.
                let anchor = children.firstIndex { $0.stringValue == "body" }
                    ?? children.firstIndex { $0.stringValue == "title" }
                let at = anchor.map { $0 + 1 } ?? 0
                children.insert(.string(spacerID), at: at)
                c["children"] = .array(children)
                c["weight"] = .number(1)   // fill the viewport so the spacer has room
                return .object(c)
            }

            comps.append(.object([
                "id": .string(spacerID),
                "component": .string("Column"),
                "weight": .number(1),
                "children": .array([]),
            ]))
            uc["components"] = .array(comps)
            msg["updateComponents"] = .object(uc)
            return .object(msg)
        }
    }
}
