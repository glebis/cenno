import CloudKit
import Foundation

/// Mirrors the CKRecord schema. All mutation goes through CloudKitRelay.
struct PromptRecord: Identifiable, Sendable {
    enum State: String { case pending, answered, timedOut = "timed_out" }
    enum DeviceHint: String { case watch, iphone, any }

    let id: String            // prompt_id — matches cenno's local history
    let payload: PromptPayload
    let deviceHint: DeviceHint
    var state: State
    var answer: PromptAnswer?
    let createdAt: Date
    let expiresAt: Date

    var isExpired: Bool { Date() >= expiresAt }
}

struct PromptPayload: Codable, Sendable {
    let title: String
    let bodyMd: String?
    let input: InputSpec?
    let choices: [String]?
    let flow: String?
    let timeoutS: Int?

    enum CodingKeys: String, CodingKey {
        case title, bodyMd = "body_md", input, choices, flow, timeoutS = "timeout_s"
    }
}

struct InputSpec: Codable, Sendable {
    let kind: String  // text | voice_text | choice | scale | confirm | none
}

struct PromptAnswer: Codable, Sendable {
    let answer: String
    let via: String
    let elapsedS: Double
    let device: String        // "watch" | "iphone"

    enum CodingKeys: String, CodingKey {
        case answer, via, elapsedS = "elapsed_s", device
    }
}

// MARK: - CKRecord ↔ PromptRecord

extension PromptRecord {
    static let recordType = "Prompt"

    init?(record: CKRecord) {
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

    func applyTo(_ record: CKRecord) {
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
