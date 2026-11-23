import XCTest

final class ScreenshotPipelineTests: XCTestCase {
    @MainActor
    func testCapture() throws {
        let environment = ProcessInfo.processInfo.environment
        let outputName = environment["LOOPFLOW_UI_TEST_NAME"] ?? "loopflow-uitest"
        let delay = Double(environment["LOOPFLOW_UI_TEST_DELAY"] ?? "1.5") ?? 1.5

        var launch = WaveSurfaceLaunch()
        launch.mode = environment["LOOPFLOW_UI_TEST_MODE"] ?? "mock-waves"
        launch.detailState = environment["LOOPFLOW_UI_TEST_DETAIL_STATE"]
        launch.selectBranch = environment["LOOPFLOW_UI_TEST_SELECT_BRANCH"]
        launch.width = environment["LOOPFLOW_UI_TEST_WIDTH"].flatMap(Double.init)

        let app = launch.makeApp()
        app.launch()

        XCTAssertTrue(app.windows.element(boundBy: 0).waitForExistence(timeout: 10))
        if delay > 0 {
            Thread.sleep(forTimeInterval: delay)
        }

        let screenshot = XCUIScreen.main.screenshot()
        let data = screenshot.pngRepresentation
        let outputURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("\(outputName).png")
        try data.write(to: outputURL)
        print("UI_TEST_SCREENSHOT_PATH=\(outputURL.path)")

        app.terminate()
    }
}
