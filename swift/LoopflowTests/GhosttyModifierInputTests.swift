#if os(macOS) && canImport(GhosttyKit)
import AppKit
import GhosttyKit
import IOKit
import Loopflow
import SwiftUI
import Synchronization
import Testing
@testable import LoopflowMac

@MainActor
@Suite("Desktop terminal modifier input")
struct GhosttyModifierInputTests {
    private final class Output: Sendable {
        let bytes = Mutex(Data())
    }

    @Test("Option-B follows the terminal's Option-as-Alt setting", arguments: [
        ("true", "\u{1b}b\u{1b}b"), ("left", "\u{1b}b∫"), ("right", "∫\u{1b}b"), ("false", "∫∫"),
    ])
    func optionB(setting: String, expected: String) async throws {
        // Bracketed paste makes a mistaken trip through the clipboard API visible.
        try await expectInput(expected, after: "\u{1b}[?2004h", userConfig: "macos-option-as-alt = \(setting)") { view in
            for side in [NX_DEVICELALTKEYMASK, NX_DEVICERALTKEYMASK] {
                let flags = NSEvent.ModifierFlags.option.union(.init(rawValue: UInt(side)))
                view.keyDown(with: try terminalKeyEvent("∫", 0x0B, flags, unmodified: "b"))
            }
        }
    }

    @Test("Kitty disambiguates modified Return, Tab and Control letters")
    func kittyChords() async throws {
        try await expectInput("\r\u{1b}[13;2u\u{1b}[13;5u\u{1b}[9;2u\u{1b}[100;5u\u{1b}b", after: "\u{1b}[>1u") { view in
            for mods: NSEvent.ModifierFlags in [[], .shift, .control] {
                view.keyDown(with: try terminalKeyEvent("\r", 36, mods))
            }
            view.keyDown(with: try terminalKeyEvent("\u{19}", 48, .shift))
            view.keyDown(with: try terminalKeyEvent("\u{4}", 2, .control, unmodified: "d"))
            view.keyDown(with: try terminalKeyEvent("\u{f702}", 123, .option))
        }
    }

    @Test("Kitty reports shifted text, repeats and key releases")
    func kittyEventTypes() async throws {
        try await expectInput("\u{1b}[97:65;2u\u{1b}[47:63;2u\u{1b}[13;5:2u\u{1b}[13;5:3u", after: "\u{1b}[>15u") { view in
            view.keyDown(with: try terminalKeyEvent("A", 0, .shift))
            view.keyDown(with: try terminalKeyEvent("?", 44, .shift))
            view.keyDown(with: try terminalKeyEvent("\r", 36, .control, repeating: true))
            view.keyUp(with: try terminalKeyEvent("\r", 36, .control, type: .keyUp))
        }
    }

    @Test("Lifting left Shift while right Shift is held sends a release")
    func sideAwareRelease() async throws {
        let left = NSEvent.ModifierFlags(rawValue: UInt(NX_DEVICELSHIFTKEYMASK))
        let right = NSEvent.ModifierFlags(rawValue: UInt(NX_DEVICERSHIFTKEYMASK))
        try await expectInput("\u{1b}[57441;2u\u{1b}[57447;2u\u{1b}[57441;2:3u\u{1b}[57447;1:3u", after: "\u{1b}[>11u") { view in
            view.flagsChanged(with: try terminalKeyEvent("", 0x38, [.shift, left], type: .flagsChanged))
            view.flagsChanged(with: try terminalKeyEvent("", 0x3C, [.shift, left, right], type: .flagsChanged))
            view.flagsChanged(with: try terminalKeyEvent("", 0x38, [.shift, right], type: .flagsChanged))
            view.flagsChanged(with: try terminalKeyEvent("", 0x3C, [], type: .flagsChanged))
        }
    }

    @Test("Standalone window, tab, export and paste bindings leave their chords available to the terminal")
    func standaloneShortcuts() async throws {
        let expected = "\u{1b}[100;9u\u{1b}[13;9u\u{1b}[9;5u\u{1b}[49;9u\u{1b}[106;10u\u{1b}[118;10u"
        try await expectInput(expected, after: "\u{1b}[>11u") { view in
            view.keyDown(with: try terminalKeyEvent("d", 2, .command))
            view.keyDown(with: try terminalKeyEvent("\r", 36, .command))
            view.keyDown(with: try terminalKeyEvent("\t", 48, .control))
            view.keyDown(with: try terminalKeyEvent("1", 18, .command))
            view.keyDown(with: try terminalKeyEvent("J", 38, [.command, .shift]))
            view.keyDown(with: try terminalKeyEvent("V", 9, [.command, .shift]))
        }
    }

    @Test("Menu items that require Shift leave the unshifted chord to the terminal")
    func shiftedMenuItems() {
        let menu = NSMenu()
        @discardableResult
        func add(_ key: String, _ modifiers: NSEvent.ModifierFlags) -> NSMenuItem {
            let item = menu.addItem(withTitle: key, action: nil, keyEquivalent: key)
            item.keyEquivalentModifierMask = modifiers
            return item
        }
        func shadowed(_ key: String, _ flags: NSEvent.ModifierFlags) -> Bool {
            GhosttyTerminalShortcut.shadowedByShiftedMenuItem(characters: key, modifiers: flags, in: menu)
        }
        add("\r", [.command, .shift])
        add("e", [.command, .shift])
        add("O", .command)
        let exact = add("k", .command)
        let shifted = add("k", [.command, .shift])
        #expect(shadowed("\r", .command))
        #expect(shadowed("e", .command))
        #expect(shadowed("o", .command))
        #expect(!shadowed("\r", [.command, .shift]))
        #expect(!shadowed("k", .command))
        #expect(!shadowed("d", .command))

        exact.isHidden = true
        #expect(shadowed("k", .command))
        shifted.isEnabled = false
        #expect(!shadowed("k", .command))
        exact.isHidden = false
        exact.isEnabled = false
        shifted.isEnabled = true
        #expect(shadowed("k", .command))
        shifted.isHidden = true
        #expect(!shadowed("k", .command))
    }

    @Test("Committing marked text writes it once and canceling writes nothing")
    func composition() async throws {
        try await expectInput("日本", after: "\u{1b}[>11u") { view in
            let range = NSRange(location: 0, length: 0)
            view.setMarkedText("に", selectedRange: range, replacementRange: range)
            try #require(view.hasMarkedText())
            let shift = NSEvent.ModifierFlags(rawValue: UInt(NX_DEVICELSHIFTKEYMASK))
            view.flagsChanged(with: try terminalKeyEvent("", 0x38, [.shift, shift], type: .flagsChanged))
            view.setMarkedText("", selectedRange: range, replacementRange: range)
            #expect(!view.hasMarkedText())
            view.setMarkedText("にほん", selectedRange: range, replacementRange: range)
            view.insertText("日本", replacementRange: range)
            #expect(!view.hasMarkedText())
        }
    }

    @Test("A focused terminal leaves app menus available and runs each binding once")
    func focusedKeyEquivalents() async throws {
        let bindings = """
        keybind = super+q=text:quit
        keybind = super+,=text:settings
        keybind = super+shift+w=text:close
        keybind = unconsumed:super+d=text:split
        """
        try await expectInput("\u{1b}[13;5usplit\u{1b}[100;9u", after: "\u{1b}[>11uhello", userConfig: bindings, focused: true) { view in
            for (key, code): (String, UInt16) in [("q", 12), (",", 43), ("h", 4), ("m", 46), ("/", 44), ("n", 45)] {
                #expect(try !view.performKeyEquivalent(with: terminalKeyEvent(key, code, .command)))
            }
            #expect(try !view.performKeyEquivalent(with: terminalKeyEvent("W", 13, [.command, .shift])))
            #expect(try view.performKeyEquivalent(with: terminalKeyEvent("\r", 36, .control)))
            #expect(try view.performKeyEquivalent(with: terminalKeyEvent("d", 2, .command)))
            #expect(try view.performKeyEquivalent(with: terminalKeyEvent("a", 0, .command)))
            let surface = try #require(view.surface)
            #expect(ghostty_surface_has_selection(surface))
        }
    }

    @Test("Control-/ reaches the program in its keyboard protocol", arguments: [
        ("", "\u{1f}"), ("\u{1b}[>1u", "\u{1b}[47;5u"),
    ])
    func controlSlash(mode: String, bytes: String) async throws {
        try await expectInput(bytes, after: mode, focused: true) { view in
            let event = try terminalKeyEvent("/", 44, .control)
            #expect(view.performKeyEquivalent(with: event))
        }
    }

    @Test("Command-period redirected to cancelOperation reaches the terminal once")
    func commandPeriod() async throws {
        try await expectInput("\u{1b}[46;9u", after: "\u{1b}[>1u", focused: true) { view in
            let event = try terminalKeyEvent(".", 47, .command)
            // Posting an AppKit event stops the current run loop. Dispatch it from
            // a nested loop so Swift Testing's main loop keeps running.
            let mode = RunLoop.Mode("postTerminalKey")
            let post: @MainActor @Sendable () -> Void = { NSApp.postEvent(event, atStart: true) }
            let timer = Timer(timeInterval: 0, repeats: false) { _ in
                MainActor.assumeIsolated(post)
            }
            RunLoop.current.add(timer, forMode: mode)
            RunLoop.current.run(mode: mode, before: .distantFuture)
            let current = try #require(NSApp.nextEvent(matching: .keyDown, until: .distantPast, inMode: .default, dequeue: true))
            try #require(current.keyCode == event.keyCode && current.modifierFlags.contains(.command))
            #expect(!view.performKeyEquivalent(with: current))

            // A deferred window has no WindowServer number to route the re-sent event by.
            let route = try #require(NSEvent.addLocalMonitorForEvents(matching: .keyDown) { key in
                guard key.timestamp == current.timestamp else { return key }
                #expect(view.performKeyEquivalent(with: key))
                return nil
            })
            defer { NSEvent.removeMonitor(route) }
            view.doCommand(by: #selector(NSResponder.cancelOperation(_:)))
            view.doCommand(by: #selector(NSResponder.cancelOperation(_:)))
        }
    }

    @Test("Ghostty applies personal modifier remaps when encoding terminal keys")
    func remappedModifiers() async throws {
        let remaps = """
        key-remap = shift=ctrl
        key-remap = ctrl=super
        key-remap = super=alt
        key-remap = alt=shift
        """
        try await expectInput("\u{1b}[13;5u\u{1b}[13;9u\u{1b}[13;3u\u{1b}[13;2u", after: "\u{1b}[>11u", userConfig: remaps) { view in
            for flags: NSEvent.ModifierFlags in [.shift, .control, .command, .option] {
                view.keyDown(with: try terminalKeyEvent("\r", 36, flags))
            }
        }
    }

    @Test("Terminal editing and font shortcuts work with personal overrides")
    func surfaceBindings() async throws {
        let bindings = "keybind = alt+arrow_right=text:forward"
        try await expectInput("\u{1}\u{5}\u{15}forward", after: "", userConfig: bindings, focused: true) { view in
            for (key, code): (String, UInt16) in [("\u{f702}", 123), ("\u{f703}", 124), ("\u{7f}", 51)] {
                #expect(try view.performKeyEquivalent(with: terminalKeyEvent(key, code, .command)))
            }
            #expect(try view.performKeyEquivalent(with: terminalKeyEvent("\u{f703}", 124, .option)))
            let surface = try #require(view.surface)
            let size = ghostty_surface_size(surface).cell_height_px
            #expect(try view.performKeyEquivalent(with: terminalKeyEvent("=", 24, .command)))
            #expect(ghostty_surface_size(surface).cell_height_px > size)
            #expect(try view.performKeyEquivalent(with: terminalKeyEvent("0", 29, .command)))
            #expect(ghostty_surface_size(surface).cell_height_px == size)
        }
    }

    @Test("Edit → Select All selects terminal output")
    func selectAll() async throws {
        try await expectInput("", after: "hello") { view in
            let surface = try #require(view.surface)
            #expect(!ghostty_surface_has_selection(surface))
            view.selectAll(nil)
            #expect(ghostty_surface_has_selection(surface))
        }
    }

    @Test("Desktop and palette shortcuts wait for terminal composition", .requiresDisplay)
    func composingInFocusedTerminal() throws {
        // Dispatch through both real monitors without activating the test process over another app.
        final class KeyWindow: NSWindow {
            override var isKeyWindow: Bool { true }
        }
        _ = NSApplication.shared
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Unexpected query") }, repoPath: "/tmp")
        let context = DesktopActionContext(model: model, workspaces: SessionsWorkspaceRegistry(localMachineId: fixtureMachineId),
                                          workspace: nil, pane: nil)
        let monitors = ZStack {
            DesktopActionMonitor(context: { context }, failed: { Issue.record($0) })
            WorkPaletteShortcut(presented: false, restoreFocus: false, toggleSidebar: {},
                                open: { model.navigation.palette = .search })
        }
        let view = GhosttyMetalView(terminal: .shell("composing", machineId: fixtureMachineId),
                                   frame: CGRect(x: 0, y: 0, width: 400, height: 200))
        let window = KeyWindow(contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
        let host = NSHostingView(rootView: monitors)
        window.contentView = host
        host.addSubview(view)
        defer { window.contentView = nil }
        host.layoutSubtreeIfNeeded()
        try #require(window.makeFirstResponder(view))
        for (text, code): (String, UInt16) in [("n", 45), ("k", 40)] {
            let key = try terminalKeyEvent(text, code, .command, windowNumber: window.windowNumber)
            let range = NSRange(location: 0, length: 0)
            view.setMarkedText("に", selectedRange: range, replacementRange: range)
            try #require(view.hasMarkedText())
            NSApp.sendEvent(key)
            #expect(model.navigation.palette == nil)
            view.unmarkText()
            NSApp.sendEvent(key)
            #expect(model.navigation.palette == (text == "n" ? .newTask : .search))
            model.navigation.palette = nil
        }
    }

    private func expectInput(_ expected: String, after terminalOutput: String, userConfig: String = "",
                             focused: Bool = false, send: (GhosttyMetalView) throws -> Void) async throws {
        _ = NSApplication.shared
        try #require(GhosttyManager.libraryReady)
        let config = try #require(ghostty_config_new())
        defer { ghostty_config_free(config) }
        try GhosttyThemeConfiguration.load(GhosttyManager.loopflowConfig, into: config)
        try GhosttyThemeConfiguration.load(userConfig + "\nwindow-vsync = false", into: config)
        try GhosttyThemeConfiguration.load(GhosttyManager.embeddedConfig, into: config)
        ghostty_config_finalize(config)
        try #require(ghostty_config_diagnostics_count(config) == 0)
        var runtime = ghostty_runtime_config_s()
        runtime.wakeup_cb = { @Sendable _ in }
        runtime.action_cb = { @Sendable _, _, _ in false }
        runtime.read_clipboard_cb = { @Sendable _, _, _, _, _, _ in GHOSTTY_CLIPBOARD_READ_UNAVAILABLE }
        runtime.confirm_read_clipboard_cb = { @Sendable _, _, _, _ in }
        runtime.write_clipboard_cb = { @Sendable _, _, _, _, _ in }
        let app = try #require(ghostty_app_new(&runtime, config))
        defer { ghostty_app_free(app) }

        let view = GhosttyMetalView(terminal: .shell("modifiers", machineId: fixtureMachineId),
                                   frame: CGRect(x: 0, y: 0, width: 400, height: 200))
        // Mount before attaching the surface so a responder test needs no display link.
        let window = focused ? NSWindow(contentRect: view.frame, styleMask: .borderless, backing: .buffered, defer: true) : nil
        if let window {
            window.contentView = view
            try #require(window.makeFirstResponder(view))
        }
        defer { window?.contentView = nil }
        let output = Output()
        var options = ghostty_surface_config_new()
        options.platform_tag = GHOSTTY_PLATFORM_MACOS
        options.platform.macos.nsview = Unmanaged.passUnretained(view).toOpaque()
        options.userdata = Unmanaged.passUnretained(output).toOpaque()
        options.io_write_cb = { @Sendable userdata, bytes, count in
            guard let userdata, let bytes else { return }
            let output = Unmanaged<Output>.fromOpaque(userdata).takeUnretainedValue()
            output.bytes.withLock { $0.append(Data(bytes: bytes, count: count)) }
        }
        let surface = try #require(ghostty_surface_new(app, &options))
        view.surface = surface
        defer {
            view.destroySurface()
            withExtendedLifetime(output) {}
        }
        ghostty_surface_process_output(surface, terminalOutput, UInt(terminalOutput.utf8.count))
        try send(view)

        // Ghostty replies on the same IO queue, after the input events.
        ghostty_surface_process_output(surface, "\u{1b}[5n", 4)
        let reply = Data("\u{1b}[0n".utf8)
        let deadline = ContinuousClock.now + .seconds(5)
        while true {
            let bytes = output.bytes.withLock { $0 }
            if bytes.suffix(reply.count) == reply {
                #expect(Array(bytes.dropLast(reply.count)) == Array(expected.utf8))
                return
            }
            try #require(ContinuousClock.now < deadline, "Terminal wrote \(bytes as NSData)")
            try await Task.sleep(for: .milliseconds(10))
        }
    }
}
#endif
