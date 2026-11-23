import AppKit
import Loopflow

extension InteractionInput {
    @MainActor
    static var current: InteractionInput? {
        guard let event = NSApp?.currentEvent else { return nil }
        switch event.type {
        case .leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp, .keyDown, .scrollWheel:
            return InteractionInput(uptime: event.timestamp, source: .appKitEvent)
        default: return nil
        }
    }
}
