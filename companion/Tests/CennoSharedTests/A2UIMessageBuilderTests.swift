import XCTest
@testable import CennoShared
import A2UISwiftCore
import A2UISwiftUI

final class A2UIMessageBuilderTests: XCTestCase {
    func testDesugarPathDecodesIntoSurface() throws {
        let json = #"{"title":"Hi","body_md":"","input":{"kind":"text"}}"#
        let p = try JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        let messages = try A2UIMessageBuilder.messages(for: p)
        let vm = SurfaceViewModel(catalog: basicCatalog)
        let errors = vm.processMessages(messages)
        XCTAssertTrue(errors.isEmpty, "\(errors)")
        XCTAssertNotNil(vm.componentTree)
        // After remap the input is a custom CennoTextField node somewhere in the tree.
        XCTAssertTrue(containsType(vm.componentTree, "CennoText"))
    }

    private func containsType(_ node: ComponentNode?, _ name: String) -> Bool {
        guard let node else { return false }
        if case .custom(let n) = node.type, n == name { return true }
        return node.children.contains { containsType($0, name) }
    }
}
