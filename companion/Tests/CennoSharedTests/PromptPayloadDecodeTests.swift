import XCTest
@testable import CennoShared

final class PromptPayloadDecodeTests: XCTestCase {
    func testDecodesA2uiProgressUrgency() throws {
        let json = """
        {"title":"T","body_md":"**b**","input":{"kind":"scale"},
         "urgency":"high","progress":{"step":2,"total":5},
         "a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}}]}
        """
        let p = try JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        XCTAssertEqual(p.title, "T")
        XCTAssertEqual(p.urgency, "high")
        XCTAssertEqual(p.progress?.step, 2)
        XCTAssertEqual(p.progress?.total, 5)
        XCTAssertEqual(p.a2ui?[0]?["createSurface"]?["surfaceId"], .string("main"))
    }

    func testMissingOptionalFieldsDecodeNil() throws {
        let json = #"{"title":"T","body_md":"","input":{"kind":"text"}}"#
        let p = try JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        XCTAssertNil(p.a2ui)
        XCTAssertNil(p.progress)
        XCTAssertNil(p.urgency)
    }
}
