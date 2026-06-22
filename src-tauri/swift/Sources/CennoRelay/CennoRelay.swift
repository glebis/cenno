// CennoRelay — Mac-side CloudKit writer for the cenno companion relay.
//
// When an agent calls ask_user, cenno writes a Prompt CKRecord so the user's
// Watch/iPhone companion can display it. When the Mac user answers (or it times
// out), the record state is updated so the companion stays in sync.
//
// FFI:
//   cenno_relay_write_prompt(prompt_id, payload_json, targets, grace_secs, timeout_secs)
//   cenno_relay_update_state(prompt_id, state_cstr, answer_json)   -- nullable answer_json
//
// `targets` is the resolved routing string ("iphone:fallback,ipad:mirror");
// `grace_secs` is the fallback delay companion devices apply before surfacing.
//
// Both functions are fire-and-forget: they spawn a Swift Task and return
// immediately. CloudKit errors are printed to stderr but are not fatal.

import CloudKit
import Foundation
import Security
import os

// MARK: - Constants

private let containerID = "iCloud.app.cenno"
private let zoneName    = "Prompts"
private let recordType  = "Prompt"

// MARK: - CloudKit availability gate

// Constructing CKContainer(identifier:) for a container the app isn't entitled
// to raises an Objective-C exception that Swift `do/catch` cannot catch — it
// traps and crashes the whole process. Ad-hoc/unsigned local builds have no
// `com.apple.developer.icloud-container-identifiers` entitlement, so we must
// check for it BEFORE ever touching CKContainer. Result is cached: entitlements
// don't change over a process's lifetime.
private let cloudKitAvailable: Bool = {
    guard let task = SecTaskCreateFromSelf(nil) else { return false }
    let key = "com.apple.developer.icloud-container-identifiers" as CFString
    guard let value = SecTaskCopyValueForEntitlement(task, key, nil) else { return false }
    if let ids = value as? [String] { return ids.contains(containerID) }
    return false
}()

private var _zoneID: CKRecordZone.ID {
    CKRecordZone.ID(zoneName: zoneName, ownerName: CKCurrentUserDefaultName)
}

// MARK: - Zone bootstrap (once per process)

// Guarded by an actor so no lock is needed in async context.
private actor ZoneState {
    var ensured = false
    func markEnsured() { ensured = true }
}
private let zoneState = ZoneState()

private func ensureZone(db: CKDatabase) async {
    if await zoneState.ensured { return }
    let zone = CKRecordZone(zoneID: _zoneID)
    do {
        _ = try await db.modifyRecordZones(saving: [zone], deleting: [])
        await zoneState.markEnsured()
    } catch {
        // Non-fatal: zone may already exist; write will still succeed.
        print("[CennoRelay] ensureZone warning: \(error)")
    }
}

// MARK: - Helpers

private func makeDB() -> CKDatabase {
    CKContainer(identifier: containerID).privateCloudDatabase
}

private func string(_ ptr: UnsafePointer<CChar>?) -> String? {
    guard let ptr else { return nil }
    return String(cString: ptr)
}

// MARK: - Write prompt

/// Creates a new Prompt CKRecord. Called once when ask_user is registered.
@_cdecl("cenno_relay_write_prompt")
public func cenno_relay_write_prompt(
    prompt_id:    UnsafePointer<CChar>?,
    payload_json: UnsafePointer<CChar>?,
    targets:      UnsafePointer<CChar>?,
    grace_secs:   Int64,
    timeout_secs: Int64
) {
    guard let pid = string(prompt_id), let payload = string(payload_json) else {
        print("[CennoRelay] write_prompt: missing required args")
        return
    }
    // Skip entirely when the app isn't entitled for CloudKit — otherwise
    // constructing the container traps (uncatchable) and crashes the app.
    guard cloudKitAvailable else {
        print("[CennoRelay] write_prompt(\(pid)) skipped: no CloudKit entitlement")
        return
    }
    let targetsStr = string(targets) ?? ""
    let now        = Date()
    let expires    = now.addingTimeInterval(TimeInterval(timeout_secs) + 30)

    Task {
        let db = makeDB()
        await ensureZone(db: db)

        let recordID = CKRecord.ID(recordName: pid, zoneID: _zoneID)
        let record   = CKRecord(recordType: recordType, recordID: recordID)
        record["prompt_id"]  = pid
        record["payload"]    = payload
        record["targets"]    = targetsStr
        record["grace_s"]    = grace_secs
        record["state"]      = "pending"
        record["created_at"] = now
        record["expires_at"] = expires

        do {
            _ = try await db.modifyRecords(saving: [record], deleting: [])
        } catch {
            print("[CennoRelay] write_prompt(\(pid)) failed: \(error)")
        }
    }
}

// MARK: - Update state

/// Updates an existing Prompt record's state (and optionally answer).
/// - state: "answered" | "timed_out"
/// - answer_json: nullable; only set when state == "answered"
@_cdecl("cenno_relay_update_state")
public func cenno_relay_update_state(
    prompt_id:   UnsafePointer<CChar>?,
    state_cstr:  UnsafePointer<CChar>?,
    answer_json: UnsafePointer<CChar>?
) {
    guard let pid = string(prompt_id), let state = string(state_cstr) else {
        print("[CennoRelay] update_state: missing required args")
        return
    }
    guard cloudKitAvailable else {
        print("[CennoRelay] update_state(\(pid)) skipped: no CloudKit entitlement")
        return
    }
    let answer = string(answer_json)

    Task {
        let db       = makeDB()
        let recordID = CKRecord.ID(recordName: pid, zoneID: _zoneID)

        do {
            let record      = try await db.record(for: recordID)
            record["state"] = state
            if let answer   { record["answer"] = answer }
            _ = try await db.modifyRecords(saving: [record], deleting: [])
        } catch {
            print("[CennoRelay] update_state(\(pid), \(state)) failed: \(error)")
        }
    }
}
