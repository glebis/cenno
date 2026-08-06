import XCTest
@testable import CennoScreenContext

final class CennoScreenContextTests: XCTestCase {
    func testTruncateUsesCharacterBoundaryAndReportsClipping() {
        let result = truncateText("ab😀cd", maxChars: 3)
        XCTAssertEqual(result.text, "ab😀")
        XCTAssertTrue(result.truncated)
    }

    func testSemanticStatusNeedsUsefulContent() {
        XCTAssertEqual(semanticStatus(selected: nil, visible: nil, url: nil), .axUnavailable)
        XCTAssertEqual(semanticStatus(selected: "chosen", visible: nil, url: nil), .ok)
    }
}
