import CoreMedia
import Foundation
import Loopflow
import Observation
import OSLog
import ReplayKit
import UIKit

/// Phone capture observes the next composited ReplayKit video frame.
@MainActor
enum InteractionFrame {
    static var isEnabled: Bool { ScreenRecorder.shared.enabled }
    static func enableCapture() async throws { try await ScreenRecorder.shared.start() }
    static func disableCapture() async { await ScreenRecorder.shared.stop() }

    static func capture(window: UIWindow?, prepare: @MainActor () -> Void = {}) async -> TimeInterval? {
        guard let window, !window.isHidden, window.windowScene?.activationState == .foregroundActive,
              ScreenRecorder.shared.enabled else { return nil }
        prepare()
        window.layoutIfNeeded()
        CATransaction.flush()
        let time = await ScreenRecorder.shared.frames.next(after: ProcessInfo.processInfo.systemUptime)
        guard !window.isHidden, window.windowScene?.activationState == .foregroundActive else { return nil }
        return time
    }
}

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
