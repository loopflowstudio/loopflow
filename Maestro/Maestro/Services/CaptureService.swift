// Service for capturing screenshots for LLM review.

import AppKit
import Foundation

@MainActor
struct CaptureService {
    enum CaptureError: LocalizedError {
        case noWindow
        case captureFailed(String)
        case permissionDenied

        var errorDescription: String? {
            switch self {
            case .noWindow: return "No window to capture"
            case .captureFailed(let msg): return "Capture failed: \(msg)"
            case .permissionDenied: return "Screen Recording permission required. Grant access in System Settings → Privacy & Security → Screen Recording."
            }
        }
    }

    /// Capture the key window and save to .design/screenshots/
    func captureKeyWindow(repoRoot: URL?) throws -> URL {
        guard let window = NSApp.keyWindow else {
            throw CaptureError.noWindow
        }

        let windowID = window.windowNumber
        let outputURL = screenshotURL(repoRoot: repoRoot)

        // Ensure directory exists
        let dir = outputURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        // Use screencapture CLI - more reliable than deprecated CGWindowListCreateImage
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        process.arguments = ["-l", String(windowID), "-x", outputURL.path()]

        let errorPipe = Pipe()
        process.standardError = errorPipe

        try process.run()
        process.waitUntilExit()

        if process.terminationStatus != 0 {
            let errorData = errorPipe.fileHandleForReading.readDataToEndOfFile()
            let errorMessage = String(data: errorData, encoding: .utf8) ?? "Unknown error"
            throw CaptureError.captureFailed(errorMessage)
        }

        // Verify file was created (screencapture silently fails without permission)
        guard FileManager.default.fileExists(atPath: outputURL.path()) else {
            throw CaptureError.permissionDenied
        }

        return outputURL
    }

    private func screenshotURL(repoRoot: URL?) -> URL {
        let dir: URL
        if let repo = repoRoot {
            dir = repo.appendingPathComponent(".design/screenshots")
        } else {
            dir = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Desktop")
        }

        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        let timestamp = formatter.string(from: Date())

        return dir.appendingPathComponent("maestro-\(timestamp).png")
    }
}
