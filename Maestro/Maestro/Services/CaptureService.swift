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
        return try captureWindow(window)
    }

    private func captureWindow(_ window: NSWindow) throws -> URL {
        let outputURL = screenshotURL()
        if try captureWindowLayer(window, to: outputURL) {
            return outputURL
        }
        if try captureWindowContent(window, to: outputURL) {
            return outputURL
        }
        return try captureWindowWithScreencapture(window, to: outputURL)
    }

    private func captureWindowLayer(_ window: NSWindow, to outputURL: URL) throws -> Bool {
        guard let contentView = window.contentView else {
            return false
        }

        window.displayIfNeeded()
        contentView.displayIfNeeded()

        let bounds = contentView.bounds
        if bounds.isEmpty {
            return false
        }

        contentView.wantsLayer = true
        guard let layer = contentView.layer ?? contentView.superview?.layer else {
            return false
        }

        let scale = window.backingScaleFactor
        let width = Int(bounds.width * scale)
        let height = Int(bounds.height * scale)
        guard width > 0, height > 0 else {
            return false
        }

        guard let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: width,
            pixelsHigh: height,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        ) else {
            throw CaptureError.captureFailed("Could not allocate bitmap for capture.")
        }

        rep.size = bounds.size
        guard let ctx = NSGraphicsContext(bitmapImageRep: rep) else {
            throw CaptureError.captureFailed("Could not create graphics context.")
        }

        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = ctx
        ctx.cgContext.scaleBy(x: scale, y: scale)
        layer.render(in: ctx.cgContext)
        NSGraphicsContext.restoreGraphicsState()

        guard let pngData = rep.representation(using: .png, properties: [:]) else {
            throw CaptureError.captureFailed("Could not encode layer capture.")
        }

        try pngData.write(to: outputURL, options: .atomic)
        return true
    }

    private func captureWindowContent(_ window: NSWindow, to outputURL: URL) throws -> Bool {
        guard let contentView = window.contentView else {
            return false
        }

        window.displayIfNeeded()
        contentView.displayIfNeeded()

        let bounds = contentView.bounds
        if bounds.isEmpty {
            return false
        }

        guard let rep = contentView.bitmapImageRepForCachingDisplay(in: bounds) else {
            return false
        }

        contentView.cacheDisplay(in: bounds, to: rep)

        let image = NSImage(size: bounds.size)
        image.addRepresentation(rep)

        guard
            let tiffData = image.tiffRepresentation,
            let bitmap = NSBitmapImageRep(data: tiffData),
            let pngData = bitmap.representation(using: .png, properties: [:])
        else {
            throw CaptureError.captureFailed("Could not encode window content.")
        }

        try pngData.write(to: outputURL, options: .atomic)
        return true
    }

    private func captureWindowWithScreencapture(_ window: NSWindow, to outputURL: URL) throws -> URL {
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
