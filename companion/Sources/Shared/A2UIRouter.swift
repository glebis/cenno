import Foundation

/// Decides which device should handle a given prompt.
public enum A2UIRouter {
    public enum Target { case watch, iphone }

    public static func target(for prompt: PromptRecord) -> Target {
        // Explicit hint takes priority
        switch prompt.deviceHint {
        case .watch:   return .watch
        case .iphone:  return .iphone
        case .any:     break
        }
        return watchCanHandle(prompt) ? .watch : .iphone
    }

    public static func watchCanHandle(_ prompt: PromptRecord) -> Bool {
        guard let kind = prompt.payload.input?.kind else {
            // no input spec → info panel (kind: none) — Watch can show it
            return true
        }
        switch kind {
        case "confirm":    return true
        case "scale":      return true
        case "text":       return true
        case "voice_text": return true
        case "none":       return true
        case "choice":
            // Watch list handles up to 4 options comfortably
            return (prompt.payload.choices?.count ?? 0) <= 4
        default:
            return false  // unknown kind or custom a2ui → iPhone
        }
    }
}
