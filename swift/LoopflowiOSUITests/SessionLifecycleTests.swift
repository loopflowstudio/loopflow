import XCTest
import Vision

/// Operate the shipped view/lifecycle wiring with the explicit offline peer.
/// Network/owner acceptance lives in MachineConnectionIntegrationTests.
@MainActor
final class SessionLifecycleTests: XCTestCase {
    func testDesktopScreenReadingSurvivesNewOutput() throws {
        let output = FileManager.default.temporaryDirectory.appending(path: "\(UUID().uuidString).terminal")
        try Data().write(to: output)
        defer { try? FileManager.default.removeItem(at: output) }
        let app = XCUIApplication()
        app.launchArguments = ["--fixture", "--fixture-desktop"]
        app.launchEnvironment["LOOPFLOW_FIXTURE_OUTPUT"] = output.path
        app.launch()
        defer { app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        XCTAssertTrue(app.buttons["Use keyboard"].waitForExistence(timeout: 5))
        let history = (1...160).map { "\r\nHistory line \($0)" }.joined()
        try Data(history.utf8).write(to: output, options: .atomic)
        let terminal = app.textViews["session-terminal"]
        let scrollBar = terminal.otherElements.matching(NSPredicate(format: "label BEGINSWITH %@", "Vertical scroll bar")).firstMatch
        XCTAssertTrue(scrollBar.waitForExistence(timeout: 5))
        let viewport = app.descendants(matching: .any).matching(identifier: "terminal-viewport").firstMatch
        viewport.swipeDown()
        let latest = app.buttons["terminal-latest"]
        XCTAssertTrue(latest.waitForExistence(timeout: 5))
        capture(app, "desktop-screen-reading")
        let before = try visibleHistoryLines(app)
        XCTAssertGreaterThan(before.count, 5)
        let middle = try XCTUnwrap(before.sorted().dropFirst(before.count / 2).first)
        let previousPages = scrollBar.label
        let more = (161...240).map { "\r\nHistory line \($0)" }.joined()
        try Data((history + more).utf8).write(to: output, options: .atomic)
        let arrived = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label != %@", previousPages), object: scrollBar)
        XCTAssertEqual(XCTWaiter.wait(for: [arrived], timeout: 5), .completed)
        capture(app, "desktop-screen-after-output")
        XCTAssertTrue(try visibleHistoryLines(app).contains(middle), "The line being read must remain visible after output")
        XCTAssertTrue(app.buttons["Use keyboard"].exists, "Reading must remain passive")
        latest.tap()
        XCTAssertTrue(latest.waitForNonExistence(timeout: 5))
        XCTAssertTrue(try visibleHistoryLines(app).contains(240), "Latest must reveal the newest output")
    }

    private func visibleHistoryLines(_ app: XCUIApplication) throws -> Set<Int> {
        let image = try XCTUnwrap(app.screenshot().image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.usesLanguageCorrection = false
        request.recognitionLanguages = ["en-US"]
        try VNImageRequestHandler(cgImage: image).perform([request])
        return Set((request.results ?? []).compactMap { observation in
            guard let text = observation.topCandidates(1).first?.string,
                  text.hasPrefix("History line ") else { return nil }
            return Int(text.dropFirst("History line ".count))
        })
    }

    func testStreamingOutputKeepsTheSwipedReadingPosition() throws {
        let output = FileManager.default.temporaryDirectory.appending(path: "\(UUID().uuidString).terminal")
        try Data().write(to: output)
        defer { try? FileManager.default.removeItem(at: output) }
        let app = XCUIApplication()
        app.launchArguments = ["--fixture"]
        app.launchEnvironment["LOOPFLOW_FIXTURE_OUTPUT"] = output.path
        app.launch()
        defer { app.terminate() }
        let task = app.staticTexts["Session input study"]
        XCTAssertTrue(task.waitForExistence(timeout: 10))
        task.tap()
        let keyboard = app.buttons["Use keyboard"]
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        keyboard.tap()
        app.typeText("history\n")
        app.buttons["Hide keyboard"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 5))
        let viewport = app.descendants(matching: .any).matching(identifier: "terminal-viewport").firstMatch
        viewport.swipeDown()
        let latest = app.buttons["terminal-latest"]
        capture(app, "session-stream-swiped")
        guard latest.waitForExistence(timeout: 5) else {
            return XCTFail("Output must not pull a swipe back to the prompt")
        }
        let terminal = app.textViews["session-terminal"]
        let scrollBar = terminal.otherElements.matching(NSPredicate(format: "label BEGINSWITH %@", "Vertical scroll bar")).firstMatch
        let before = try scrollPercentage(scrollBar)
        let previousPages = scrollBar.label
        try Data((1...80).map { "\r\nStreamed output \($0)" }.joined().utf8).write(to: output, options: .atomic)
        let outputArrived = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label != %@", previousPages), object: scrollBar)
        XCTAssertEqual(XCTWaiter.wait(for: [outputArrived], timeout: 5), .completed,
                       "The new output must reach the rendered scrollback before checking retention")
        XCTAssertTrue(latest.exists)
        XCTAssertLessThanOrEqual(try scrollPercentage(scrollBar), before,
                                 "Appending output can reduce the scroll percentage, but must not advance the reader")
        capture(app, "session-stream-retained")
        app.buttons["Find in Session"].tap()
        let query = app.textFields["terminal-find-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("Streamed output")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
    }

    private func scrollPercentage(_ bar: XCUIElement) throws -> Int {
        let value = try XCTUnwrap(bar.value as? String)
        return try XCTUnwrap(Int(value.filter(\.isNumber)), "Missing scroll percentage: \(value)")
    }

    func testSwipeReadsScrollbackAndLatestReturnsToThePrompt() throws {
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
        app.typeText("history\n")
        app.buttons["Hide keyboard"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 5))
        let latest = app.buttons["terminal-latest"]
        XCTAssertFalse(latest.exists, "New output should initially show the prompt")
        let viewport = app.descendants(matching: .any).matching(identifier: "terminal-viewport").firstMatch
        viewport.swipeDown()
        capture(app, "session-swiped-scrollback")
        guard latest.waitForExistence(timeout: 5) else {
            return XCTFail("Reading older output must expose Latest")
        }
        latest.tap()
        XCTAssertTrue(latest.waitForNonExistence(timeout: 5))
        capture(app, "session-scrollback-latest")
        XCTAssertTrue(app.buttons["Show keyboard"].isHittable)
        viewport.swipeDown()
        XCTAssertTrue(latest.waitForExistence(timeout: 5))
        app.buttons["Show keyboard"].tap()
        app.typeText("history\n")
        app.buttons["Hide keyboard"].tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 5))
        capture(app, "session-scrollback-after-reply")
        XCTAssertFalse(latest.exists, "Replying from scrollback should return to following the prompt")
    }

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
        guard query.waitForNonExistence(timeout: 5) else {
            capture(app, "session-find-did-not-close")
            return XCTFail("Close Find must dismiss the search field before terminal typing continues")
        }
        XCTAssertTrue(app.buttons["New line"].isHittable)
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
        app.navigationBars.buttons["Tasks"].tap()
        XCTAssertTrue(task.waitForExistence(timeout: 5))
        app.staticTexts["Review a change"].tap()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.staticTexts["Design review"].exists, "Session choices belong to their own Task")
        app.navigationBars.buttons["Tasks"].tap()
        XCTAssertTrue(task.waitForExistence(timeout: 5))
        task.tap()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 5))
        guard app.staticTexts["Design review"].waitForExistence(timeout: 5) else {
            XCTFail("Reopening the Task should retain its chosen Session")
            return
        }
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        app.buttons["Find in Session"].tap()
        let query = app.textFields["terminal-find-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("review draft")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        app.buttons["Close Find"].tap()
        XCTAssertTrue(keyboard.isHittable, "Returning must not retake keyboard control")
        app.buttons["Sessions"].tap()
        app.buttons.containing(.staticText, identifier: "Implementation").firstMatch.tap()
        XCTAssertTrue(app.staticTexts["Viewing Session"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Implementation"].exists)
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        app.buttons["Find in Session"].tap()
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.typeText("original draft")
        XCTAssertTrue(app.staticTexts["Match found"].waitForExistence(timeout: 5))
        app.buttons["Close Find"].tap()
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
        // Safari starts a separate view service; its controls can arrive
        // after our app's presentation animation has finished.
        guard close.waitForExistence(timeout: 10) else {
            XCTFail("Safari did not expose its Close control")
            return
        }
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
