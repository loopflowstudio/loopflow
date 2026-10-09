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
            return try MainActor.assumeIsolated { () throws -> String in
                let data = try JSONEncoder().encode(WorkLinkRouter.shared.inspect())
                return String(decoding: data, as: UTF8.self)
            }
        } catch {
            scriptErrorNumber = NSInternalScriptError
            scriptErrorString = error.localizedDescription
            return nil
        }
    }
}

/// The direct parameter is data, not an evaluated script or a focus selector.
class SetPaneVisibilityCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        do {
            guard let json = directParameter as? String else {
                throw RegistryQueryError("Expected a JSON pane visibility request.")
            }
            let request = try JSONDecoder().decode(DesktopPaneVisibility.self, from: Data(json.utf8))
            return try MainActor.assumeIsolated { () throws -> String in
                let reading = try WorkLinkRouter.shared.setVisibility(request)
                return String(decoding: try JSONEncoder().encode(reading), as: UTF8.self)
            }
        } catch {
            scriptErrorNumber = NSInternalScriptError
            scriptErrorString = error.localizedDescription
            return nil
        }
    }
}
