import XCTest

/// Executable proof that the stable Wave surface renders the five states the
/// W2-178 Proof names — selected, loading, error, empty, and future-child
/// indentation — *distinctly*, at both a narrow and a wide desktop size.
///
/// Each state is checked by a UNIQUE on-screen affordance, so the states can't
/// collapse into one another undetected. This is the test that would have
/// caught PR #972's forwarding gap: with `LOOPFLOW_UI_TEST_DETAIL_STATE` not
/// reaching the app, `loading` and `error` rendered as `selected`, and the
/// loading/error assertions below would fail.
///
/// Runs under the host-permissioned macOS UI-test gate (Automation access),
/// the same gate the screenshot pipeline uses.
final class WaveSurfaceStateTests: XCTestCase {
    /// A genuinely narrow desktop (still wide enough to show the 300pt list and
    /// the detail panes' minimums) and a wide one.
    private let widths: [Double] = [900, 1440]

    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testSelectedStateShowsPopulatedHierarchy() {
        forEachWidth { app in
            XCTAssertTrue(waitFor(app, id: "wave-objective-lead"),
                          "selected must lead with the objective")
            XCTAssertFalse(exists(app, id: "wave-detail-loading"),
                           "selected must not show the loading affordance")
            XCTAssertFalse(exists(app, id: "wave-live-status-footer"),
                           "selected must not show the error footer")
            assertEveryWaveRowHittable(app)
        }
    }

    @MainActor
    func testLoadingStateShowsLoadingAffordance() {
        forEachWidth(detailState: "loading") { app in
            XCTAssertTrue(waitFor(app, id: "wave-detail-loading"),
                          "loading must show the loading affordance, not the populated hierarchy")
            XCTAssertFalse(exists(app, id: "wave-live-status-footer"),
                           "loading is not the error state")
            assertEveryWaveRowHittable(app)
        }
    }

    @MainActor
    func testErrorStatePreservesDetailUnderFooter() {
        forEachWidth(detailState: "error") { app in
            XCTAssertTrue(waitFor(app, id: "wave-live-status-footer"),
                          "error must show the quiet cached-plan footer")
            XCTAssertTrue(exists(app, id: "wave-objective-lead"),
                          "error preserves the last-good detail — the objective still leads")
            XCTAssertFalse(exists(app, id: "wave-detail-loading"),
                           "error is not the loading state")
            assertEveryWaveRowHittable(app)
        }
    }

    @MainActor
    func testEmptyStateShowsCalmCreateSurface() {
        forEachWidth(mode: "empty-workspaces") { app in
            XCTAssertTrue(waitFor(app, id: "wave-empty-create"),
                          "empty must offer a calm create surface")
            XCTAssertEqual(waveRows(app).count, 0, "empty has no Wave rows")
        }
    }

    @MainActor
    func testChildIndentationKeepsBothRowsSelectable() {
        forEachWidth(selectBranch: "cadenza") { app in
            let rows = waveRows(app)
            XCTAssertTrue(rows.contains { $0.label.contains("infrastructure") },
                          "the parent Wave stays in the list")
            XCTAssertTrue(rows.contains { $0.label.contains("cadenza") },
                          "the indented child Wave is present")
            assertEveryWaveRowHittable(app)
        }
    }

    // MARK: - Harness

    @MainActor
    private func forEachWidth(
        mode: String = "mock-waves",
        detailState: String? = nil,
        selectBranch: String? = nil,
        _ assertions: (XCUIApplication) -> Void
    ) {
        for width in widths {
            var launch = WaveSurfaceLaunch()
            launch.mode = mode
            launch.detailState = detailState
            launch.selectBranch = selectBranch
            launch.width = width
            let app = launch.makeApp()
            app.launch()
            XCTAssertTrue(app.windows.element(boundBy: 0).waitForExistence(timeout: 10),
                          "window must appear at width \(Int(width))")
            assertions(app)
            app.terminate()
        }
    }

    @MainActor
    private func waveRows(_ app: XCUIApplication) -> [XCUIElement] {
        // WaveRow combines its children into one element labeled "Wave: <name>. <reason>".
        let predicate = NSPredicate(format: "label BEGINSWITH %@", "Wave: ")
        return app.descendants(matching: .any).matching(predicate).allElementsBoundByIndex
    }

    @MainActor
    private func assertEveryWaveRowHittable(_ app: XCUIApplication) {
        let rows = waveRows(app)
        XCTAssertFalse(rows.isEmpty, "expected at least one Wave row")
        for row in rows {
            XCTAssertTrue(row.isHittable,
                          "every Wave stays selectable without clipping: \(row.label)")
        }
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
