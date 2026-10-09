// AppleScript command handlers for Loopflow automation.

import Cocoa
import Loopflow

class CaptureScreenshotCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        var result: String?
        var errorString: String?

        MainActor.assumeIsolated {
            let service = SnapshotService()
            do {
                let screenshotURL = try service.snapshotKeyWindow()
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


/// Passive Apple event: operates on registered windows, never the key window.
class InspectDesktopCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        do {
            let result: String = try MainActor.assumeIsolated {
                let data = try JSONEncoder().encode(WorkLinkRouter.shared.inspect())
                return String(decoding: data, as: UTF8.self)
            }
            return result
        } catch {
            scriptErrorNumber = NSInternalScriptError
            scriptErrorString = error.localizedDescription
            return nil
        }
    }
}
