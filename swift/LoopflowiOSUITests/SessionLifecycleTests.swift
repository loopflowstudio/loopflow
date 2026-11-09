import XCTest

/// Operate the shipped view/lifecycle wiring with the explicit offline peer.
/// Network/owner acceptance lives in MachineConnectionIntegrationTests.
@MainActor
final class SessionLifecycleTests: XCTestCase {
    func testNavigationKeysEditWithoutSubmittingOrHidingTheKeyboard() throws {
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
        app.typeText("ac")
        let keys = app.buttons["terminal-navigation-keys"]
        XCTAssertTrue(keys.isHittable)
        XCTAssertGreaterThanOrEqual(keys.frame.width, 44)
        XCTAssertGreaterThanOrEqual(keys.frame.height, 44)
        let viewport = app.descendants(matching: .any).matching(identifier: "terminal-viewport").firstMatch
        let height = viewport.frame.height
        keys.tap()
        let left = app.buttons["Left"]
        XCTAssertTrue(left.waitForExistence(timeout: 5))
        capture(app, "session-navigation-keys")
        XCTAssertTrue(app.keyboards.firstMatch.exists)
        for title in ["Up", "Down", "Left", "Right", "Tab", "Escape", "Typing keys"] {
            XCTAssertGreaterThanOrEqual(app.buttons[title].frame.width, 44)
            XCTAssertGreaterThanOrEqual(app.buttons[title].frame.height, 44)
        }
        try app.performAccessibilityAudit(for: [.hitRegion, .sufficientElementDescription])
        XCTAssertEqual(viewport.frame.height, height, accuracy: 1)
        left.tap()
        XCTAssertTrue(app.keyboards.firstMatch.exists)
        app.typeText("b")
        app.buttons["Typing keys"].tap()
        XCTAssertTrue(keys.isHittable)
        XCTAssertEqual(viewport.frame.height, height, accuracy: 1)
        app.buttons["Find in Session"].tap()
        let query = app.textFields["terminal-find-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("abc")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        app.buttons["Close Find"].tap()
        XCTAssertTrue(keys.isHittable, "Closing Find must restore the normal typing bar")
        capture(app, "session-navigation-edited-draft")
    }

    func testLandscapeKeepsTheTerminalAndFindAboveTheKeyboard() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        app.launch()
        defer { XCUIDevice.shared.orientation = .portrait; app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        let keyboard = app.buttons["Use keyboard"]
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        XCUIDevice.shared.orientation = .landscapeLeft
        XCTAssertGreaterThan(app.windows.firstMatch.frame.width, app.windows.firstMatch.frame.height)
        keyboard.tap()
        app.typeText("landscape draft")
        XCTAssertTrue(app.buttons["New line"].waitForExistence(timeout: 5))
        capture(app, "session-landscape-keyboard")
        let viewport = app.descendants(matching: .any).matching(identifier: "terminal-viewport").firstMatch
        XCTAssertGreaterThan(viewport.frame.height, 30)
        XCTAssertLessThan(app.buttons["New line"].frame.midY, app.keyboards.firstMatch.frame.minY)
        app.buttons["Find in Session"].tap()
        let query = app.textFields["terminal-find-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("landscape draft")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        capture(app, "session-landscape-find")
        XCTAssertGreaterThan(viewport.frame.height, 20)
        XCTAssertTrue(app.buttons["Close Find"].isHittable)
        app.buttons["Close Find"].tap()
        app.buttons["Session details"].tap()
        XCTAssertTrue(app.alerts["Session details"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.alerts.staticTexts["Session input study\n\nOffline fixture\n\nKeyboard connected"].exists)
        app.alerts.buttons["Close"].tap()
        app.typeText(" + continued")
        XCUIDevice.shared.orientation = .portrait
        XCTAssertTrue(app.staticTexts["Keyboard connected"].waitForExistence(timeout: 5))
        capture(app, "session-portrait-after-rotation")
        app.buttons["Find in Session"].tap()
        query.buttons["Clear text"].tap()
        query.typeText("landscape draft + continued")
        XCTAssertEqual(query.value as? String, "landscape draft + continued")
        XCTAssertTrue((app.textViews["session-terminal"].value as? String)?.contains("landscape draft + continued") == true,
                      "Terminal input value: \(String(describing: app.textViews["session-terminal"].value))")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["Close Find"].isHittable)
        capture(app, "session-portrait-find-after-rotation")
    }

    func testPortraitAndLandscapeKeepTheUnsentDraft() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        defer { XCUIDevice.shared.orientation = .portrait; app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        let keyboard = app.buttons["Use keyboard"]
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        keyboard.tap()
        app.typeText("portrait draft")

        XCUIDevice.shared.orientation = .landscapeRight
        XCTAssertGreaterThan(app.windows.firstMatch.frame.width, app.windows.firstMatch.frame.height)
        XCTAssertTrue(app.buttons["New line"].isHittable)
        app.typeText(" + landscape")
        capture(app, "draft-landscape")
        XCTAssertGreaterThanOrEqual(app.buttons["New line"].frame.height, 44)
        XCTAssertLessThanOrEqual(app.buttons["New line"].frame.maxY, app.keyboards.firstMatch.frame.minY,
                                 "Typing controls must remain fully above the keyboard")
        XCUIDevice.shared.orientation = .portrait
        XCTAssertLessThan(app.windows.firstMatch.frame.width, app.windows.firstMatch.frame.height)
        app.buttons["Find in Session"].tap()
        let query = app.textFields["terminal-find-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("portrait draft + landscape")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["Close Find"].isHittable)
        capture(app, "draft-portrait")
    }

    func testChooseAnotherSessionAndReturnToTheOriginalDraft() throws {
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
        app.typeText("original draft")
        app.buttons["Sessions"].tap()
        let review = app.buttons.containing(.staticText, identifier: "Design review").firstMatch
        XCTAssertTrue(review.waitForExistence(timeout: 5))
        try app.performAccessibilityAudit(for: [.hitRegion, .sufficientElementDescription])
        capture(app, "session-picker")
        review.tap()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Design review"].exists)
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        keyboard.tap()
        app.typeText("review draft")
        app.buttons["Sessions"].tap()
        app.buttons.containing(.staticText, identifier: "Implementation").firstMatch.tap()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Implementation"].exists)
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        capture(app, "session-picker-return")
    }

    func testFindUsesThePhoneKeyboardWithoutTakingSessionControl() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        app.launch()
        defer { app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        let find = app.buttons["Find in Session"]
        XCTAssertTrue(find.waitForExistence(timeout: 5))
        find.tap()
        let query = app.textFields["terminal-find-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("Loopflow")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Viewing Session"].exists)
        XCTAssertFalse(app.staticTexts["Keyboard connected"].exists)
        capture(app, "session-find-observer")
        query.typeText("\n")
        app.buttons["Close Find"].tap()
        XCTAssertFalse(query.exists)
        XCTAssertTrue(app.buttons["Use keyboard"].exists)
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        capture(app, "session-find-closed")
    }

    func testObserverOpensAnExplicitTerminalLinkAndReturnsWithoutTakingControl() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        app.launch()
        defer { app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        XCTAssertTrue(app.buttons["Use keyboard"].waitForExistence(timeout: 5))
        let terminal = app.descendants(matching: .any).matching(identifier: "session-terminal").firstMatch
        XCTAssertTrue(terminal.waitForExistence(timeout: 5))
        // Tap the first line's OSC 8 label through the real renderer recognizer.
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        terminal.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0))
            .withOffset(CGVector(dx: 25, dy: 10)).tap()
        let close = app.buttons["Close"]
        XCTAssertTrue(close.waitForExistence(timeout: 5))
        capture(app, "observer-terminal-link")
        // Safari is a remote view service; element.tap() used the wrong
        // transform on iOS 26.2. Its reported frame is in app coordinates.
        app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: close.frame.midX, dy: close.frame.midY)).tap()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["Use keyboard"].exists)
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        capture(app, "observer-after-link")
    }

    func testTaskAndSessionAccessibility() throws {
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        app.launch()
        defer { app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        // Contrast/clipping also need rendered review: iOS 26.2 flagged fully
        // visible custom-font labels and text independently measured at 14:1.
        let checks: XCUIAccessibilityAuditType = [.hitRegion, .sufficientElementDescription]
        capture(app, "accessibility-task-list")
        try app.performAccessibilityAudit(for: checks)
        task.tap()
        XCTAssertTrue(app.buttons["Use keyboard"].waitForExistence(timeout: 5))
        capture(app, "accessibility-session")
        try app.performAccessibilityAudit(for: checks)
    }

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
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
