import XCTest

/// Executable proof for the new primary control-room root. Fixture states use
/// the same roadmap DTO file as Rust and Swift contract tests.
final class ControlRoomStateTests: XCTestCase {
    private let widths: [Double] = [900, 1440]

    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testPopulatedControlRoomKeepsScopeWorkAndSelectionVisible() {
        forEachWidth { app in
            assertControlRoomRegions(app)
            XCTAssertTrue(waitFor(app, id: "control-room-wave-list"))
            XCTAssertTrue(waitFor(app, id: "control-room-now-readyForReview"))
            XCTAssertTrue(waitFor(app, id: "control-room-now-working"))
            XCTAssertFalse(exists(app, id: "wave-chat-transcript"),
                           "cold launch must spend the primary window on live administration")

            let task = app.descendants(matching: .any)["control-room-task-issue-now"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            task.click()

            let title = app.descendants(matching: .any)["control-room-selection-title"]
            XCTAssertTrue(title.waitForExistence(timeout: 8))
            XCTAssertEqual(title.label, "Make lf roadmap the machine-wide view")
            assertControlRoomRegions(app)
        }
    }

    @MainActor
    func testRoadmapProjectAndAvailableTaskSelectIntoInspector() {
        forEachWidth { app in
            let roadmap = app.buttons["Roadmap"]
            XCTAssertTrue(roadmap.waitForExistence(timeout: 8))
            roadmap.click()

            let project = app.descendants(matching: .any)["control-room-project-project-1"]
            XCTAssertTrue(project.waitForExistence(timeout: 8))
            project.click()
            XCTAssertEqual(
                app.descendants(matching: .any)["control-room-selection-title"].label,
                "Loopflow API"
            )

            let task = app.descendants(matching: .any)["control-room-task-issue-available"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            task.click()
            XCTAssertEqual(
                app.descendants(matching: .any)["control-room-selection-title"].label,
                "Wave Chat consumes the roadmap snapshot"
            )
            assertControlRoomRegions(app)
        }
    }

    @MainActor
    func testLoadingKeepsPortfolioScopeVisible() {
        forEachWidth(detailState: "loading") { app in
            XCTAssertTrue(waitFor(app, id: "control-room-work-loading"))
            XCTAssertTrue(exists(app, id: "control-room-wave-list"))
            XCTAssertFalse(exists(app, id: "control-room-work-unavailable"))
        }
    }

    @MainActor
    func testUnavailableIsNotRenderedAsEmpty() {
        forEachWidth(detailState: "error") { app in
            XCTAssertTrue(waitFor(app, id: "control-room-work-unavailable"))
            XCTAssertTrue(exists(app, id: "control-room-wave-list"))
            XCTAssertFalse(exists(app, id: "control-room-work-empty"))
        }
    }

    @MainActor
    func testEmptyFleetHasDistinctRosterAndWorkStates() {
        forEachWidth(mode: "empty-workspaces") { app in
            XCTAssertTrue(waitFor(app, id: "control-room-roster-empty"))
            XCTAssertTrue(waitFor(app, id: "control-room-work-empty"))
            XCTAssertTrue(exists(app, id: "control-room-no-selection"))
        }
    }

    @MainActor
    private func forEachWidth(
        mode: String = "mock-waves",
        detailState: String? = nil,
        _ assertions: (XCUIApplication) -> Void
    ) {
        for width in widths {
            var launch = WaveSurfaceLaunch()
            launch.mode = mode
            launch.detailState = detailState
            launch.width = width
            let app = launch.makeApp()
            app.launch()
            XCTAssertTrue(app.windows.element(boundBy: 0).waitForExistence(timeout: 10))
            assertions(app)
            app.terminate()
        }
    }

    @MainActor
    private func assertControlRoomRegions(_ app: XCUIApplication) {
        XCTAssertTrue(waitFor(app, id: "control-room"))
        XCTAssertTrue(waitFor(app, id: "control-room-sidebar"))
        XCTAssertTrue(waitFor(app, id: "control-room-work"))
        XCTAssertTrue(waitFor(app, id: "control-room-inspector"))
    }

    @MainActor
    private func waitFor(_ app: XCUIApplication, id: String, timeout: TimeInterval = 8) -> Bool {
        app.descendants(matching: .any)[id].waitForExistence(timeout: timeout)
    }

    @MainActor
    private func exists(_ app: XCUIApplication, id: String) -> Bool {
        app.descendants(matching: .any)[id].exists
    }
}
