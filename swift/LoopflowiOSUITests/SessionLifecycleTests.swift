import XCTest

/// Operate the shipped view/lifecycle wiring with the explicit offline peer.
/// Network/owner acceptance lives in MachineConnectionIntegrationTests.
@MainActor
final class SessionLifecycleTests: XCTestCase {
    func testReturningToTheAppObservesTheSameSessionBeforeKeyboardTakeover() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        app.launch()
        defer { app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        let keyboard = app.buttons["Use keyboard"]
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        keyboard.tap()
        XCTAssertTrue(app.staticTexts["Keyboard connected"].waitForExistence(timeout: 5))
        app.typeText("draft from keyboard")
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["New line"].isHittable)
        XCTAssertLessThanOrEqual(app.buttons["New line"].frame.maxY, app.keyboards.firstMatch.frame.minY + 1)
        capture(app, "session-keyboard-before-background")

        XCUIDevice.shared.press(.home)
        XCTAssertTrue(app.wait(for: .runningBackground, timeout: 5))
        app.activate()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.navigationBars["Session input study"].exists)
        XCTAssertTrue(keyboard.exists)
        XCTAssertFalse(app.staticTexts["Keyboard connected"].exists)
        XCTAssertFalse(app.buttons["Reconnect"].exists)
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        capture(app, "session-restored-as-observer")

        keyboard.tap()
        XCTAssertTrue(app.staticTexts["Keyboard connected"].waitForExistence(timeout: 5))
        app.typeText(" + continued\n")
        capture(app, "session-keyboard-after-return")
    }

    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
