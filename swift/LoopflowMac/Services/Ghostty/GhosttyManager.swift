// Manager for the embedded Ghostty terminal.
// Wraps the libghostty C API for terminal embedding in Loopflow.

import Foundation
import SwiftUI
import AppKit
import Darwin

extension Notification.Name {
    static let ghosttyTerminalBell = Notification.Name("loopflow.ghosttyTerminalBell")
    static let ghosttyTerminalTitle = Notification.Name("loopflow.ghosttyTerminalTitle")
    static let ghosttySurfaceClosed = Notification.Name("loopflow.ghosttySurfaceClosed")
}

enum TerminalIdentity: Hashable, Sendable {
    case session(String)
    case shell(String)
    case taskTerminal(String)
}

struct GhosttyTerminalTitle {
    let terminal: TerminalIdentity
    let title: String
}

#if GHOSTTY_ENABLED
import GhosttyKit
import CoreVideo

enum GhosttyRuntimeResources {
    static let sourceRevision = "4c838723173da757a16a2f3afd4c94f16732ef6a"

    static var directoryURL: URL? {
        #if SWIFT_PACKAGE
        if let resources = Bundle.main.resourceURL,
           let bundle = Bundle(url: resources.appendingPathComponent(
               "LoopflowSwift_LoopflowMac.bundle",
               isDirectory: true
           )),
           let directory = directoryURL(in: bundle) {
            return directory
        }
        return directoryURL(in: .module)
        #else
        return directoryURL(in: .main)
        #endif
    }

    static func directoryURL(in bundle: Bundle) -> URL? {
        guard let resources = bundle.resourceURL else { return nil }
        let root = resources.appendingPathComponent("GhosttyResources", isDirectory: true)
        let revision = root.appendingPathComponent("REVISION")
        let share = root.appendingPathComponent("share", isDirectory: true)
        let directory = share.appendingPathComponent("ghostty", isDirectory: true)
        let integration = directory.appendingPathComponent(
            "shell-integration/zsh/ghostty-integration"
        )
        let terminfo = share.appendingPathComponent("terminfo/78/xterm-ghostty")

        guard let bundledRevision = try? String(contentsOf: revision, encoding: .utf8),
              bundledRevision.trimmingCharacters(in: .whitespacesAndNewlines) == sourceRevision,
              FileManager.default.fileExists(atPath: integration.path),
              FileManager.default.fileExists(atPath: terminfo.path)
        else { return nil }
        return directory
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

    // These settings load after user defaults. Resources enable Ghostty's
    // native shell integration for detected shells; provider Sessions launch
    // `lf`, so they receive no shell hooks. Keep their historical TERM value
    // even though the matching Ghostty terminfo is now bundled.
    private static let embeddedConfig = """
    term = xterm-256color
    shell-integration = detect
    scrollback-limit = 10000000
    image-storage-limit = 67108864
    """

    private init() {}

    private func writeConfig(_ contents: String, named name: String) -> String? {
        let tempDir = FileManager.default.temporaryDirectory
        let configPath = tempDir.appendingPathComponent(name)
        do {
            try contents.write(to: configPath, atomically: true, encoding: .utf8)
            return configPath.path
        } catch {
            print("[GhosttyManager] Failed to write config: \(error)")
            return nil
        }
    }

    func initialize() {
        guard state == .uninitialized else { return }
        state = .initializing

        guard let resources = GhosttyRuntimeResources.directoryURL else {
            state = .failed("Bundled Ghostty shell resources are missing or mismatched")
            return
        }
        guard setenv("GHOSTTY_RESOURCES_DIR", resources.path, 1) == 0 else {
            state = .failed("Failed to configure bundled Ghostty shell resources")
            return
        }

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
        if let path = writeConfig(Self.loopflowConfig, named: "loopflow-ghostty-theme") {
            path.withCString { ghostty_config_load_file(cfg, $0) }
        }
        ghostty_config_load_default_files(cfg)
        var embeddedConfig = Self.embeddedConfig
        // Ghostty treats a failed CoreVideo display link as an allocation error
        // and refuses every surface. Its timer renderer works without that link.
        var displayLink: CVDisplayLink?
        let displayLinkStatus = CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)
        if displayLinkStatus != kCVReturnSuccess || displayLink == nil {
            embeddedConfig += "\nwindow-vsync = false\n"
            print("[GhosttyManager] CoreVideo display link unavailable (\(displayLinkStatus)); using timer rendering")
        }
        displayLink = nil
        if let path = writeConfig(embeddedConfig, named: "loopflow-ghostty-embedded") {
            path.withCString { ghostty_config_load_file(cfg, $0) }
        }
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
        runtimeConfig.action_cb = { _, target, action in
            guard target.tag == GHOSTTY_TARGET_SURFACE else { return false }
            guard let surface = target.target.surface,
                  let userdata = ghostty_surface_userdata(surface)
            else { return false }
            // Resolve identity while the callback's surface is valid. Deferred
            // notifications carry values, never an unowned terminal pointer.
            let terminal = MainActor.assumeIsolated {
                Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue().terminal
            }
            if action.tag == GHOSTTY_ACTION_RING_BELL {
                Task { @MainActor in
                    NotificationCenter.default.post(name: .ghosttyTerminalBell, object: terminal)
                }
                return true
            }
            if action.tag == GHOSTTY_ACTION_SET_TITLE,
               let title = action.action.set_title.title {
                let value = String(cString: title)
                Task { @MainActor in
                    NotificationCenter.default.post(
                        name: .ghosttyTerminalTitle,
                        object: GhosttyTerminalTitle(terminal: terminal, title: value)
                    )
                }
                return true
            }
            return false
        }
        runtimeConfig.read_clipboard_cb = { userdata, _, state in
            guard let userdata else { return }
            MainActor.assumeIsolated {
                let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                guard let surface = view.surface else { return }
                let value = NSPasteboard.general.string(forType: .string) ?? ""
                value.withCString {
                    ghostty_surface_complete_clipboard_request(surface, $0, state, false)
                }
            }
        }
        runtimeConfig.confirm_read_clipboard_cb = { userdata, content, state, request in
            guard let userdata else { return }
            MainActor.assumeIsolated {
                let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                guard let surface = view.surface else { return }
                let allowed = request == GHOSTTY_CLIPBOARD_REQUEST_PASTE
                    || request == GHOSTTY_CLIPBOARD_REQUEST_OSC_52_WRITE
                let value = allowed ? content.map(String.init(cString:)) ?? "" : ""
                value.withCString {
                    ghostty_surface_complete_clipboard_request(surface, $0, state, allowed)
                }
            }
        }
        runtimeConfig.write_clipboard_cb = { _, _, content, len, _ in
            guard let content, len > 0, let data = content.pointee.data else { return }
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString(String(cString: data), forType: .string)
        }
        runtimeConfig.close_surface_cb = { userdata, _ in
            guard let userdata else { return }
            let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
            Task { @MainActor in
                view.handleSurfaceClose()
                GhosttyManager.shared.tick()
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
        view: GhosttyMetalView
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

    static let shared = GhosttyManager()

    private init() {}

    func initialize() {
        state = .failed("GhosttyKit not available - build Ghostty first")
    }

    func tick() {}
}

#endif
