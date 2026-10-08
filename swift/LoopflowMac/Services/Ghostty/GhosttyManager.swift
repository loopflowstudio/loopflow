// Manager for the embedded Ghostty terminal.
// Wraps the libghostty C API for terminal embedding in Loopflow.

import Foundation
import SwiftUI
import AppKit
import Darwin

// Foundation can select the account's shared temp directory despite TMPDIR.
// Honor the process's explicit directory for embedded terminal files.
func terminalTemporaryDirectory() -> URL {
    if let path = ProcessInfo.processInfo.environment["TMPDIR"], !path.isEmpty {
        return URL(fileURLWithPath: path, isDirectory: true)
    }
    return FileManager.default.temporaryDirectory
}

extension Notification.Name {
    static let ghosttyTerminalBell = Notification.Name("loopflow.ghosttyTerminalBell")
    static let ghosttyTerminalTitle = Notification.Name("loopflow.ghosttyTerminalTitle")
    static let ghosttySurfaceClosed = Notification.Name("loopflow.ghosttySurfaceClosed")
}

enum TerminalIdentity: Hashable, Sendable {
    case session(String)
    case shell(String)
}

struct GhosttyTerminalTitle {
    let terminal: TerminalIdentity
    let title: String
}

#if GHOSTTY_ENABLED
import GhosttyKit
import CoreVideo

enum GhosttyRuntimeResources {
    static let sourceRevision = "a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0"

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

    // Loopflow color scheme — warm charcoal in the canvas's hue family
    // (`TerminalPalette`). Every ANSI color except 0 clears 3:1 on it; 0 is
    // black by convention and stays near the surface.
    static let loopflowConfig = """
    # Loopflow Terminal Theme - Warm charcoal
    background = \(TerminalPalette.css(TerminalPalette.backgroundHex))
    foreground = \(TerminalPalette.css(TerminalPalette.foregroundHex))
    cursor-color = \(TerminalPalette.css(TerminalPalette.accentHex))
    selection-background = \(TerminalPalette.css(TerminalPalette.selectionHex))
    selection-foreground = \(TerminalPalette.css(TerminalPalette.foregroundHex))

    # Palette - muted tones on warm charcoal
    palette = 0=#1A1816
    palette = 1=#D4756A
    palette = 2=#8B9A6B
    palette = 3=#D4A574
    palette = 4=#7B8FA8
    palette = 5=#A67B93
    palette = 6=#7FAFAF
    palette = 7=#C8C1B8

    # Bright variants (8 lifted from #3C4550, which was 1.65:1 on the surface)
    palette = 8=#7F766F
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
    // even though the matching Ghostty terminfo is now bundled. The padding
    // is fixed here because block overlays are laid out from the same numbers.
    // Command-Up and Command-Down move between prompts whatever the person's
    // own Ghostty config binds them to.
    static let embeddedConfig = """
    term = xterm-256color
    macos-login-session = false
    shell-integration = detect
    scrollback-limit = 10000000
    window-padding-x = \(Int(GhosttyTerminalPadding.x))
    window-padding-y = \(Int(GhosttyTerminalPadding.y))
    window-padding-balance = false
    image-storage-limit = 67108864
    keybind = super+arrow_up=jump_to_prompt:-1
    keybind = super+arrow_down=jump_to_prompt:1
    """

    /// Ghostty's process-wide state is set up once, however many callers ask.
    static let libraryReady = ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv) == GHOSTTY_SUCCESS

    private init() {}

    private func loadConfig(_ contents: String, into config: ghostty_config_t) {
        let path = terminalTemporaryDirectory().appendingPathComponent("loopflow-ghostty-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: path) }
        do {
            // The unique file is private until this synchronous load. Atomic
            // Foundation writes can stage in the account's shared temp directory.
            try Data(contents.utf8).write(to: path)
            path.path.withCString { ghostty_config_load_file(config, $0) }
        } catch {
            print("[GhosttyManager] Failed to load embedded config: \(error)")
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
        guard Self.libraryReady else {
            state = .failed("ghostty_init failed")
            return
        }

        // Create configuration
        guard let cfg = ghostty_config_new() else {
            state = .failed("Failed to create Ghostty config")
            return
        }

        // Load Loopflow theme first, then user defaults
        loadConfig(Self.loopflowConfig, into: cfg)
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
        loadConfig(embeddedConfig, into: cfg)
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
            if action.tag == GHOSTTY_ACTION_PROGRAM_STATUS,
               let status = action.action.program_status?.pointee,
               let event = copyProgramStatusEvent(
                   event: Int(status.event.rawValue), state: Int(status.state),
                   kind: Int(status.kind), progress: Int(status.progress),
                   id: status.id, app: status.app, title: status.title, msg: status.msg
               ) {
                let (owner, incarnation) = MainActor.assumeIsolated {
                    let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                    return (view.programStatus, view.programStatus.incarnation)
                }
                Task { @MainActor in owner.receive(event, incarnation: incarnation) }
                return true
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
                MainActor.assumeIsolated {
                    Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue().terminalTitle = value
                }
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
        runtimeConfig.read_clipboard_cb = { userdata, _, state, _, _, _ in
            guard let userdata else { return GHOSTTY_CLIPBOARD_READ_UNAVAILABLE }
            return MainActor.assumeIsolated {
                let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                guard let surface = view.surface else { return GHOSTTY_CLIPBOARD_READ_UNAVAILABLE }
                let value = NSPasteboard.general.string(forType: .string) ?? ""
                value.withCString { text in
                    "text/plain".withCString { mime in
                        var content = ghostty_clipboard_content_s(mime: mime, data: text, len: value.utf8.count)
                        withUnsafePointer(to: &content) { contents in
                            var complete = ghostty_clipboard_complete_s(
                                contents: contents, contents_len: 1, available: nil, available_len: 0,
                                confirmed: false, remember: false
                            )
                            ghostty_surface_complete_clipboard_request(surface, &complete, state)
                        }
                    }
                }
                return GHOSTTY_CLIPBOARD_READ_STARTED
            }
        }
        runtimeConfig.confirm_read_clipboard_cb = { userdata, content, state, request in
            guard let userdata else { return }
            MainActor.assumeIsolated {
                let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                guard let surface = view.surface else { return }
                let allowed = request == GHOSTTY_CLIPBOARD_REQUEST_PASTE
                    || request == GHOSTTY_CLIPBOARD_REQUEST_OSC_52_WRITE
                guard allowed, let content else {
                    ghostty_surface_deny_clipboard_request(surface, state)
                    return
                }
                var complete = ghostty_clipboard_complete_s(
                    contents: content.pointee.contents, contents_len: content.pointee.contents_len,
                    available: content.pointee.available, available_len: content.pointee.available_len,
                    confirmed: true, remember: false
                )
                ghostty_surface_complete_clipboard_request(surface, &complete, state)
            }
        }
        runtimeConfig.write_clipboard_cb = { _, _, content, len, _ in
            guard let content, len > 0, let data = content.pointee.data else { return }
            let bytes = UnsafeRawBufferPointer(start: data, count: content.pointee.len)
            let value = String(decoding: bytes, as: UTF8.self)
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString(value, forType: .string)
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
