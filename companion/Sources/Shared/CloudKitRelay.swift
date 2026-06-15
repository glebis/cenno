import CloudKit
import Foundation

/// The single source of truth for reading and writing Prompt records.
/// Both the iPhone and Watch targets share this via the Shared framework.
@MainActor
public final class CloudKitRelay: ObservableObject {
    static let containerID = "iCloud.app.cenno"
    static let zoneID = CKRecordZone.ID(zoneName: "Prompts", ownerName: CKCurrentUserDefaultName)

    @Published public var pendingPrompts: [PromptRecord] = []
    @Published public var error: String?

    private let container: CKContainer
    private let db: CKDatabase
    private var subscriptionID = "pending-prompts"

    public init() {
        container = CKContainer(identifier: Self.containerID)
        db = container.privateCloudDatabase
    }

    // MARK: - Bootstrap

    public func start() async {
        await ensureZone()
        await ensureSubscription()
        await fetchPending()
    }

    // MARK: - Fetch

    public func fetchPending() async {
        let pred = NSPredicate(format: "state == %@ AND expires_at > %@",
                               "pending", Date() as CVarArg)
        let query = CKQuery(recordType: PromptRecord.recordType, predicate: pred)
        query.sortDescriptors = [NSSortDescriptor(key: "created_at", ascending: true)]

        do {
            let (results, _) = try await db.records(matching: query, inZoneWith: Self.zoneID)
            pendingPrompts = results.compactMap { _, result in
                guard case .success(let record) = result else { return nil }
                return PromptRecord(record: record)
            }
        } catch {
            self.error = error.localizedDescription
        }
    }

    // MARK: - Answer

    public func submit(answer: PromptAnswer, for promptID: String) async {
        // Find the CKRecord by querying for the prompt_id
        let pred = NSPredicate(format: "prompt_id == %@", promptID)
        let query = CKQuery(recordType: PromptRecord.recordType, predicate: pred)

        do {
            let (results, _) = try await db.records(matching: query, inZoneWith: Self.zoneID)
            guard let (_, result) = results.first,
                  case .success(let record) = result else { return }

            let encoder = JSONEncoder()
            if let data = try? encoder.encode(answer),
               let json = String(data: data, encoding: .utf8) {
                record["answer"] = json
            }
            record["state"] = "answered"
            _ = try await db.modifyRecords(saving: [record], deleting: [])

            pendingPrompts.removeAll { $0.id == promptID }
        } catch {
            self.error = error.localizedDescription
        }
    }

    public func markTimedOut(promptID: String) async {
        let pred = NSPredicate(format: "prompt_id == %@", promptID)
        let query = CKQuery(recordType: PromptRecord.recordType, predicate: pred)

        do {
            let (results, _) = try await db.records(matching: query, inZoneWith: Self.zoneID)
            guard let (_, result) = results.first,
                  case .success(let record) = result else { return }
            record["state"] = "timed_out"
            _ = try await db.modifyRecords(saving: [record], deleting: [])
            pendingPrompts.removeAll { $0.id == promptID }
        } catch {
            self.error = error.localizedDescription
        }
    }

    // MARK: - Push notification → refresh

    public func handleRemoteNotification() async {
        await fetchPending()
    }

    // MARK: - Private setup

    private func ensureZone() async {
        let zone = CKRecordZone(zoneID: Self.zoneID)
        _ = try? await db.modifyRecordZones(saving: [zone], deleting: [])
    }

    private func ensureSubscription() async {
        let sub = CKQuerySubscription(
            recordType: PromptRecord.recordType,
            // Match ALL Prompt records, not just `state == pending`. A state
            // change to answered/timed_out moves a record OUT of a
            // `pending`-only predicate, and CloudKit does NOT fire an update
            // notification for a record that no longer matches (QA1917). With a
            // true predicate, both creation and the answered/timed_out update
            // wake us; `fetchPending()` then drops the non-pending rows.
            predicate: NSPredicate(value: true),
            subscriptionID: subscriptionID,
            options: [.firesOnRecordCreation, .firesOnRecordUpdate]
        )
        let info = CKSubscription.NotificationInfo()
        info.shouldSendContentAvailable = true  // silent push — app wakes, fetches
        sub.notificationInfo = info
        _ = try? await db.modifySubscriptions(saving: [sub], deleting: [])
    }
}
