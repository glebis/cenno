import AppKit
import ApplicationServices
import Foundation

enum SemanticStatus: String, Codable {
    case ok
    case permissionDenied = "permission_denied"
    case axUnavailable = "ax_unavailable"
}

struct TruncatedText {
    let text: String
    let truncated: Bool
}

func truncateText(_ text: String, maxChars: Int) -> TruncatedText {
    let limit = max(0, maxChars)
    guard text.count > limit else { return TruncatedText(text: text, truncated: false) }
    return TruncatedText(text: String(text.prefix(limit)), truncated: true)
}

func semanticStatus(selected: String?, visible: String?, url: String?) -> SemanticStatus {
    [selected, visible, url].contains { value in
        value.map { !$0.isEmpty } ?? false
    } ? .ok : .axUnavailable
}

private struct RawScreenContext: Codable {
    let status: SemanticStatus
    let appName: String?
    let bundleId: String?
    let windowTitle: String?
    let url: String?
    let host: String?
    let focusedRole: String?
    let selectedText: String?
    let visibleText: String?
    let truncated: Bool
}

public typealias ScreenContextCallback = @convention(c) (
    UnsafeMutableRawPointer?, UnsafePointer<CChar>?
) -> Void

private func stringAttribute(_ element: AXUIElement?, _ attribute: CFString) -> String? {
    guard let element else { return nil }
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute, &value) == .success else { return nil }
    guard let string = value as? String, !string.isEmpty else { return nil }
    return string
}

private func elementAttribute(_ element: AXUIElement, _ attribute: CFString) -> AXUIElement? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute, &value) == .success,
          let value,
          CFGetTypeID(value) == AXUIElementGetTypeID()
    else { return nil }
    return unsafeBitCast(value, to: AXUIElement.self)
}

private func visibleRangeText(_ element: AXUIElement?) -> String? {
    guard let element else { return nil }
    var range: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXVisibleCharacterRangeAttribute as CFString, &range) == .success,
          let range
    else { return nil }
    var value: CFTypeRef?
    guard AXUIElementCopyParameterizedAttributeValue(
        element,
        kAXStringForRangeParameterizedAttribute as CFString,
        range,
        &value
    ) == .success else { return nil }
    guard let string = value as? String, !string.isEmpty else { return nil }
    return string
}

private func focusedRole(_ element: AXUIElement?) -> String? {
    guard let role = stringAttribute(element, kAXRoleAttribute as CFString) else { return nil }
    guard let subrole = stringAttribute(element, kAXSubroleAttribute as CFString) else { return role }
    return "\(role)/\(subrole)"
}

private func directURL(role: String?, value: String?) -> (url: String?, host: String?) {
    guard role?.hasPrefix(kAXTextFieldRole as String) == true,
          let value,
          let parsed = URL(string: value),
          parsed.scheme == "http" || parsed.scheme == "https",
          let host = parsed.host
    else { return (nil, nil) }
    return (parsed.absoluteString, host.lowercased())
}

private func readContext(includeVisibleText: Bool, maxChars: Int) -> RawScreenContext {
    let options = [
        kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true
    ] as CFDictionary
    guard AXIsProcessTrustedWithOptions(options) else {
        return RawScreenContext(
            status: .permissionDenied,
            appName: nil, bundleId: nil, windowTitle: nil, url: nil, host: nil,
            focusedRole: nil, selectedText: nil, visibleText: nil, truncated: false
        )
    }

    let system = AXUIElementCreateSystemWide()
    let app = elementAttribute(system, kAXFocusedApplicationAttribute as CFString)
    let window = app.flatMap { elementAttribute($0, kAXFocusedWindowAttribute as CFString) }
    let focused = app.flatMap { elementAttribute($0, kAXFocusedUIElementAttribute as CFString) }

    var pid: pid_t = 0
    if let app { AXUIElementGetPid(app, &pid) }
    let running = pid == 0 ? nil : NSRunningApplication(processIdentifier: pid)
    let role = focusedRole(focused)
    let directValue = stringAttribute(focused, kAXValueAttribute as CFString)
    let direct = directURL(role: role, value: directValue)

    var truncated = false
    let selectedRaw = stringAttribute(focused, kAXSelectedTextAttribute as CFString)
    let selectedResult = selectedRaw.map { truncateText($0, maxChars: maxChars) }
    truncated = selectedResult?.truncated ?? false
    let remaining = max(0, maxChars - (selectedResult?.text.count ?? 0))

    var visibleResult: TruncatedText?
    if includeVisibleText && remaining > 0 {
        let visibleRaw = directValue ?? visibleRangeText(focused)
        visibleResult = visibleRaw.map { truncateText($0, maxChars: remaining) }
        truncated = truncated || (visibleResult?.truncated ?? false)
    } else if includeVisibleText && (directValue != nil || visibleRangeText(focused) != nil) {
        truncated = true
    }

    let selected = selectedResult?.text
    let visible = visibleResult?.text
    return RawScreenContext(
        status: semanticStatus(selected: selected, visible: visible, url: direct.url),
        appName: running?.localizedName,
        bundleId: running?.bundleIdentifier,
        windowTitle: stringAttribute(window, kAXTitleAttribute as CFString),
        url: direct.url,
        host: direct.host,
        focusedRole: role,
        selectedText: selected,
        visibleText: visible,
        truncated: truncated
    )
}

private func encode(_ context: RawScreenContext) -> String? {
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    guard let data = try? encoder.encode(context) else { return nil }
    return String(data: data, encoding: .utf8)
}

@_cdecl("cenno_screen_context_read")
public func cenno_screen_context_read(
    _ includeVisibleText: Int32,
    _ maxChars: UInt32,
    _ ctx: UnsafeMutableRawPointer?,
    _ callback: ScreenContextCallback
) -> Int32 {
    let context = readContext(
        includeVisibleText: includeVisibleText != 0,
        maxChars: max(1, min(Int(maxChars), 8000))
    )
    guard let json = encode(context) else { return 1 }
    json.withCString { callback(ctx, $0) }
    return 0
}
