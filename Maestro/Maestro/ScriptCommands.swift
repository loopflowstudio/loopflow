// AppleScript command handlers for Maestro automation.

import Cocoa

class CaptureScreenshotCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        guard let repoPath = directParameter as? String else {
            scriptErrorNumber = NSRequiredArgumentsMissingScriptError
            scriptErrorString = "Repository path required"
            return nil
        }

        let repoURL = URL(fileURLWithPath: repoPath)

        // AppleScript commands are delivered on the main thread
        var result: String?
        var errorString: String?

        MainActor.assumeIsolated {
            let service = CaptureService()
            do {
                let screenshotURL = try service.captureWindow(repoRoot: repoURL)
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
