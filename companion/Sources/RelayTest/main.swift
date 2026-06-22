import CloudKit
import Foundation

// Phase-1 CloudKit relay writer — a dev-signed macOS app whose only job is to
// publish ONE pending Prompt record into the same private DB / zone the
// iPhone+Watch companion reads from, so we can prove delivery end-to-end
// independent of the Tauri app's (currently unentitled) writer.

let containerID = "iCloud.app.cenno"
let zoneID = CKRecordZone.ID(zoneName: "Prompts", ownerName: CKCurrentUserDefaultName)
let resultPath = "/tmp/cenno-writer-result.txt"

func writeResult(_ s: String) {
    let line = s + "\n"
    if let fh = FileHandle(forWritingAtPath: resultPath) {
        fh.seekToEndOfFile(); fh.write(Data(line.utf8)); fh.closeFile()
    } else {
        try? line.write(toFile: resultPath, atomically: true, encoding: .utf8)
    }
    FileHandle.standardError.write(Data(line.utf8))
}

func run() async {
    let db = CKContainer(identifier: containerID).privateCloudDatabase

    // 0. Account status — surface if not signed in / restricted.
    do {
        let status = try await CKContainer(identifier: containerID).accountStatus()
        writeResult("account status: \(status.rawValue) (3=available)")
    } catch {
        writeResult("ACCOUNT ERROR: \(error.localizedDescription)")
    }

    // 1. Ensure the custom zone exists (mirrors CloudKitRelay.ensureZone).
    //    modifyRecordZones does NOT throw on per-zone failure — inspect results.
    do {
        let (zoneResults, _) = try await db.modifyRecordZones(
            saving: [CKRecordZone(zoneID: zoneID)], deleting: [])
        for (zid, result) in zoneResults {
            if case .failure(let e) = result {
                writeResult("ZONE FAIL \(zid.zoneName): \(e)")
            } else {
                writeResult("zone ok: \(zid.zoneName)")
            }
        }
    } catch {
        writeResult("ZONE OP ERROR: \(error)")
    }

    // 2. Build a pending Prompt record matching PromptRecord's schema.
    let id = "test-\(Int(Date().timeIntervalSince1970))"
    let payload: [String: Any] = [
        "title": "Phone delivery test",
        "body_md": "Written by CennoRelayTest via CloudKit Development",
        "input": ["kind": "choice"],
        "choices": ["Got it", "Nope"],
        "flow": "question",
        "timeout_s": 3600,
    ]
    let payloadJSON = String(
        data: try! JSONSerialization.data(withJSONObject: payload), encoding: .utf8)!

    let recordID = CKRecord.ID(recordName: id, zoneID: zoneID)
    let record = CKRecord(recordType: "Prompt", recordID: recordID)
    record["prompt_id"] = id
    record["payload"] = payloadJSON
    record["state"] = "pending"
    record["device_hint"] = "any"
    record["created_at"] = Date()
    record["expires_at"] = Date().addingTimeInterval(3600)

    // 3. Save it — inspect PER-RECORD results (the op call won't throw on a
    //    per-record failure; that's what masked the earlier failure).
    do {
        let (saveResults, _) = try await db.modifyRecords(saving: [record], deleting: [])
        for (rid, result) in saveResults {
            switch result {
            case .success:
                writeResult("OK wrote prompt id=\(rid.recordName) zone=Prompts (Development)")
            case .failure(let e):
                writeResult("SAVE FAIL \(rid.recordName): \(e)")
            }
        }
    } catch {
        writeResult("SAVE OP ERROR: \(error)")
    }
}

func verify() async {
    let container = CKContainer(identifier: containerID)
    let db = container.privateCloudDatabase
    // Who are we? (stable per-account user record id)
    if let uid = try? await container.userRecordID() {
        writeResult("writer userRecordID: \(uid.recordName)")
    }
    // Run the EXACT reader query.
    let pred = NSPredicate(format: "state == %@ AND expires_at > %@", "pending", Date() as CVarArg)
    let query = CKQuery(recordType: "Prompt", predicate: pred)
    query.sortDescriptors = [NSSortDescriptor(key: "created_at", ascending: true)]
    do {
        let (results, _) = try await db.records(matching: query, inZoneWith: zoneID)
        writeResult("fetchPending-equivalent returned \(results.count) record(s)")
    } catch {
        writeResult("QUERY ERROR: \(error)")
    }
}

let sem = DispatchSemaphore(value: 0)
Task { await run(); await verify(); sem.signal() }
sem.wait()
exit(0)
