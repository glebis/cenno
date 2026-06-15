import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// A companion device class the Mac can route a prompt to. Mirrors the Rust
/// `routing::DeviceClass` vocabulary (the wire format is `class:mode`).
public enum DeviceClass: String, Sendable, CaseIterable {
    case iphone, ipad, watch

    /// The class this running companion build reports as. Used by the ambient
    /// "second screen" loop to filter prompts meant for it.
    public static var current: DeviceClass {
        #if os(watchOS)
        return .watch
        #elseif canImport(UIKit)
        return UIDevice.current.userInterfaceIdiom == .pad ? .ipad : .iphone
        #else
        return .iphone
        #endif
    }
}

/// How a device class participates in routing for a given prompt. Mirrors the
/// Rust `routing::DeviceMode`.
public enum RouteMode: String, Sendable {
    /// Not a target for this prompt.
    case off
    /// Surface only after the prompt's grace delay has elapsed (overflow).
    case fallback
    /// Surface immediately (a live second screen).
    case mirror
}

/// Parsed `targets` field: per-class route modes. The wire format is a
/// comma-joined list of `class:mode` pairs, e.g. `"iphone:fallback,ipad:mirror"`.
/// Unknown classes/modes and malformed entries are skipped, never fatal.
public struct RoutingTargets: Sendable, Equatable {
    public let modes: [DeviceClass: RouteMode]

    public init(_ modes: [DeviceClass: RouteMode]) { self.modes = modes }

    public init(parsing raw: String) {
        var parsed: [DeviceClass: RouteMode] = [:]
        for part in raw.split(separator: ",") {
            let kv = part.split(separator: ":", maxSplits: 1)
            guard kv.count == 2,
                  let cls = DeviceClass(rawValue: kv[0].trimmingCharacters(in: .whitespaces)),
                  let mode = RouteMode(rawValue: kv[1].trimmingCharacters(in: .whitespaces))
            else { continue }
            parsed[cls] = mode
        }
        self.modes = parsed
    }

    public func mode(for cls: DeviceClass) -> RouteMode { modes[cls] ?? .off }

    /// Re-encode to the wire format (deterministic class order), for round-trips.
    public var encoded: String {
        DeviceClass.allCases
            .compactMap { cls in modes[cls].map { "\(cls.rawValue):\($0.rawValue)" } }
            .joined(separator: ",")
    }
}
