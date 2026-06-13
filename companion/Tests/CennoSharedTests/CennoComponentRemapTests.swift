import XCTest
@testable import CennoShared

final class CennoComponentRemapTests: XCTestCase {
    func testRenamesLeafComponentsOnly() {
        let msgs: [JSONValue] = [.object(["updateComponents": .object(["components": .array([
            .object(["id": .string("title"), "component": .string("Text")]),
            .object(["id": .string("col"), "component": .string("Column")]),
            .object(["id": .string("yes"), "component": .string("Button")]),
            .object(["id": .string("choices"), "component": .string("ChoicePicker")]),
            .object(["id": .string("scale"), "component": .string("Scale")]),
            .object(["id": .string("dots"), "component": .string("Dots")]),
        ])])])]
        let out = CennoComponentRemap.apply(msgs)
        let comps = out[0]["updateComponents"]?["components"]?.arrayValue ?? []
        func comp(_ id: String) -> String? {
            comps.first { $0["id"]?.stringValue == id }?["component"]?.stringValue }
        XCTAssertEqual(comp("title"), "CennoText")
        XCTAssertEqual(comp("choices"), "CennoChoicePicker")
        XCTAssertEqual(comp("scale"), "CennoScale")
        XCTAssertEqual(comp("dots"), "CennoDots")
        XCTAssertEqual(comp("col"), "Column")    // structural: unchanged
        XCTAssertEqual(comp("yes"), "Button")    // structural: unchanged
    }

    func testPassesNonComponentMessagesThrough() {
        let msgs: [JSONValue] = [.object(["createSurface": .object(["surfaceId": .string("main")])])]
        XCTAssertEqual(CennoComponentRemap.apply(msgs), msgs)
    }
}
