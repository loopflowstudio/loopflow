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
            assertStableNavigation(app)
            assertHalLenses(app)
            assertPrimaryInformationHierarchy(app)
            assertEveryWaveRowHittable(app)
        }
    }

    @MainActor
    func testControlIsSecondaryAndOpensToActiveSessions() {
        forEachWidth { app in
            XCTAssertTrue(waitFor(app, id: "wave-chat-transcript"),
                          "Chat must be the selected Wave's default surface")
            XCTAssertFalse(exists(app, id: "control-surface"),
                           "Control must stay behind an explicit interaction")

            let button = app.descendants(matching: .any)["wave-control-button"]
            XCTAssertTrue(button.waitForExistence(timeout: 8),
                          "the secondary Control destination must be discoverable")
            button.click()

            XCTAssertTrue(waitFor(app, id: "control-surface"),
                          "Control must open from the Wave header")
            XCTAssertTrue(waitFor(app, id: "control-active-sessions"),
                          "Control must open to Active Sessions")
            XCTAssertTrue(exists(app, id: "control-run-history-tab"),
                          "Run History remains visible as later disclosure")
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
            XCTAssertTrue(waitFor(app, id: "first-wave-quick-start"),
                          "an empty repository must open the calm first-Wave surface")
            let canonicalRoles = [
                "product": "PRD",
                "infrastructure": "ENG",
                "intelligence": "SCI",
                "operations": "OPS",
            ]
            for (role, tag) in canonicalRoles {
                let choice = app.descendants(matching: .any)["first-wave-role-\(role)"]
                XCTAssertTrue(choice.exists, "the first-Wave surface must offer \(role)")
                XCTAssertTrue(choice.label.contains(tag),
                              "\(role)'s durable \(tag) tag must be visible before the click")
            }
            let custom = app.descendants(matching: .any)["first-wave-custom"]
            XCTAssertTrue(custom.exists, "the fifth choice must allow a custom Wave name")
            custom.click()
            let customName = app.descendants(matching: .any)["first-wave-custom-name"]
            XCTAssertTrue(customName.waitForExistence(timeout: 8),
                          "the custom path must ask for the Wave name")
            XCTAssertTrue(exists(app, id: "first-wave-custom-tag"),
                          "the custom path must require an explicit durable Task tag")
            XCTAssertTrue(exists(app, id: "first-wave-custom-submit"),
                          "the custom path must have one clear Start action")
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
    private func assertStableNavigation(_ app: XCUIApplication) {
        let dropdown = app.descendants(matching: .any)["repo-dropdown"]
        let list = app.descendants(matching: .any)["repo-wave-list"]
        XCTAssertTrue(dropdown.waitForExistence(timeout: 8),
                      "repository scope must be one dropdown above the Wave list")
        XCTAssertTrue(list.waitForExistence(timeout: 8),
                      "the stable Wave list must remain the primary navigation")
        XCTAssertLessThanOrEqual(dropdown.frame.maxY, list.frame.minY,
                                 "the repository dropdown belongs above, not beside, the Wave list")

        let names = waveRows(app).compactMap(Self.waveName)
        XCTAssertEqual(names, ["feedback", "infrastructure", "cadenza", "intelligence"],
                       "liveness must not regroup the stable outline")

        let child = waveRows(app).first { Self.waveName($0) == "cadenza" }
        XCTAssertTrue((child?.value as? String)?.contains("child wave") == true,
                      "the child row must preserve its hierarchy affordance")
    }

    @MainActor
    private func assertHalLenses(_ app: XCUIApplication) {
        let expected = [
            "infrastructure": "green lens",
            "intelligence": "red lens",
            "feedback": "black lens",
            "cadenza": "green lens",
        ]
        let rows = waveRows(app)
        for (name, lens) in expected {
            guard let row = rows.first(where: { Self.waveName($0) == name }) else {
                XCTFail("missing \(name) Wave row")
                continue
            }
            XCTAssertTrue((row.value as? String)?.hasPrefix(lens) == true,
                          "\(name) must expose its \(lens) attention signal")
        }
    }

    @MainActor
    private func assertPrimaryInformationHierarchy(_ app: XCUIApplication) {
        XCTAssertTrue(exists(app, id: "wave-projects"),
                      "Projects must follow the objective")
        XCTAssertTrue(exists(app, id: "wave-project"),
                      "the selected Wave must expose its Project")
        XCTAssertTrue(exists(app, id: "project-open-tasks"),
                      "a Project must foreground its open-task count")
        XCTAssertTrue(exists(app, id: "project-key-result"),
                      "a Project must foreground its KR list")
        XCTAssertTrue(exists(app, id: "wave-task"),
                      "Task rows must stay progressively disclosed under Projects")
        XCTAssertTrue(exists(app, id: "wave-chat-transcript"),
                      "Chat must remain present beside the plan")
        XCTAssertTrue(exists(app, id: "wave-control-button"),
                      "Control must be reachable without replacing Chat")
    }

    @MainActor
    private static func waveName(_ row: XCUIElement) -> String? {
        let prefix = "Wave: "
        guard row.label.hasPrefix(prefix),
              let end = row.label.dropFirst(prefix.count).firstIndex(of: ".")
        else { return nil }
        return String(row.label.dropFirst(prefix.count)[..<end])
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
