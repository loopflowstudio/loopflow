// AppleScript command handlers for Loopflow automation.

import Cocoa
import Loopflow

class CaptureScreenshotCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        scriptReply {
            try SnapshotService().snapshotKeyWindow().path
        }
    }
}

/// Passive Apple event: operates on registered windows, never the key window.
class InspectDesktopCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        desktopReply { _ in WorkLinkRouter.shared.inspect() }
    }
}

/// The direct parameter is data, not an evaluated script or a focus selector.
class ControlPaneCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        desktopReply { json in
            try WorkLinkRouter.shared.controlPane(Self.decodeRequest(DesktopPaneCommand.self, from: json))
        }
    }
}

class ReadTerminalTextCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        desktopReply { json in
            try WorkLinkRouter.shared.readText(Self.decodeRequest(DesktopTextRequest.self, from: json))
        }
    }
}

private extension NSScriptCommand {
    static func decodeRequest<Request: Decodable>(_ type: Request.Type, from json: String?) throws -> Request {
        guard let json else {
            throw RegistryQueryError("Expected a JSON Desktop request.")
        }
        return try JSONDecoder().decode(type, from: Data(json.utf8))
    }

    /// Resolve and encode in one MainActor turn. Failed replies grant no replay.
    func desktopReply<Reading: Encodable>(_ read: @MainActor (String?) throws -> Reading) -> String? {
        let json = directParameter as? String
        return scriptReply {
            String(decoding: try JSONEncoder().encode(read(json)), as: UTF8.self)
        }
    }

    func scriptReply(_ operation: @MainActor () throws -> String) -> String? {
        do {
            return try MainActor.assumeIsolated(operation)
        } catch {
            scriptErrorNumber = NSInternalScriptError
            scriptErrorString = error.localizedDescription
            return nil
        }
    }
}
