import Foundation
#if os(macOS)
import AppKit
import ScreenCaptureKit
#elseif os(iOS)
import UIKit
import ReplayKit
import CoreMedia
import Observation
import OSLog
#endif

/// A native screen-frame observation. Desktop includes capture acquisition cost;
/// phone uses the next ReplayKit video frame, never an offscreen view snapshot.
@MainActor
public enum InteractionFrame {
    #if os(macOS)
    public static func requestAccess() { _ = CGRequestScreenCaptureAccess() }

    public static func capture(window: NSWindow?, prepare: @MainActor () -> Void = {}) async -> TimeInterval? {
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
    #elseif os(iOS)
    public static var isEnabled: Bool { ScreenRecorder.shared.enabled }
    public static func enableCapture() async throws { try await ScreenRecorder.shared.start() }
    public static func disableCapture() async { await ScreenRecorder.shared.stop() }

    public static func capture(window: UIWindow?, prepare: @MainActor () -> Void = {}) async -> TimeInterval? {
        guard let window, !window.isHidden, window.windowScene?.activationState == .foregroundActive,
              ScreenRecorder.shared.enabled else { return nil }
        prepare()
        window.layoutIfNeeded()
        CATransaction.flush()
        let time = await ScreenRecorder.shared.frames.next(after: ProcessInfo.processInfo.systemUptime)
        guard !window.isHidden, window.windowScene?.activationState == .foregroundActive else { return nil }
        return time
    }
    #endif
}

#if os(iOS)
/// ReplayKit observes composited app frames. It runs only during an explicitly
/// enabled visual-timing session; no audio, images or movie are retained.
@Observable @MainActor
private final class ScreenRecorder {
    static let shared = ScreenRecorder()
    private(set) var enabled = false
    let frames = ScreenFrames()

    func start() async throws {
        guard !enabled else { return }
        let recorder = RPScreenRecorder.shared()
        recorder.isMicrophoneEnabled = false
        recorder.isCameraEnabled = false
        try await recorder.startCapture { [weak self] sample, kind, error in
            guard let self else { return }
            if let error {
                let message = error.localizedDescription
                Task { @MainActor in
                    self.enabled = false
                    self.frames.stop()
                    Logger(subsystem: "studio.loopflow", category: "perf").error("Visual capture stopped: \(message, privacy: .public)")
                }
                return
            }
            guard kind == .video, CMSampleBufferIsValid(sample), CMSampleBufferGetImageBuffer(sample) != nil else { return }
            let host = CMClockGetTime(CMClockGetHostTimeClock()).seconds
            let acquired = ProcessInfo.processInfo.systemUptime
            let presented = CMSampleBufferGetPresentationTimeStamp(sample).seconds + acquired - host
            Task { @MainActor in
                guard self.enabled else { return }
                self.frames.receive(presented: presented, acquired: acquired)
            }
        }
        guard UIApplication.shared.applicationState == .active else {
            await stopRecorder()
            return
        }
        enabled = true
    }

    func stop() async {
        guard enabled else { return }
        enabled = false
        frames.stop()
        await stopRecorder()
    }

    private func stopRecorder() async {
        await withCheckedContinuation { continuation in
            RPScreenRecorder.shared().stopCapture { _ in continuation.resume() }
        }
    }
}
#endif
