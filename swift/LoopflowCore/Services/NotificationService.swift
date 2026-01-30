// NotificationService - local macOS notifications for wave state changes.
// Notifies users when waves need attention, fail, or complete with PRs.

import Foundation
import UserNotifications

#if os(macOS)
import AppKit
#endif

public final class NotificationService: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    public static let shared = NotificationService()

    private override init() {
        super.init()
        UNUserNotificationCenter.current().delegate = self
    }

    public func requestAuthorization() async throws {
        let center = UNUserNotificationCenter.current()
        try await center.requestAuthorization(options: [.alert, .sound])
    }

    public func notifyNeedsInteractive(waveId: String, waveName: String, step: String) {
        post(
            id: "interactive-\(waveId)",
            title: "\(waveName) waiting: \(step)",
            body: "Interactive step needs your input",
            waveId: waveId
        )
    }

    public func notifyError(waveId: String, waveName: String, message: String) {
        let truncated = String(message.prefix(100))
        post(
            id: "error-\(waveId)",
            title: "\(waveName) failed",
            body: truncated,
            waveId: waveId
        )
    }

    public func notifyPRReady(waveId: String, waveName: String, prNumber: Int) {
        post(
            id: "pr-\(waveId)",
            title: "\(waveName) PR #\(prNumber)",
            body: "Ready for review",
            waveId: waveId
        )
    }

    private func post(id: String, title: String, body: String, waveId: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        content.threadIdentifier = waveId
        content.userInfo = ["waveId": waveId]

        let request = UNNotificationRequest(
            identifier: id,
            content: content,
            trigger: nil  // deliver immediately
        )

        UNUserNotificationCenter.current().add(request)
    }

    // MARK: - UNUserNotificationCenterDelegate

    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if let waveId = response.notification.request.content.userInfo["waveId"] as? String {
            // Post to internal notification center for app to handle
            NotificationCenter.default.post(
                name: .selectWave,
                object: nil,
                userInfo: ["waveId": waveId]
            )
            // Bring app to front
            #if os(macOS)
            DispatchQueue.main.async {
                NSApplication.shared.activate(ignoringOtherApps: true)
            }
            #endif
        }
        completionHandler()
    }

    // Show notifications even when app is in foreground
    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }
}

// MARK: - Notification.Name extension

public extension Notification.Name {
    static let selectWave = Notification.Name("selectWave")
}
