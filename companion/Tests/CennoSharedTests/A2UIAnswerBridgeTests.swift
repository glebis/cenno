import XCTest
@testable import CennoShared

final class A2UIAnswerBridgeTests: XCTestCase {
    func testIgnoresNonSubmitActions() {
        XCTAssertNil(A2UIAnswerBridge.answer(name: "focus", context: [:], elapsedS: 1, device: "iphone"))
    }
    func testTextAnswerViaText() {
        let a = A2UIAnswerBridge.answer(name: "submit",
            context: ["value": .string("hello"), "via": .string("text")],
            elapsedS: 2.5, device: "iphone")
        XCTAssertEqual(a?.answer, "hello"); XCTAssertEqual(a?.via, "text")
        XCTAssertEqual(a?.elapsedS, 2.5); XCTAssertEqual(a?.device, "iphone")
    }
    func testChoiceUnwrapsArray() {
        let a = A2UIAnswerBridge.answer(name: "submit-choice",
            context: ["value": .array([.string("Calm")]), "via": .string("choice")],
            elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.answer, "Calm"); XCTAssertEqual(a?.via, "choice")
    }
    func testScaleStringifiesNumber() {
        let a = A2UIAnswerBridge.answer(name: "submit-scale",
            context: ["value": .number(4), "via": .string("choice")], elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.answer, "4")
    }
    func testNullValueBecomesEmptyAck() {
        let a = A2UIAnswerBridge.answer(name: "submit",
            context: ["value": .null, "via": .string("text")], elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.answer, "")
    }
    func testUnknownViaDefaultsToText() {
        let a = A2UIAnswerBridge.answer(name: "submit-yes",
            context: ["value": .string("yes"), "via": .string("weird")], elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.via, "text")
    }
}
