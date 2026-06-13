import XCTest
@testable import CennoShared

final class JSONValueTests: XCTestCase {
    func testRoundTripsNestedObject() throws {
        let json = #"{"a":1,"b":["x",true,null],"c":{"d":2.5}}"#
        let value = try JSONDecoder().decode(JSONValue.self, from: Data(json.utf8))
        XCTAssertEqual(value["a"], .number(1))
        XCTAssertEqual(value["b"]?[1], .bool(true))
        XCTAssertEqual(value["b"]?[2], .null)
        XCTAssertEqual(value["c"]?["d"], .number(2.5))
    }

    func testEncodesBackToStableJSON() throws {
        let value: JSONValue = .object(["k": .string("v")])
        let data = try JSONEncoder().encode(value)
        let back = try JSONDecoder().decode(JSONValue.self, from: data)
        XCTAssertEqual(value, back)
    }

    func testSubscriptsReturnNilOffType() {
        let value: JSONValue = .string("hi")
        XCTAssertNil(value["a"])
        XCTAssertNil(value[0])
    }
}
