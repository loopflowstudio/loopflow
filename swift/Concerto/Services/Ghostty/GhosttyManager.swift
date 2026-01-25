// Manager for the embedded Ghostty terminal.
// Wraps the libghostty C API for terminal embedding in Concerto.

import Foundation
import SwiftUI

#if canImport(GhosttyKit)
import GhosttyKit

@MainActor
final class GhosttyManager: ObservableObject {
    enum State {
        case uninitialized
        case initializing
        case ready
        case failed(String)
    }

    @Published private(set) var state: State = .uninitialized

    private var app: ghostty_app_t?
    private var config: ghostty_config_t?

    static let shared = GhosttyManager()

    private init() {}

    func initialize() {
        guard state == .uninitialized else { return }
        state = .initializing

        // Create configuration
        guard let cfg = ghostty_config_new() else {
            state = .failed("Failed to create Ghostty config")
            return
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
        runtimeConfig.action_cb = { app, target, action in
            // Handle actions from the terminal
            return false
        }
        runtimeConfig.read_clipboard_cb = { userdata, clipboard, state in
            // Clipboard read callback - minimal implementation
        }
        runtimeConfig.confirm_read_clipboard_cb = { userdata, str, state, request in
            // Confirm clipboard read - minimal implementation
        }
        runtimeConfig.write_clipboard_cb = { userdata, clipboard, content, len, confirm in
            guard let content else { return }
            // Write to system clipboard
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            if len > 0, let data = content.pointee.data {
                let text = String(cString: data)
                pasteboard.setString(text, forType: .string)
            }
        }
        runtimeConfig.close_surface_cb = { userdata, processAlive in
            // Surface close callback
        }

        // Create the app
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
        surfaceConfig.scale_factor = NSScreen.main?.backingScaleFactor ?? 2.0

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
    enum State {
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
}

#endif
