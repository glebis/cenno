import CloudKit
import Foundation

/// Mirrors the CKRecord schema. All mutation goes through CloudKitRelay.
public struct PromptRecord: Identifiable, Sendable {
    public enum State: String, Sendable { case pending, answered, timedOut = "timed_out" }
    public enum DeviceHint: String, Sendable { case watch, iphone, any }

    public let id: String            // prompt_id — matches cenno's local history
    public let payload: PromptPayload
    public let deviceHint: DeviceHint
    public var state: State
    public var answer: PromptAnswer?
    public let createdAt: Date
    public let expiresAt: Date

    public var isExpired: Bool { Date() >= expiresAt }

    /// Memberwise initializer (the struct's only other init is the failable
    /// `init?(record:)`). Used by SwiftUI previews and tests.
    public init(id: String, payload: PromptPayload, deviceHint: DeviceHint, state: State,
                answer: PromptAnswer?, createdAt: Date, expiresAt: Date) {
        self.id = id
        self.payload = payload
        self.deviceHint = deviceHint
        self.state = state
        self.answer = answer
        self.createdAt = createdAt
        self.expiresAt = expiresAt
    }
}

public struct PromptPayload: Codable, Sendable {
    public let title: String
    public let bodyMd: String?
    public let input: InputSpec?
    public let choices: [String]?
    public let flow: String?
    public let timeoutS: Int?
    public let urgency: String?
    public let progress: PromptProgress?
    public let a2ui: JSONValue?

    enum CodingKeys: String, CodingKey {
        case title, bodyMd = "body_md", input, choices, flow
        case timeoutS = "timeout_s", urgency, progress, a2ui
    }
}

/// Named `PromptProgress` (not `Progress`) to avoid clashing with Foundation.Progress.
public struct PromptProgress: Codable, Sendable {
    public let step: Int
    public let total: Int

    public init(step: Int, total: Int) {
        self.step = step
        self.total = total
    }
}

public struct InputSpec: Codable, Sendable {
    public let kind: String  // text | voice_text | choice | scale | confirm | none
}

public struct PromptAnswer: Codable, Sendable {
    public let answer: String
    public let via: String
    public let elapsedS: Double
    public let device: String        // "watch" | "iphone"

    public init(answer: String, via: String, elapsedS: Double, device: String) {
        self.answer = answer
        self.via = via
        self.elapsedS = elapsedS
        self.device = device
    }

    enum CodingKeys: String, CodingKey {
        case answer, via, elapsedS = "elapsed_s", device
    }
}

// MARK: - Hashable (identity based on prompt id)

extension PromptRecord: Hashable {
    public static func == (lhs: PromptRecord, rhs: PromptRecord) -> Bool { lhs.id == rhs.id }
    public func hash(into hasher: inout Hasher) { hasher.combine(id) }
}

// MARK: - CKRecord ↔ PromptRecord

extension PromptRecord {
    public static let recordType = "Prompt"

    public init?(record: CKRecord) {
        guard
            let id = record["prompt_id"] as? String,
            let payloadJSON = record["payload"] as? String,
            let payloadData = payloadJSON.data(using: .utf8),
            let payload = try? JSONDecoder().decode(PromptPayload.self, from: payloadData),
            let stateRaw = record["state"] as? String,
            let state = State(rawValue: stateRaw),
            let createdAt = record["created_at"] as? Date,
            let expiresAt = record["expires_at"] as? Date
        else { return nil }

        self.id = id
        self.payload = payload
        self.deviceHint = DeviceHint(rawValue: record["device_hint"] as? String ?? "any") ?? .any
        self.state = state
        self.createdAt = createdAt
        self.expiresAt = expiresAt

        if let answerJSON = record["answer"] as? String,
           let answerData = answerJSON.data(using: .utf8),
           let ans = try? JSONDecoder().decode(PromptAnswer.self, from: answerData) {
            self.answer = ans
        }
    }

    public func applyTo(_ record: CKRecord) {
        let encoder = JSONEncoder()
        if let data = try? encoder.encode(payload),
           let json = String(data: data, encoding: .utf8) {
            record["payload"] = json
        }
        record["prompt_id"] = id
        record["device_hint"] = deviceHint.rawValue
        record["state"] = state.rawValue
        record["created_at"] = createdAt
        record["expires_at"] = expiresAt
        if let answer, let data = try? encoder.encode(answer),
           let json = String(data: data, encoding: .utf8) {
            record["answer"] = json
        }
    }
}
