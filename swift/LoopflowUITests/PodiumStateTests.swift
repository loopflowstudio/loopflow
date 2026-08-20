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
    func testConsoleSelectionScopesTheActivityLedger() {
        forEachWidth { app in
            app.descendants(matching: .any)["podium-path-root"].click()
            let wave = app.descendants(matching: .any)["podium-console-wave-wave-1"]
            XCTAssertTrue(wave.waitForExistence(timeout: 8))
            wave.click()

            let project = app.descendants(matching: .any)["podium-console-project-project-1"]
            XCTAssertTrue(project.waitForExistence(timeout: 8))
            project.click()
            XCTAssertEqual(
                app.descendants(matching: .any)["podium-activity-scope"].label,
                "Project · Loopflow API"
            )

            // The Project detail lists its Tasks flat; selecting one from the
            // open drawer commits and retracts.
            let task = app.descendants(matching: .any)["podium-console-task-issue-available"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            task.click()
            XCTAssertTrue(waitForAbsence(app, id: "podium-drawer-waves"))
            XCTAssertEqual(
                app.descendants(matching: .any)["podium-activity-scope"].label,
                "Task · W2-156"
            )
            XCTAssertTrue(waitFor(app, id: "podium-detail-task"))
            assertPodiumRegions(app)
        }
    }

    @MainActor
    func testConsoleDrawersDiscloseWorkWithOneOutputLanguage() {
        forEachWidth { app in
            app.descendants(matching: .any)["podium-path-root"].click()
            let wave = app.descendants(matching: .any)["podium-console-wave-wave-1"]
            XCTAssertTrue(wave.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: wave.value).contains("tokens per second"))

            wave.click()
            let project = app.descendants(matching: .any)["podium-console-project-project-1"]
            XCTAssertTrue(project.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: project.value).contains("tokens per second"))

            project.click()
            let task = app.descendants(matching: .any)["podium-console-task-issue-now"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            XCTAssertTrue(String(describing: task.value).contains("tokens per second"))

            // Picking the leaf commits the selection and retracts the cascade.
            task.click()
            XCTAssertTrue(waitForAbsence(app, id: "podium-drawer-waves"))
            let scope = app.descendants(matching: .any)["podium-activity-scope"]
            XCTAssertTrue(scope.waitForExistence(timeout: 8))
            XCTAssertEqual(scope.label, "Task · W2-144")
        }
    }

    @MainActor
    func testWaveProjectAndTaskSelectionsZoomTheWorkPane() {
        forEachWidth { app in
            app.descendants(matching: .any)["podium-path-root"].click()
            let wave = app.descendants(matching: .any)["podium-console-wave-wave-1"]
            XCTAssertTrue(wave.waitForExistence(timeout: 8))
            wave.click()
            XCTAssertTrue(waitFor(app, id: "podium-detail-wave"))
            XCTAssertFalse(exists(app, id: "work-now"))

            let project = app.descendants(matching: .any)["podium-console-project-project-1"]
            XCTAssertTrue(project.waitForExistence(timeout: 8))
            project.click()
            XCTAssertTrue(waitFor(app, id: "podium-detail-project"))
            XCTAssertFalse(exists(app, id: "podium-detail-wave"))

            let task = app.descendants(matching: .any)["podium-console-task-issue-now"]
            XCTAssertTrue(task.waitForExistence(timeout: 8))
            task.click()
            XCTAssertTrue(waitFor(app, id: "podium-detail-task"))
            XCTAssertFalse(exists(app, id: "podium-detail-project"))
        }
    }

    @MainActor
    func testConsoleDrawerClosesWithoutRemovingTheLiveSurface() {
        forEachWidth { app in
            let root = app.descendants(matching: .any)["podium-path-root"]
            XCTAssertTrue(root.waitForExistence(timeout: 8))
            root.click()
            XCTAssertTrue(waitFor(app, id: "podium-drawer-waves"))

            root.click()
            XCTAssertTrue(waitForAbsence(app, id: "podium-drawer-waves"))
            XCTAssertTrue(exists(app, id: "podium-work"))
            XCTAssertTrue(exists(app, id: "podium-activity"))
        }
    }

    @MainActor
    func testLoadingKeepsWaveScopeVisible() {
        forEachWidth(detailState: "loading") { app in
            XCTAssertTrue(waitFor(app, id: "podium-work-loading"))
            XCTAssertTrue(exists(app, id: "podium-console-path"))
            XCTAssertTrue(exists(app, id: "podium-activity-loading"))
            XCTAssertFalse(exists(app, id: "podium-work-unavailable"))
        }
    }

    @MainActor
    func testUnavailableIsNotRenderedAsEmpty() {
        forEachWidth(detailState: "error") { app in
            XCTAssertTrue(waitFor(app, id: "podium-work-unavailable"))
            XCTAssertTrue(exists(app, id: "podium-console-path"))
            XCTAssertFalse(exists(app, id: "podium-work-empty"))
            XCTAssertFalse(exists(app, id: "podium-activity-empty"))
        }
    }

    @MainActor
    func testEmptyPodiumKeepsDistinctWaveWorkAndActivityStates() {
        forEachWidth(mode: "empty-workspaces") { app in
            XCTAssertTrue(waitFor(app, id: "podium-console-empty"))
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
            assertions(app)
            app.terminate()
        }
    }

    @MainActor
    private func assertPodiumRegions(_ app: XCUIApplication) {
        let podium = app.descendants(matching: .any)["podium"]
        XCTAssertTrue(podium.waitForExistence(timeout: 8))
        XCTAssertTrue(waitFor(app, id: "podium-console-path"))
        XCTAssertTrue(waitFor(app, id: "podium-repo-scope"))
        XCTAssertTrue(waitFor(app, id: "podium-work"))
        XCTAssertTrue(waitFor(app, id: "podium-activity"))
        XCTAssertGreaterThan(
            podium.frame.height,
            app.windows.element(boundBy: 0).frame.height * 0.75,
            "The Podium must fill the window instead of centering an intrinsic-height strip"
        )
        let path = app.descendants(matching: .any)["podium-console-path"]
        let work = app.descendants(matching: .any)["podium-work"]
        let activity = app.descendants(matching: .any)["podium-activity"]
        XCTAssertLessThan(
            abs(work.frame.minY - path.frame.maxY),
            12,
            "Work must begin directly below the console path bar"
        )
        XCTAssertLessThan(
            abs(activity.frame.minY - path.frame.maxY),
            12,
            "Activity must begin directly below the console path bar"
        )
    }

    @MainActor
    private func waitFor(_ app: XCUIApplication, id: String, timeout: TimeInterval = 8) -> Bool {
        app.descendants(matching: .any)[id].waitForExistence(timeout: timeout)
    }

    @MainActor
    private func waitForAbsence(
        _ app: XCUIApplication,
        id: String,
        timeout: TimeInterval = 8
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if !app.descendants(matching: .any)[id].exists { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }
        return false
    }

    @MainActor
    private func exists(_ app: XCUIApplication, id: String) -> Bool {
        app.descendants(matching: .any)[id].exists
    }
}
