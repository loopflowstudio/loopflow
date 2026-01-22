// AppleScript command handlers for Concerto automation.

import Cocoa

class CaptureScreenshotCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        var result: String?
        var errorString: String?

        MainActor.assumeIsolated {
            let service = CaptureService()
            do {
                let screenshotURL = try service.captureKeyWindow()
                result = screenshotURL.path
            } catch {
                errorString = error.localizedDescription
            }
        }

        if let errorString {
            scriptErrorNumber = NSInternalScriptError
            scriptErrorString = errorString
            return nil
        }

        return result
    }
}
