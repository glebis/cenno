import XCTest

/// Drives the real app in the simulator (coordinate-free, by accessibility
/// label) to prove the A2UI tap → submit pipeline works end to end. The DEBUG
/// `-cennoDemo` harness shows a "✓ answered: …" overlay on submit.
final class CennoRenderUITests: XCTestCase {
    func testChoiceChipSubmits() {
        let app = XCUIApplication()
        app.launchArguments = ["-cennoDemo", "choice"]
        app.launch()
        let calm = app.buttons["Calm"]
        XCTAssertTrue(calm.waitForExistence(timeout: 8), "Calm chip should render")
        calm.tap()
        let answered = app.staticTexts.containing(
            NSPredicate(format: "label CONTAINS[c] 'answered'")).firstMatch
        XCTAssertTrue(answered.waitForExistence(timeout: 5), "submit should surface the answer")
        XCTAssertTrue(answered.label.contains("Calm"), "answer should be the tapped value")
    }

    func testConfirmYesSubmits() {
        let app = XCUIApplication()
        app.launchArguments = ["-cennoDemo", "confirm"]
        app.launch()
        let yes = app.buttons["Yes"]
        XCTAssertTrue(yes.waitForExistence(timeout: 8), "Yes button should render")
        yes.tap()
        let answered = app.staticTexts.containing(
            NSPredicate(format: "label CONTAINS[c] 'answered'")).firstMatch
        XCTAssertTrue(answered.waitForExistence(timeout: 5), "confirm submit should surface the answer")
    }

    func testScaleNumberSubmits() {
        let app = XCUIApplication()
        app.launchArguments = ["-cennoDemo", "scale"]
        app.launch()
        let five = app.buttons["5"]
        XCTAssertTrue(five.waitForExistence(timeout: 8), "scale numeral should render")
        five.tap()
        let answered = app.staticTexts.containing(
            NSPredicate(format: "label CONTAINS[c] 'answered'")).firstMatch
        XCTAssertTrue(answered.waitForExistence(timeout: 5), "scale submit should surface the answer")
        XCTAssertTrue(answered.label.contains("5"), "answer should be the tapped numeral")
    }
}
