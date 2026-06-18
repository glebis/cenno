import Foundation

/// Pure port of the submit-action → answer logic in src/PromptPanel.tsx.
public enum A2UIAnswerBridge {
    public static func answer(name: String, context: [String: JSONValue],
                              elapsedS: Double, device: String) -> PromptAnswer? {
        guard name.hasPrefix("submit") else { return nil }
        let viaRaw = context["via"]?.stringValue
        let via = (viaRaw == "choice" || viaRaw == "voice_text") ? viaRaw! : "text"

        // /choice arrives as a 1-element array; unwrap. null/missing → "" (ack).
        let rawValue: JSONValue?
        if let arr = context["value"]?.arrayValue { rawValue = arr.first } else { rawValue = context["value"] }
        let answer = stringify(rawValue)
        return PromptAnswer(answer: answer, via: via, elapsedS: elapsedS, device: device)
    }

    private static func stringify(_ v: JSONValue?) -> String {
        switch v {
        case .some(.string(let s)): return s
        case .some(.number(let n)): return n == n.rounded() ? String(Int(n)) : String(n)
        case .some(.bool(let b)): return b ? "true" : "false"
        case .none, .some(.null): return ""
        // A nested object/array value should not occur for /draft, /choice[0],
        // or /scale, but mirror JS String() best-effort rather than dropping it.
        case .some(let other):
            if let data = try? JSONEncoder().encode(other), let s = String(data: data, encoding: .utf8) {
                return s
            }
            return ""
        }
    }
}
