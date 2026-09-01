import XCTest

final class SessionsInteractionTests: XCTestCase {
    private enum Fixture: String, CaseIterable {
        case interactive
        case ask
        case flow

        var id: String { "ui-\(rawValue)" }
    }

    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testEverySessionKindOpensWithOnlyItsResolutionActions() {
        for fixture in Fixture.allCases {
            var launch = WaveSurfaceLaunch()
            launch.mode = "session-fixtures"
            launch.sessionKind = fixture.rawValue
            let app = launch.makeApp()
            app.launchArguments += ["--repo", repositoryRoot.path]
            app.launch()

            XCTAssertTrue(app.windows.element(boundBy: 0).waitForExistence(timeout: 10))
            let row = element(app, id: "session-row-\(fixture.id)")
            XCTAssertTrue(row.waitForExistence(timeout: 8))
            row.click()

            let pane = element(app, id: "session-pane-\(fixture.id)")
            XCTAssertTrue(pane.waitForExistence(timeout: 8))
            XCTAssertTrue(pane.label.contains("active"))
            XCTAssertTrue(waitForAbsence(app, id: "sessions-empty-new-shell"))

            let complete = element(app, id: "session-action-complete")
            let approve = element(app, id: "session-action-approve")
            let iterate = element(app, id: "session-action-iterate")
            switch fixture {
            case .interactive, .ask:
                XCTAssertTrue(complete.waitForExistence(timeout: 4))
                XCTAssertTrue(complete.isEnabled)
                XCTAssertFalse(approve.exists)
                XCTAssertFalse(iterate.exists)
                complete.click()
            case .flow:
                XCTAssertFalse(complete.exists)
                XCTAssertTrue(approve.waitForExistence(timeout: 4))
                XCTAssertTrue(approve.isEnabled)
                XCTAssertTrue(iterate.exists)
                XCTAssertTrue(iterate.isEnabled)
                approve.click()
                let summary = app.alerts.textFields.element(boundBy: 0)
                XCTAssertTrue(summary.waitForExistence(timeout: 4))
                summary.click()
                summary.typeText("Verified fixture summary")
                let confirmation = app.alerts.buttons["Approve and continue"]
                XCTAssertTrue(confirmation.waitForExistence(timeout: 4))
                XCTAssertTrue(confirmation.isEnabled)
                confirmation.click()
            }

            XCTAssertTrue(waitForAbsence(app, id: "session-row-\(fixture.id)"))
            XCTAssertTrue(
                element(app, id: "sessions-empty-new-shell").waitForExistence(timeout: 8)
            )
            XCTAssertFalse(pane.exists)
            app.terminate()
        }
    }

    private var repositoryRoot: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    @MainActor
    private func element(_ app: XCUIApplication, id: String) -> XCUIElement {
        app.descendants(matching: .any)[id]
    }

    @MainActor
    private func waitForAbsence(
        _ app: XCUIApplication,
        id: String,
        timeout: TimeInterval = 8
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if !element(app, id: id).exists { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }
        return false
    }
}
