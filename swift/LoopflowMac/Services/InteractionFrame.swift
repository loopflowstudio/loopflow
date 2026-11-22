import AppKit
import Foundation
import ScreenCaptureKit

/// Desktop capture includes the cost of acquiring a visible native frame.
@MainActor
enum InteractionFrame {
    static func requestAccess() { _ = CGRequestScreenCaptureAccess() }

    static func capture(window: NSWindow?, prepare: @MainActor () -> Void = {}) async -> TimeInterval? {
        guard let window, window.isVisible, window.occlusionState.contains(.visible),
              CGPreflightScreenCaptureAccess() else { return nil }
        prepare()
        window.displayIfNeeded()
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
            guard let source = content.windows.first(where: { $0.windowID == CGWindowID(window.windowNumber) }) else { return nil }
            let filter = SCContentFilter(desktopIndependentWindow: source)
            let configuration = SCStreamConfiguration()
            configuration.width = max(1, Int(window.frame.width * (window.screen?.backingScaleFactor ?? 1)))
            configuration.height = max(1, Int(window.frame.height * (window.screen?.backingScaleFactor ?? 1)))
            configuration.showsCursor = false
            let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration)
            guard image.width > 0, image.height > 0, window.isVisible,
                  window.occlusionState.contains(.visible) else { return nil }
            return ProcessInfo.processInfo.systemUptime
        } catch { return nil }
    }
}
