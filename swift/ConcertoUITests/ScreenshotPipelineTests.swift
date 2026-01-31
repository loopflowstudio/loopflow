import XCTest

final class ScreenshotPipelineTests: XCTestCase {
    func testCapture() throws {
        let environment = ProcessInfo.processInfo.environment
        let outputName = environment["CONCERTO_UI_TEST_NAME"] ?? "concerto-uitest"
        let mode = environment["CONCERTO_UI_TEST_MODE"] ?? "mock-waves"
        let delay = Double(environment["CONCERTO_UI_TEST_DELAY"] ?? "1.5") ?? 1.5
        let selectBranch = environment["CONCERTO_UI_TEST_SELECT_BRANCH"]

        let app = XCUIApplication()
        app.launchArguments += ["-ui-test-mode", mode]
        app.launchEnvironment["CONCERTO_UI_TEST_MODE"] = mode
        if let selectBranch {
            app.launchEnvironment["CONCERTO_UI_TEST_SELECT_BRANCH"] = selectBranch
        }

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
