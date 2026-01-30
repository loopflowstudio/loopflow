// Manager for the embedded Ghostty terminal.
// Wraps the libghostty C API for terminal embedding in Concerto.

import Foundation
import SwiftUI

#if GHOSTTY_ENABLED
import GhosttyKit

@MainActor
final class GhosttyManager: ObservableObject {
    enum State: Equatable {
        case uninitialized
        case initializing
        case ready
        case failed(String)
    }

    @Published private(set) var state: State = .uninitialized

    private nonisolated(unsafe) var app: ghostty_app_t?
    private nonisolated(unsafe) var config: ghostty_config_t?

    // Current active session ID (MVP: only one session at a time)
    private var activeSessionId: String?

    // Current active surface for termination
    private nonisolated(unsafe) var activeSurface: ghostty_surface_t?

    // Callback when the active session closes (process exits or user closes terminal)
    var onSessionClosed: (() -> Void)?

    static let shared = GhosttyManager()

    // Loopflow color scheme
    private static let loopflowConfig = """
    # Loopflow Terminal Theme - Cream on Burgundy
    background = #4A1A2C
    foreground = #F5E6D3
    cursor-color = #F5E6D3
    selection-background = #6B2D42
    selection-foreground = #F5E6D3

    # Palette - muted tones complementing burgundy
    palette = 0=#2A0F18
    palette = 1=#C75B75
    palette = 2=#8B9A6B
    palette = 3=#D4A574
    palette = 4=#7B8FA8
    palette = 5=#A67B93
    palette = 6=#7FAFAF
    palette = 7=#E8D4C4

    # Bright variants
    palette = 8=#4A2A38
    palette = 9=#E87B95
    palette = 10=#ABB97B
    palette = 11=#E8C594
    palette = 12=#9BAFC8
    palette = 13=#C69BB3
    palette = 14=#9FCFCF
    palette = 15=#F5E6D3

    font-size = 13
    """

    private init() {}

    private func writeLoopflowConfig() -> String? {
        let tempDir = FileManager.default.temporaryDirectory
        let configPath = tempDir.appendingPathComponent("concerto-ghostty-config")
        do {
            try Self.loopflowConfig.write(to: configPath, atomically: true, encoding: .utf8)
            return configPath.path
        } catch {
            print("[GhosttyManager] Failed to write config: \(error)")
            return nil
        }
    }

    func initialize() {
        guard state == .uninitialized else { return }
        state = .initializing

        // Initialize Ghostty library
        let initResult = ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv)
        guard initResult == GHOSTTY_SUCCESS else {
            state = .failed("ghostty_init failed with code \(initResult)")
            return
        }

        // Create configuration
        guard let cfg = ghostty_config_new() else {
            state = .failed("Failed to create Ghostty config")
            return
        }

        // Load Loopflow theme first, then user defaults
        if let path = writeLoopflowConfig() {
            path.withCString { ghostty_config_load_file(cfg, $0) }
        }
        ghostty_config_load_default_files(cfg)
        ghostty_config_finalize(cfg)
        self.config = cfg

        // Create runtime config with callbacks
        var runtimeConfig = ghostty_runtime_config_s()
        runtimeConfig.userdata = Unmanaged.passUnretained(self).toOpaque()
        runtimeConfig.supports_selection_clipboard = false
        runtimeConfig.wakeup_cb = { userdata in
            guard let userdata else { return }
            let manager = Unmanaged<GhosttyManager>.fromOpaque(userdata).takeUnretainedValue()
            Task { @MainActor in
                manager.tick()
            }
        }
        runtimeConfig.action_cb = { _, _, _ in false }
        runtimeConfig.read_clipboard_cb = { _, _, _ in }
        runtimeConfig.confirm_read_clipboard_cb = { _, _, _, _ in }
        runtimeConfig.write_clipboard_cb = { _, _, content, len, _ in
            guard let content, len > 0, let data = content.pointee.data else { return }
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString(String(cString: data), forType: .string)
        }
        runtimeConfig.close_surface_cb = { (userdata: UnsafeMutableRawPointer?, _: Bool) in
            guard let userdata else { return }
            let manager = Unmanaged<GhosttyManager>.fromOpaque(userdata).takeUnretainedValue()
            Task { @MainActor in
                manager.handleSessionClosed()
            }
        }

        guard let ghosttyApp = ghostty_app_new(&runtimeConfig, cfg) else {
            state = .failed("Failed to create Ghostty app")
            return
        }

        self.app = ghosttyApp
        state = .ready
    }

    func tick() {
        guard let app else { return }
        ghostty_app_tick(app)
    }

    func createSurface(
        workingDirectory: String,
        command: String? = nil,
        view: NSView
    ) -> ghostty_surface_t? {
        guard let app, case .ready = state else { return nil }

        var surfaceConfig = ghostty_surface_config_new()
        surfaceConfig.userdata = Unmanaged.passUnretained(view).toOpaque()
        surfaceConfig.platform_tag = GHOSTTY_PLATFORM_MACOS
        surfaceConfig.platform = ghostty_platform_u(
            macos: ghostty_platform_macos_s(nsview: Unmanaged.passUnretained(view).toOpaque())
        )
        surfaceConfig.scale_factor = Double(NSScreen.main?.backingScaleFactor ?? 2.0)

        return workingDirectory.withCString { wdPtr in
            surfaceConfig.working_directory = wdPtr

            if let command {
                return command.withCString { cmdPtr in
                    surfaceConfig.command = cmdPtr
                    return ghostty_surface_new(app, &surfaceConfig)
                }
            } else {
                return ghostty_surface_new(app, &surfaceConfig)
            }
        }
    }

    func destroySurface(_ surface: ghostty_surface_t) {
        ghostty_surface_free(surface)
    }

    /// Register a surface as the active session.
    func registerActiveSession(_ surface: ghostty_surface_t, sessionId: String) {
        activeSessionId = sessionId
        activeSurface = surface
    }

    /// Destroy the active session's surface, killing the child process.
    func destroyActiveSession() {
        guard let surface = activeSurface else { return }
        activeSessionId = nil
        activeSurface = nil
        ghostty_surface_free(surface)
    }

    /// Send text to the active terminal session.
    func sendText(_ text: String) {
        guard let surface = activeSurface else { return }
        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
    }

    /// Called when Ghostty closes the active surface (process exits).
    private func handleSessionClosed() {
        activeSessionId = nil
        activeSurface = nil
        onSessionClosed?()
    }

    deinit {
        if let app {
            ghostty_app_free(app)
        }
        if let config {
            ghostty_config_free(config)
        }
    }
}

#else

// Stub implementation when GhosttyKit is not available
@MainActor
final class GhosttyManager: ObservableObject {
    enum State: Equatable {
        case uninitialized
        case initializing
        case ready
        case failed(String)
    }

    @Published private(set) var state: State = .failed("GhosttyKit not available")

    // Callback when a session closes (stub - never called)
    var onSessionClosed: (() -> Void)?

    static let shared = GhosttyManager()

    private init() {}

    func initialize() {
        state = .failed("GhosttyKit not available - build Ghostty first")
    }

    func tick() {}

    func destroyActiveSession() {
        // Stub - no-op
    }

    func sendText(_ text: String) {
        // Stub - no-op
    }
}

#endif
