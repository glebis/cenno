import Foundation

/// Renames cenno-specific LEAF components to Cenno* typeNames so a2ui-swift
/// routes them to CennoComponentCatalog. Structural components (Row/Column/
/// Button/Image) stay basic — a2ui-swift's Button dispatches actions and its
/// containers render custom children natively.
public enum CennoComponentRemap {
    static let leaves: Set<String> = ["Text", "TextField", "ChoicePicker", "Slider", "Scale", "Dots", "DateTimeInput", "ScoreMatrix"]

    public static func apply(_ messages: [JSONValue]) -> [JSONValue] {
        messages.map { msg in
            guard case .object(var top) = msg,
                  let uc = top["updateComponents"], case .object(var ucObj) = uc,
                  let comps = ucObj["components"]?.arrayValue else { return msg }
            ucObj["components"] = .array(comps.map(renameComponent))
            top["updateComponents"] = .object(ucObj)
            return .object(top)
        }
    }

    private static func renameComponent(_ comp: JSONValue) -> JSONValue {
        guard case .object(var o) = comp,
              let name = o["component"]?.stringValue, leaves.contains(name) else { return comp }
        o["component"] = .string("Cenno" + name)
        return .object(o)
    }
}
