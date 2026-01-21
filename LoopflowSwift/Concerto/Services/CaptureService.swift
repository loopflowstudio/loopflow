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

    /// Capture the key window and save to /tmp/maestro-<timestamp>.png
    func captureKeyWindow() throws -> URL {
        guard let window = NSApp.keyWindow else {
            throw CaptureError.noWindow
        }
        return try captureWindow(window, to: screenshotURL())
    }

    /// Capture the key window to a specific path
    func captureKeyWindow(to outputPath: String) throws -> URL {
        guard let window = NSApp.keyWindow else {
            throw CaptureError.noWindow
        }
        return try captureWindow(window, to: URL(fileURLWithPath: outputPath))
    }

    func captureWindow(_ window: NSWindow, to outputPath: String) throws -> URL {
        try captureWindow(window, to: URL(fileURLWithPath: outputPath))
    }

    /// Capture a window using screencapture command.
    /// This is the most reliable method for capturing NavigationSplitView and other complex views.
    func captureWindow(_ window: NSWindow, to outputURL: URL) throws -> URL {
        let windowID = window.windowNumber
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

    private func screenshotURL() -> URL {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        let timestamp = formatter.string(from: Date())

        return URL(fileURLWithPath: "/tmp/maestro-\(timestamp).png")
    }
}
