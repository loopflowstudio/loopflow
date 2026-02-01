// Service for snapshotting windows without Screen Recording permission.

import AppKit
import Foundation

@MainActor
struct SnapshotService {
    enum SnapshotError: LocalizedError {
        case noWindow
        case noContentView
        case renderFailed

        var errorDescription: String? {
            switch self {
            case .noWindow: return "No window to snapshot"
            case .noContentView: return "No window content to snapshot"
            case .renderFailed: return "Snapshot render failed"
            }
        }
    }

    /// Snapshot the key window and save to /tmp/concerto-<timestamp>.png
    func snapshotKeyWindow() throws -> URL {
        guard let window = NSApp.keyWindow else {
            throw SnapshotError.noWindow
        }
        return try snapshotWindow(window, to: snapshotURL())
    }

    /// Snapshot the key window to a specific path
    func snapshotKeyWindow(to outputPath: String) throws -> URL {
        guard let window = NSApp.keyWindow else {
            throw SnapshotError.noWindow
        }
        return try snapshotWindow(window, to: URL(fileURLWithPath: outputPath))
    }

    func snapshotWindow(_ window: NSWindow, to outputPath: String) throws -> URL {
        try snapshotWindow(window, to: URL(fileURLWithPath: outputPath))
    }

    /// Snapshot a window by rendering its content view.
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL {
        guard let view = window.contentView else {
            throw SnapshotError.noContentView
        }

        // Force window and view to update
        window.displayIfNeeded()
        view.layoutSubtreeIfNeeded()
        let bounds = view.bounds

        guard let rep = view.bitmapImageRepForCachingDisplay(in: bounds) else {
            throw SnapshotError.renderFailed
        }

        view.cacheDisplay(in: bounds, to: rep)

        guard let data = rep.representation(using: .png, properties: [:]) else {
            throw SnapshotError.renderFailed
        }

        try data.write(to: outputURL)
        return outputURL
    }

    private func snapshotURL() -> URL {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        let timestamp = formatter.string(from: Date())

        return URL(fileURLWithPath: "/tmp/concerto-\(timestamp).png")
    }
}
