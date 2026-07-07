// Manager for the embedded Ghostty terminal.
// Wraps the libghostty C API for terminal embedding in Concerto.

import Foundation
import SwiftUI
import AppKit

#if GHOSTTY_ENABLED
import GhosttyKit

@MainActor
protocol GhosttySessionSurfaceOwner: AnyObject {
    func destroyManagedSurface(_ surface: ghostty_surface_t)
}

private final class ManagedGhosttySurface {
    weak var owner: GhosttySessionSurfaceOwner?
    let surface: ghostty_surface_t

    init(surface: ghostty_surface_t, owner: GhosttySessionSurfaceOwner) {
        self.surface = surface
        self.owner = owner
    }
}

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

    private nonisolated(unsafe) var surfaces: [String: ManagedGhosttySurface] = [:]

    static let shared = GhosttyManager()

    // Loopflow color scheme — slate grey, adapts to system appearance
    private static let loopflowConfig = """
    # Loopflow Terminal Theme - Slate
    background = #2B3036
    foreground = #F5F1EA
    cursor-color = #F5F1EA
    selection-background = #46505B
    selection-foreground = #F5F1EA

    # Palette - muted tones on slate
    palette = 0=#1E2228
    palette = 1=#D4756A
    palette = 2=#8B9A6B
    palette = 3=#D4A574
    palette = 4=#7B8FA8
    palette = 5=#A67B93
    palette = 6=#7FAFAF
    palette = 7=#C8C1B8

    # Bright variants
    palette = 8=#3C4550
    palette = 9=#E89888
    palette = 10=#ABB97B
    palette = 11=#E8C594
    palette = 12=#9BAFC8
    palette = 13=#C69BB3
    palette = 14=#9FCFCF
    palette = 15=#F5F1EA

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
                manager.tick()
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

    func registerSurface(
        _ surface: ghostty_surface_t,
        sessionId: String,
        owner: GhosttySessionSurfaceOwner
    ) {
        surfaces[sessionId] = ManagedGhosttySurface(surface: surface, owner: owner)
    }

    func unregisterSurface(_ sessionId: String, surface: ghostty_surface_t) {
        guard let handle = surfaces[sessionId], handle.surface == surface else { return }
        surfaces.removeValue(forKey: sessionId)
    }

    func hasSession(_ sessionId: String) -> Bool {
        surfaces[sessionId] != nil
    }

    func destroySession(_ sessionId: String) {
        guard let handle = surfaces.removeValue(forKey: sessionId) else { return }
        if let owner = handle.owner {
            owner.destroyManagedSurface(handle.surface)
        } else {
            ghostty_surface_free(handle.surface)
        }
    }

    func sendText(_ text: String, sessionId: String) {
        guard let surface = surfaces[sessionId]?.surface else { return }
        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
    }

    deinit {
        MainActor.assumeIsolated {
            for handle in surfaces.values {
                if let owner = handle.owner {
                    owner.destroyManagedSurface(handle.surface)
                } else {
                    ghostty_surface_free(handle.surface)
                }
            }
        }
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

    static let shared = GhosttyManager()

    private init() {}

    func initialize() {
        state = .failed("GhosttyKit not available - build Ghostty first")
    }

    func tick() {}

    func hasSession(_ sessionId: String) -> Bool {
        false
    }

    func unregisterSurface(_ sessionId: String, surface: OpaquePointer) {
        // Stub - no-op
    }

    func destroySession(_ sessionId: String) {
        // Stub - no-op
    }

    func sendText(_ text: String, sessionId: String) {
        // Stub - no-op
    }
}

#endif
