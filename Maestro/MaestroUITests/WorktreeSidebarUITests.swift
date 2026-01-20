import XCTest

final class WorktreeSidebarUITests: XCTestCase {
    func testEmptyStateCopy() {
        let app = XCUIApplication()
        app.launchArguments = ["-ui-test-mode", "empty-workspaces"]
        app.launch()

        XCTAssertTrue(app.staticTexts["worktree-empty-title"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["worktree-empty-description"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["worktree-empty-create"].waitForExistence(timeout: 5))
    }
}
