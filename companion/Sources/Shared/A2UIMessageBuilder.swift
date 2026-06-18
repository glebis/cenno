import Foundation
import A2UISwiftUI   // re-exports A2UISwiftCore (A2uiMessage, RawComponent, ...)
import A2UISwiftCore

/// Converts cenno's JSONValue envelope into a2ui-swift's [A2uiMessage], after
/// applying CennoComponentRemap. Source is either the desugared simple prompt
/// or the raw `a2ui` passthrough field.
public enum A2UIMessageBuilder {
    public enum BuildError: Error { case passthroughNotAnArray }

    public static func messages(for payload: PromptPayload) throws -> [A2uiMessage] {
        if let a2ui = payload.a2ui {
            guard let arr = a2ui.arrayValue else { throw BuildError.passthroughNotAnArray }
            return try decode(CennoComponentRemap.apply(arr))
        }
        return try desugarMessages(for: payload)
    }

    /// The desugared-only envelope, ignoring any `a2ui` passthrough. Used as the
    /// fallback when a passthrough payload fails to build/process (parity with
    /// PromptPanel.tsx, which renders `desugar(prompt)` if the rich surface throws).
    public static func desugarMessages(for payload: PromptPayload) throws -> [A2uiMessage] {
        try decode(CennoComponentRemap.apply(A2UIDesugar.messages(for: payload)))
    }

    private static func decode(_ values: [JSONValue]) throws -> [A2uiMessage] {
        let data = try JSONEncoder().encode(values)
        return try JSONDecoder().decode([A2uiMessage].self, from: data)
    }
}
