import XCTest

/// Hosted navigation proof. Fixtures establish UI behavior, not external-work trials.
final class PodiumStateTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    @MainActor
    func testUnifiedListOpensUpcomingWorkInBothPresentations() {
        for width in [900.0, 1440.0] {
            var launch = WaveSurfaceLaunch()
            launch.mode = "mock-waves"
            launch.width = width
            let app = launch.makeApp()
            app.launchArguments += ["--repo", "/src/loopflow"]
            app.launch()
            defer { app.terminate() }
            let task = app.buttons["workspace-task-issue-available"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            XCTAssertTrue(app.buttons["workspace-task-issue-now"].exists)
            XCTAssertTrue(app.buttons["workspace-task-issue-review"].exists)
            task.click()
            XCTAssertTrue(element(app, "podium-detail-task").waitForExistence(timeout: 8))
            XCTAssertTrue(element(app, "workspace-task-directive").exists)
            XCTAssertTrue(element(app, "workspace-project-proof").exists)
            XCTAssertTrue(element(app, "workspace-no-sessions").exists)
            XCTAssertFalse(element(app, "workspace-navigator").exists)
            app.buttons["workspace-toggle-list"].click()
            XCTAssertTrue(element(app, "workspace-navigator").exists)
            XCTAssertTrue(element(app, "podium-detail-task").exists)
            app.buttons["workspace-project-project-1"].click()
            XCTAssertTrue(element(app, "podium-detail-project").waitForExistence(timeout: 4))
            app.buttons["workspace-wave-wave-1"].click()
            XCTAssertTrue(element(app, "podium-detail-wave").waitForExistence(timeout: 4))
            app.buttons["workspace-disclose-wave-wave-1"].click()
            XCTAssertFalse(app.buttons["workspace-task-issue-available"].exists)
            app.buttons["workspace-all-work"].click()
            XCTAssertFalse(app.buttons["workspace-task-issue-available"].exists)
            app.buttons["workspace-disclose-wave-wave-1"].click()
            XCTAssertTrue(app.buttons["workspace-task-issue-available"].exists)
        }
    }

    @MainActor
    func testUnavailableAndLoadingPlanningNeverClaimEmpty() {
        for state in ["loading", "error"] {
            var launch = WaveSurfaceLaunch()
            launch.mode = "mock-waves"
            launch.detailState = state
            let app = launch.makeApp()
            app.launch()
            defer { app.terminate() }
            let expected = state == "loading" ? "workspace-planning-loading" : "workspace-planning-unavailable"
            XCTAssertTrue(element(app, expected).waitForExistence(timeout: 8))
            XCTAssertFalse(element(app, "workspace-planning-empty").exists)
        }
    }

    @MainActor
    private func element(_ app: XCUIApplication, _ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id]
    }
}
