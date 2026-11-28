// Manager for the embedded Ghostty terminal.
// Wraps the libghostty C API for terminal embedding in Loopflow.

import Foundation
import Loopflow
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
    static let loopflowThemeChanged = Notification.Name("loopflow.themeChanged")
    static let ghosttyCopyConfirmation = Notification.Name("loopflow.ghosttyCopyConfirmation")
    static let ghosttyTerminalBell = Notification.Name("loopflow.ghosttyTerminalBell")
    static let ghosttySessionReady = Notification.Name("loopflow.ghosttySessionReady")
    static let ghosttyTerminalTitle = Notification.Name("loopflow.ghosttyTerminalTitle")
    static let ghosttySurfaceClosed = Notification.Name("loopflow.ghosttySurfaceClosed")
}

enum TerminalIdentity: Hashable, Sendable {
    case session(String, machineId: String)
    case shell(String, machineId: String)

    var machineId: String {
        switch self {
        case .session(_, let machineId), .shell(_, let machineId): machineId
        }
    }
}

struct GhosttyTerminalTitle {
    let terminal: TerminalIdentity
    let title: String
}

enum GhosttyRuntimeResources {
    static let sourceRevision = "a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0"

    static var directoryURL: URL? { directoryURL(in: .loopflowMacResources) }

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

// Resource bytes travel with creation, so a remote Machine needs no app bundle.
func desktopShellBootstrap() throws -> String {
    guard let root = GhosttyRuntimeResources.directoryURL else {
        throw RegistryQueryError("Desktop shell integration is unavailable")
    }
    let resources = [
        ".zshenv": "task-zshenv",
        "bash-env": "task-bash-env",
        "ghostty/shell-integration/zsh/.zshenv": "shell-integration/zsh/.zshenv",
        "ghostty/shell-integration/zsh/ghostty-integration": "shell-integration/zsh/ghostty-integration",
        "ghostty/shell-integration/bash/ghostty.bash": "shell-integration/bash/ghostty.bash",
        "ghostty/shell-integration/bash/bash-preexec.sh": "shell-integration/bash/bash-preexec.sh",
        "ghostty/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish": "shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish",
    ]
    var files: [String: String] = [:]
    for (destination, source) in resources {
        files[destination] = try String(contentsOf: root.appendingPathComponent(source), encoding: .utf8)
    }
    let script = try String(contentsOf: root.appendingPathComponent("task-shell.sh"), encoding: .utf8)
    let data = try JSONSerialization.data(withJSONObject: ["script": script, "files": files], options: [.sortedKeys])
    return String(decoding: data, as: UTF8.self)
}

#if GHOSTTY_ENABLED
import GhosttyKit
import CoreVideo

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

    static let shared = GhosttyManager()

    private nonisolated(unsafe) var baseline: ghostty_config_t?
    // Current preview plus applied candidate; cancel reuses validated config.
    private nonisolated(unsafe) var themeConfigs: [String: (TerminalPalette, ghostty_config_t)] = [:]
    private(set) var theme = ThemeDefinition.light

    /// Loaded before the person's files so their bindings and values win.
    static let loopflowConfig = """
    font-size = \(Int(Typography.terminalSize))
    adjust-cell-height = 10%
    font-feature = -calt
    font-feature = -liga
    minimum-contrast = 3
    keybind = clear
    keybind = alt+arrow_left=esc:b
    keybind = alt+arrow_right=esc:f
    keybind = super+arrow_left=text:\\x01
    keybind = super+arrow_right=text:\\x05
    keybind = super+backspace=text:\\x15
    keybind = super+shift+arrow_up=jump_to_prompt:-1
    keybind = super+shift+arrow_down=jump_to_prompt:1
    keybind = super+a=select_all
    keybind = performable:shift+arrow_left=adjust_selection:left
    keybind = performable:shift+arrow_right=adjust_selection:right
    keybind = performable:shift+arrow_up=adjust_selection:up
    keybind = performable:shift+arrow_down=adjust_selection:down
    keybind = performable:shift+page_up=adjust_selection:page_up
    keybind = performable:shift+page_down=adjust_selection:page_down
    keybind = performable:shift+home=adjust_selection:home
    keybind = performable:shift+end=adjust_selection:end
    keybind = super+home=scroll_to_top
    keybind = super+end=scroll_to_bottom
    keybind = super+page_up=scroll_page_up
    keybind = super+page_down=scroll_page_down
    keybind = performable:super+j=scroll_to_selection
    keybind = super+==increase_font_size:1
    keybind = super++=increase_font_size:1
    keybind = super+-=decrease_font_size:1
    keybind = super+0=reset_font_size
    keybind = performable:super+e=search_selection
    keybind = performable:super+g=navigate_search:next
    keybind = performable:super+shift+g=navigate_search:previous
    keybind = performable:super+shift+f=end_search
    keybind = performable:escape=end_search

    """

    func applyTheme(_ theme: ThemeDefinition, retaining appliedID: String? = nil) throws {
        if let baseline {
            let candidate: ghostty_config_t
            if let cached = themeConfigs[theme.id], cached.0 == theme.terminal {
                candidate = cached.1
            } else {
                candidate = try GhosttyThemeConfiguration.candidate(theme, baseline: baseline)
                if let previous = themeConfigs[theme.id] { ghostty_config_free(previous.1) }
                themeConfigs[theme.id] = (theme.terminal, candidate)
            }
            if let app {
                ghostty_app_update_config(app, candidate)
                ghostty_app_set_color_scheme(app, theme.isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
            }
            for id in Array(themeConfigs.keys) where id != theme.id && id != appliedID {
                if let discarded = themeConfigs.removeValue(forKey: id) { ghostty_config_free(discarded.1) }
            }
        } else {
            guard Self.libraryReady, let empty = ghostty_config_new() else {
                throw ThemeError("Could not initialize terminal appearance.")
            }
            defer { ghostty_config_free(empty) }
            let candidate = try GhosttyThemeConfiguration.candidate(theme, baseline: empty)
            ghostty_config_free(candidate)
        }
        self.theme = theme
        NotificationCenter.default.post(name: .loopflowThemeChanged, object: theme)
    }

    /// Loaded after personal settings. Ghostty appends Spline Sans Mono as a
    /// fallback for any personal font family. Clearing the theme prevents Ghostty
    /// from replaying personal colors over the palette selected in Appearance.
    /// Provider Sessions launch `lf`, so shell detection skips their hooks.
    /// Keep their historical TERM and match padding to the block overlays.
    static let embeddedConfig = """
    font-family = \(Typography.monoFamily)
    term = xterm-256color
    macos-login-session = false
    shell-integration = detect
    scrollback-limit = 10000000
    link-previews = true
    window-padding-x = \(Int(GhosttyTerminalPadding.x))
    window-padding-y = \(Int(GhosttyTerminalPadding.y))
    window-padding-balance = false
    image-storage-limit = 67108864
    keybind = super+arrow_up=jump_to_prompt:-1
    keybind = super+arrow_down=jump_to_prompt:1
    keybind = super+q=unbind
    keybind = super+,=unbind
    keybind = super+comma=unbind
    keybind = super+shift+w=unbind
    theme =

    """

    /// Ghostty's process-wide state is set up once, however many callers ask.
    static let libraryReady: Bool = {
        guard let resources = GhosttyRuntimeResources.directoryURL,
              setenv("GHOSTTY_RESOURCES_DIR", resources.path, 1) == 0 else { return false }
        return ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv) == GHOSTTY_SUCCESS
    }()

    private init() {}

    func initialize() {
        guard state == .uninitialized else { return }
        state = .initializing

        // Initialize Ghostty library
        guard Self.libraryReady else {
            state = .failed("Could not initialize Ghostty with its bundled shell resources")
            return
        }

        // Create configuration
        guard let cfg = ghostty_config_new() else {
            state = .failed("Failed to create Ghostty config")
            return
        }

        let config: ghostty_config_t
        do {
            try GhosttyThemeConfiguration.load(Self.loopflowConfig, into: cfg)
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
            try GhosttyThemeConfiguration.load(embeddedConfig, into: cfg)
            ghostty_config_finalize(cfg)
            self.baseline = cfg
            config = try GhosttyThemeConfiguration.candidate(theme, baseline: cfg)
            themeConfigs[theme.id] = (theme.terminal, config)
        } catch {
            if baseline == nil { ghostty_config_free(cfg) }
            state = .failed(error.localizedDescription)
            return
        }
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
            // Every URL goes through Desktop's link routing. Ghostty falls
            // back to a system opener when the embedder returns false.
            guard target.tag == GHOSTTY_TARGET_SURFACE,
                  let surface = target.target.surface,
                  let userdata = ghostty_surface_userdata(surface)
            else { return action.tag == GHOSTTY_ACTION_OPEN_URL }
            // Resolve identity while the callback's surface is valid. Deferred
            // notifications carry values, never an unowned terminal pointer.
            let (view, terminal) = MainActor.assumeIsolated {
                let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                return (view, view.terminal)
            }
            switch action.tag {
            case GHOSTTY_ACTION_OPEN_URL:
                let request = action.action.open_url
                let target = copyLinkTarget(request.url, count: Int(request.len))
                let link = TerminalHyperlink(target: target)
                Task { @MainActor [weak view] in await view?.openLink(link) }
                return true
            case GHOSTTY_ACTION_MOUSE_SHAPE:
                MainActor.assumeIsolated { view.setMouseShape(action.action.mouse_shape) }
                return true
            case GHOSTTY_ACTION_MOUSE_OVER_LINK:
                let request = action.action.mouse_over_link
                let target = copyLinkTarget(request.url, count: request.len)
                MainActor.assumeIsolated { view.showLinkTarget(target) }
                return true
            case GHOSTTY_ACTION_START_SEARCH:
                let needle = action.action.start_search.needle.map { String(cString: $0) }
                Task { @MainActor [weak view] in
                    guard let view, !view.childExited else { return }
                    view.searchBar.open(needle: needle)
                }
                return true
            case GHOSTTY_ACTION_END_SEARCH:
                Task { @MainActor [weak view] in view?.searchBar.end() }
                return true
            case GHOSTTY_ACTION_SEARCH_TOTAL:
                let total = action.action.search_total.total
                Task { @MainActor [weak view] in view?.searchBar.receive(total: total) }
                return true
            case GHOSTTY_ACTION_COMMAND_FINISHED:
                let code = action.action.command_finished.exit_code
                Task { @MainActor [weak view] in
                    view?.finished += 1
                    view?.lastExitStatus = code < 0 ? nil : Int(code)
                }
                return true
            case GHOSTTY_ACTION_SEARCH_SELECTED:
                let selected = action.action.search_selected.selected
                Task { @MainActor [weak view] in view?.searchBar.receive(selected: selected) }
                return true
            default: break
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
                if value == "loopflow:input-ready" {
                    guard case .session = terminal else { return true }
                    let incarnation = MainActor.assumeIsolated {
                        Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
                            .programStatus.incarnation.uuidString.lowercased()
                    }
                    Task { @MainActor in
                        NotificationCenter.default.post(name: .ghosttySessionReady, object: terminal,
                            userInfo: ["surface": incarnation])
                    }
                    return true
                }
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

        guard let ghosttyApp = ghostty_app_new(&runtimeConfig, config) else {
            state = .failed("Failed to create Ghostty app")
            return
        }

        self.app = ghosttyApp
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardSelectionChanged),
            name: NSTextInputContext.keyboardSelectionDidChangeNotification, object: nil)
        ghostty_app_set_color_scheme(ghosttyApp, theme.isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
        state = .ready
    }

    @objc private func keyboardSelectionChanged(_ notification: Notification) {
        if let app { ghostty_app_keyboard_changed(app) }
    }

    func tick() {
        guard let app else { return }
        ghostty_app_tick(app)
    }

    func createSurface(
        workingDirectory: String,
        command: String? = nil,
        initialInput: String? = nil,
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
            return withOptionalCString(command) { cmdPtr in
                surfaceConfig.command = cmdPtr
                return withOptionalCString(initialInput) { inputPtr in
                    surfaceConfig.initial_input = inputPtr
                    return ghostty_surface_new(app, &surfaceConfig)
                }
            }
        }
    }

    deinit {
        if let app {
            ghostty_app_free(app)
        }
        for (_, config) in themeConfigs.values { ghostty_config_free(config) }
        if let baseline { ghostty_config_free(baseline) }
    }
}

private func copyLinkTarget(_ bytes: UnsafePointer<CChar>?, count: Int) -> String? {
    guard let bytes, count > 0 else { return nil }
    // Keep an invalid target distinct from Ghostty clearing the hover.
    return String(validating: UnsafeRawBufferPointer(start: bytes, count: count), as: UTF8.self) ?? ""
}

private func withOptionalCString<Result>(_ text: String?, _ body: (UnsafePointer<CChar>?) -> Result) -> Result {
    guard let text else { return body(nil) }
    return text.withCString { body($0) }
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

    private(set) var theme = ThemeDefinition.light
    func applyTheme(_ theme: ThemeDefinition, retaining appliedID: String? = nil) throws { self.theme = theme }

    private init() {}

    func initialize() {
        state = .failed("GhosttyKit not available - build Ghostty first")
    }

    func tick() {}
}

#endif
