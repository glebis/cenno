import XCTest
@testable import CennoShared
import A2UISwiftCore
import A2UISwiftUI

final class SmokeTest: XCTestCase {
    func testA2UISwiftLinks() {
        // Proves the vendored package compiles + links into the test target.
        let vm = SurfaceViewModel(catalog: basicCatalog)
        XCTAssertNil(vm.componentTree)
    }
}
