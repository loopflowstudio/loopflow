import XCTest

/// Shared launch configuration for the Mac Wave surface under UI test.
///
/// The app under test is a SEPARATE process from the runner, so every knob must
/// be forwarded onto `launchEnvironment` — a value that lives only in the
/// runner's own environment never reaches the app. That gap silently rendered
/// the `loading` and `error` states as `selected` (the default) until the
/// forwarding was centralized here, where the screenshot harness and the
/// state-distinctness test both go through it.
struct WaveSurfaceLaunch {
    var mode: String = "mock-waves"
    var detailState: String?
    var selectBranch: String?
    var sessionKind: String?
    /// A fixed window width, so the narrow and wide legs of the
    /// selectable-without-clipping proof are deterministic.
    var width: Double?

    @MainActor
    func makeApp() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-ui-test-mode", mode]
        app.launchEnvironment["LOOPFLOW_UI_TEST_MODE"] = mode
        if let detailState {
            app.launchEnvironment["LOOPFLOW_UI_TEST_DETAIL_STATE"] = detailState
        }
        if let selectBranch {
            app.launchEnvironment["LOOPFLOW_UI_TEST_SELECT_BRANCH"] = selectBranch
        }
        if let sessionKind {
            app.launchEnvironment["LOOPFLOW_UI_TEST_SESSION_KIND"] = sessionKind
        }
        if let width {
            app.launchEnvironment["LOOPFLOW_UI_TEST_WIDTH"] = String(Int(width))
        }
        return app
    }
}
