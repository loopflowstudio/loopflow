import XCTest

/// Shared launch configuration for the Mac Wave surface under UI test.
///
/// The app under test is a SEPARATE process from the runner, so every knob must
/// be forwarded onto `launchEnvironment` — a value that lives only in the
/// runner's own environment never reaches the app. That gap once silently
/// rendered every requested state as the default until the forwarding was
/// centralized here, where the screenshot harness goes through it.
struct WaveSurfaceLaunch {
    var mode: String = "mock-waves"
    var detailState: String?
    var selectBranch: String?
    /// A fixed window width, so the narrow and wide legs of the
    /// selectable-without-clipping proof are deterministic.
    var width: Double?
    /// An `lf-world build` directory. A `live` launch reads it through the
    /// app's real `lf`: its `env.json` homes and its repository.
    var world: String?

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
        if let width {
            app.launchEnvironment["LOOPFLOW_UI_TEST_WIDTH"] = String(Int(width))
        }
        if let world {
            let url = URL(fileURLWithPath: world)
            if let data = try? Data(contentsOf: url.appendingPathComponent("env.json")),
               let env = try? JSONDecoder().decode([String: String].self, from: data) {
                app.launchEnvironment.merge(env) { _, world in world }
            }
            app.launchArguments += ["--repo", url.appendingPathComponent("repo").path]
        }
        return app
    }
}
