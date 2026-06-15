import XCTest
@testable import CennoShared

final class PromptRoutingTests: XCTestCase {
    // MARK: - RoutingTargets parsing

    func testParsesClassModePairs() {
        let t = RoutingTargets(parsing: "iphone:fallback,ipad:mirror")
        XCTAssertEqual(t.mode(for: .iphone), .fallback)
        XCTAssertEqual(t.mode(for: .ipad), .mirror)
        XCTAssertEqual(t.mode(for: .watch), .off) // absent → off
    }

    func testParsingSkipsMalformedAndUnknown() {
        let t = RoutingTargets(parsing: "iphone:fallback,bogus:mirror,ipad:nope,,watch")
        XCTAssertEqual(t.mode(for: .iphone), .fallback)
        XCTAssertEqual(t.mode(for: .ipad), .off)   // "nope" not a mode
        XCTAssertEqual(t.mode(for: .watch), .off)  // no ":mode"
    }

    func testEmptyStringYieldsAllOff() {
        let t = RoutingTargets(parsing: "")
        for c in DeviceClass.allCases { XCTAssertEqual(t.mode(for: c), .off) }
    }

    func testEncodeRoundTripsDeterministically() {
        let t = RoutingTargets([.ipad: .mirror, .iphone: .fallback])
        XCTAssertEqual(t.encoded, "iphone:fallback,ipad:mirror") // class order, not insertion
        XCTAssertEqual(RoutingTargets(parsing: t.encoded), t)
    }

    // MARK: - shouldSurface

    private func record(
        targets: RoutingTargets,
        graceS: Int = 20,
        state: PromptRecord.State = .pending,
        createdAt: Date,
        expiresAt: Date
    ) -> PromptRecord {
        PromptRecord(
            id: "p1",
            payload: PromptPayload(title: "t", bodyMd: nil, input: nil, choices: nil,
                                   flow: nil, timeoutS: nil, urgency: nil, progress: nil, a2ui: nil),
            deviceHint: .any, state: state, answer: nil,
            createdAt: createdAt, expiresAt: expiresAt,
            targets: targets, graceS: graceS
        )
    }

    func testMirrorSurfacesImmediately() {
        let now = Date()
        let r = record(targets: RoutingTargets([.ipad: .mirror]),
                       createdAt: now, expiresAt: now.addingTimeInterval(600))
        XCTAssertTrue(r.shouldSurface(on: .ipad, now: now))
    }

    func testFallbackWaitsForGrace() {
        let now = Date()
        let r = record(targets: RoutingTargets([.iphone: .fallback]), graceS: 20,
                       createdAt: now, expiresAt: now.addingTimeInterval(600))
        // Before grace: hidden. After grace: shown.
        XCTAssertFalse(r.shouldSurface(on: .iphone, now: now.addingTimeInterval(10)))
        XCTAssertTrue(r.shouldSurface(on: .iphone, now: now.addingTimeInterval(21)))
    }

    func testOffClassNeverSurfaces() {
        let now = Date()
        let r = record(targets: RoutingTargets([.iphone: .fallback]),
                       createdAt: now, expiresAt: now.addingTimeInterval(600))
        XCTAssertFalse(r.shouldSurface(on: .ipad, now: now.addingTimeInterval(999)))
    }

    func testAnsweredNeverSurfaces() {
        let now = Date()
        let r = record(targets: RoutingTargets([.ipad: .mirror]), state: .answered,
                       createdAt: now, expiresAt: now.addingTimeInterval(600))
        XCTAssertFalse(r.shouldSurface(on: .ipad, now: now))
    }

    func testExpiredNeverSurfaces() {
        let now = Date()
        let r = record(targets: RoutingTargets([.ipad: .mirror]),
                       createdAt: now.addingTimeInterval(-1000),
                       expiresAt: now.addingTimeInterval(-1))
        XCTAssertFalse(r.shouldSurface(on: .ipad, now: now))
    }
}
