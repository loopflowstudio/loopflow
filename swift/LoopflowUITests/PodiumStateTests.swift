import XCTest

/// Executable proof for The Podium root. Fixture states use the same roadmap
/// and Activity DTO files as Rust and Swift contract tests.
final class PodiumStateTests: XCTestCase {
    private let widths: [Double] = [900, 1440]

    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testPopulatedPodiumKeepsScopeWorkAndActivityVisible() {
        forEachWidth { app in
            assertPodiumRegions(app)
            XCTAssertTrue(waitFor(app, id: "podium-wave-list"))
            XCTAssertTrue(waitFor(app, id: "podium-now-readyForReview"))
            XCTAssertTrue(waitFor(app, id: "podium-now-working"))
            XCTAssertTrue(waitFor(app, id: "podium-token-meter"))
            XCTAssertFalse(exists(app, id: "wave-chat-transcript"),
                           "cold launch must spend the primary window on live administration")

            let task = app.descendants(matching: .any)["podium-task-issue-now"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            task.click()

            let scope = app.descendants(matching: .any)["podium-activity-scope"]
            XCTAssertTrue(scope.waitForExistence(timeout: 8))
            XCTAssertEqual(scope.label, "Task · W2-144")
            XCTAssertTrue(exists(app, id: "podium-selection-summary"))
            assertPodiumRegions(app)
        }
    }

    @MainActor
    func testRoadmapSelectionScopesTheActivityLedger() {
        forEachWidth { app in
            let roadmap = app.buttons["Roadmap"]
            XCTAssertTrue(roadmap.waitForExistence(timeout: 8))
            roadmap.click()

            let project = app.descendants(matching: .any)["podium-project-project-1"]
            XCTAssertTrue(project.waitForExistence(timeout: 8))
            project.click()
            XCTAssertEqual(
                app.descendants(matching: .any)["podium-activity-scope"].label,
                "Project · Loopflow API"
            )

            let task = app.descendants(matching: .any)["podium-task-issue-available"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            task.click()
            XCTAssertEqual(
                app.descendants(matching: .any)["podium-activity-scope"].label,
                "Task · W2-156"
            )
            assertPodiumRegions(app)
        }
    }

    @MainActor
    func testScoreDisclosesWorkHierarchyWithOneOutputLanguage() {
        forEachWidth { app in
            let wave = app.descendants(matching: .any)["podium-score-wave-wave-1"]
            XCTAssertTrue(wave.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: wave.value).contains("tokens per second"))

            app.descendants(matching: .any)["podium-score-wave-wave-1-disclosure"].click()
            let project = app.descendants(matching: .any)["podium-score-project-project-1"]
            XCTAssertTrue(project.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: project.value).contains("tokens per second"))

            app.descendants(matching: .any)["podium-score-project-project-1-disclosure"].click()
            let task = app.descendants(matching: .any)["podium-score-task-issue-now"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: task.value).contains("tokens per second"))

            app.descendants(matching: .any)["podium-score-task-issue-now-disclosure"].click()
            let exec = app.descendants(matching: .any)["podium-score-exec-exec:exec-podium"]
            XCTAssertTrue(exec.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: exec.value).contains("tokens per second"))
        }
    }

    @MainActor
    func testWaveScoreClosesWithoutRemovingTheLiveSurface() {
        forEachWidth { app in
            ensureWaveScoreOpen(app)
            app.descendants(matching: .any)["podium-wave-score-toggle"].click()

            XCTAssertFalse(exists(app, id: "podium-wave-score"))
            XCTAssertTrue(exists(app, id: "podium-work"))
            XCTAssertTrue(exists(app, id: "podium-activity"))

            app.descendants(matching: .any)["podium-wave-score-toggle"].click()
            XCTAssertTrue(waitFor(app, id: "podium-wave-score"))
        }
    }

    @MainActor
    func testLoadingKeepsWaveScopeVisible() {
        forEachWidth(detailState: "loading") { app in
            XCTAssertTrue(waitFor(app, id: "podium-work-loading"))
            XCTAssertTrue(exists(app, id: "podium-wave-list"))
            XCTAssertTrue(exists(app, id: "podium-activity-loading"))
            XCTAssertFalse(exists(app, id: "podium-work-unavailable"))
        }
    }

    @MainActor
    func testUnavailableIsNotRenderedAsEmpty() {
        forEachWidth(detailState: "error") { app in
            XCTAssertTrue(waitFor(app, id: "podium-work-unavailable"))
            XCTAssertTrue(exists(app, id: "podium-wave-list"))
            XCTAssertFalse(exists(app, id: "podium-work-empty"))
            XCTAssertFalse(exists(app, id: "podium-activity-empty"))
        }
    }

    @MainActor
    func testEmptyPodiumKeepsDistinctWaveWorkAndActivityStates() {
        forEachWidth(mode: "empty-workspaces") { app in
            XCTAssertTrue(waitFor(app, id: "podium-wave-score-empty"))
            XCTAssertTrue(waitFor(app, id: "podium-work-empty"))
            XCTAssertTrue(waitFor(app, id: "podium-activity-empty"))
            XCTAssertTrue(
                app.descendants(matching: .any)["podium-work-empty"].label
                    .contains("No planned Work")
            )
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
            ensureWaveScoreOpen(app)
            assertions(app)
            app.terminate()
        }
    }

    @MainActor
    private func ensureWaveScoreOpen(_ app: XCUIApplication) {
        let score = app.descendants(matching: .any)["podium-wave-score"]
        if !score.waitForExistence(timeout: 1) {
            let toggle = app.descendants(matching: .any)["podium-wave-score-toggle"]
            XCTAssertTrue(toggle.waitForExistence(timeout: 8))
            toggle.click()
            XCTAssertTrue(score.waitForExistence(timeout: 8))
        }
    }

    @MainActor
    private func assertPodiumRegions(_ app: XCUIApplication) {
        let podium = app.descendants(matching: .any)["podium"]
        XCTAssertTrue(podium.waitForExistence(timeout: 8))
        XCTAssertTrue(waitFor(app, id: "podium-wave-score"))
        XCTAssertTrue(waitFor(app, id: "podium-work"))
        XCTAssertTrue(waitFor(app, id: "podium-activity"))
        XCTAssertGreaterThan(
            podium.frame.height,
            app.windows.element(boundBy: 0).frame.height * 0.75,
            "The Podium must fill the window instead of centering an intrinsic-height strip"
        )
        let bar = app.descendants(matching: .any)["podium-bar"]
        let work = app.descendants(matching: .any)["podium-work"]
        let activity = app.descendants(matching: .any)["podium-activity"]
        XCTAssertLessThan(
            abs(work.frame.minY - bar.frame.maxY),
            12,
            "Work must begin directly below the Podium bar"
        )
        XCTAssertLessThan(
            abs(activity.frame.minY - bar.frame.maxY),
            12,
            "Activity must begin directly below the Podium bar"
        )
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
