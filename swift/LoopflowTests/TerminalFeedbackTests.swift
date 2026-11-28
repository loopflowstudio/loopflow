#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Foundation
import GhosttyKit
import IOKit
import Loopflow
import QuartzCore
import Testing
@testable import LoopflowMac

@Suite("Terminal feedback")
struct TerminalFeedbackTests {
    @Test("A paused Session previews and opens its OSC 8 link", .requiresDisplay)
    @MainActor
    func sessionHyperlink() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let view = GhosttyMetalView(terminal: .session("link-proof", machineId: fixtureMachineId))
        view.workingDirectory = NSTemporaryDirectory()
        view.command = #"/bin/sh -c 'printf "\033]8;;https://example.com\007link-proof\033]8;;\007\n"; exec /bin/cat'"#
        let mount = GhosttyTerminalMount(terminal: view)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 480),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = mount
        defer { view.destroySurface(); window.contentView = nil }
        mount.layoutSubtreeIfNeeded()
        view.createSurface(manager: manager)
        let surface = try #require(view.surface)
        try #require(!view.connectionAcceptsInput)
        let deadline = ContinuousClock.now + .seconds(5)
        while true {
            if case .available(let text, _) = try view.readText(region: .screen, maxBytes: 8192),
               text.contains("link-proof") { break }
            try #require(ContinuousClock.now < deadline, "The Session did not print its hyperlink.")
            try await Task.sleep(for: .milliseconds(10))
        }
        let grid = ghostty_surface_size(surface)
        let scale = window.backingScaleFactor
        let cell = NSPoint(x: GhosttyTerminalPadding.x + CGFloat(grid.cell_width_px) / scale / 2,
                           y: view.bounds.height - GhosttyTerminalPadding.y - CGFloat(grid.cell_height_px) / scale / 2)
        let point = view.convert(cell, to: nil)
        let command = NSEvent.ModifierFlags.command.union(.init(rawValue: UInt(NX_DEVICELCMDKEYMASK)))
        func mouse(_ type: NSEvent.EventType, _ flags: NSEvent.ModifierFlags = []) throws -> NSEvent {
            try #require(NSEvent.mouseEvent(with: type, location: point, modifierFlags: flags, timestamp: 0,
                                           windowNumber: window.windowNumber, context: nil, eventNumber: 0,
                                           clickCount: 1, pressure: 1))
        }
        func modifiers(_ flags: NSEvent.ModifierFlags) throws -> NSEvent {
            try #require(NSEvent.keyEvent(with: .flagsChanged, location: point, modifierFlags: flags, timestamp: 0,
                                          windowNumber: window.windowNumber, context: nil, characters: "",
                                          charactersIgnoringModifiers: "", isARepeat: false, keyCode: 0x37))
        }
        view.mouseMoved(with: try mouse(.mouseMoved))
        #expect(view.linkFeedback.isHidden)
        view.flagsChanged(with: try modifiers(command))
        #expect(view.linkFeedback.stringValue == "https://example.com")
        view.cursorUpdate(with: try mouse(.mouseMoved, command))
        #expect(NSCursor.current == .pointingHand)
        view.flagsChanged(with: try modifiers([]))
        #expect(view.linkFeedback.isHidden)

        var opened: TerminalLink?
        view.onOpenLink = { link, _ in opened = link }
        view.mouseDown(with: try mouse(.leftMouseDown, command))
        view.mouseUp(with: try mouse(.leftMouseUp, command))
        let clickDeadline = ContinuousClock.now + .seconds(2)
        while opened == nil {
            try #require(ContinuousClock.now < clickDeadline, "Ghostty did not deliver the OSC 8 click.")
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(opened == .web(try #require(URL(string: "https://example.com"))))
    }

    @Test("Host Copy, Paste and Find do not consume modified TUI shortcuts")
    func shortcutOwnership() {
        for (key, shortcut) in [("c", GhosttyTerminalShortcut.copy), ("v", .paste), ("f", .find)] {
            #expect(GhosttyTerminalShortcut(characters: key, modifiers: .command) == shortcut)
            #expect(GhosttyTerminalShortcut(characters: key.uppercased(), modifiers: [.command, .capsLock]) == shortcut)
            for extra in [NSEvent.ModifierFlags.shift, .control, .option] {
                #expect(GhosttyTerminalShortcut(characters: key, modifiers: [.command, extra]) == nil)
            }
            #expect(GhosttyTerminalShortcut(characters: key, modifiers: .control) == nil)
            #expect(GhosttyTerminalShortcut(characters: key, modifiers: []) == nil)
        }
        #expect(GhosttyTerminalShortcut(characters: "t", modifiers: .command) == nil)
    }

    @Test("⌘` and ⌘⇧` leave the terminal to cycle windows; ⌃` and plain ` stay with it")
    func windowCycleBelongsToAppKit() {
        #expect(GhosttyTerminalShortcut(characters: "`", modifiers: .command) == .cycleWindows)
        #expect(GhosttyTerminalShortcut(characters: "~", modifiers: [.command, .shift]) == .cycleWindows)
        #expect(GhosttyTerminalShortcut(characters: "`", modifiers: .control) == nil)
        #expect(GhosttyTerminalShortcut(characters: "`", modifiers: []) == nil)
        #expect(GhosttyTerminalShortcut(characters: "`", modifiers: [.command, .option]) == nil)
    }

    @Test("Copy notices stay with the originating window and retained view")
    @MainActor
    func copyNoticeOwnership() {
        let firstPool = GhosttySurfacePool()
        let secondPool = GhosttySurfacePool()
        let terminal = TerminalIdentity.shell("copy-notice", machineId: fixtureMachineId)
        let first = firstPool.view(for: terminal)
        _ = secondPool.view(for: terminal)
        for message in ["Copied 1 line", nil] {
            let notice = Notification(name: .ghosttyCopyConfirmation, object: first.programStatus.incarnation,
                                      userInfo: message.map { ["message": $0] })
            #expect(firstPool.ownsCopyConfirmation(notice, terminal: terminal))
            #expect(!secondPool.ownsCopyConfirmation(notice, terminal: terminal))
        }
        firstPool.release(terminal)
        _ = firstPool.view(for: terminal)
        let lateDismissal = Notification(name: .ghosttyCopyConfirmation, object: first.programStatus.incarnation)
        #expect(!firstPool.ownsCopyConfirmation(lateDismissal, terminal: terminal))
    }

    @Test("Copy, Find and hidden retained surfaces preserve text", .requiresDisplay)
    @MainActor
    func retainedFeedback() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let view = GhosttyMetalView(terminal: .shell("feedback-proof", machineId: "fixture"))
        view.workingDirectory = NSTemporaryDirectory()
        let fixture = ProcessInfo.processInfo.environment["LOOPFLOW_TERMINAL_FEEDBACK_INPUT"]
        view.command = fixture.flatMap {
            buildGhosttyShellCommand(argv: ["/bin/sh", "-c", "cat \"$1\"; exec /bin/cat", "feedback", $0], env: [:])
        }
            ?? "/bin/sh -c 'printf \"error alpha\\nerror 界 é\\nfeedback-ready\\n\"; exec /bin/cat'"
        let mount = GhosttyTerminalMount(terminal: view)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 480),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = mount
        defer { view.destroySurface(); window.contentView = nil }
        mount.layoutSubtreeIfNeeded()
        view.createSurface(manager: manager)
        let surface = try #require(view.surface)
        let deadline = ContinuousClock.now + .seconds(5)
        while ContinuousClock.now < deadline {
            if case .available(let text, _) = try view.readText(region: .screen, maxBytes: 16384),
               text.contains("feedback-ready") { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        var observations: [[String: Any]] = []
        func measure(_ scenario: String, _ action: () -> Void) {
            let state = observations.contains { $0["scenario"] as? String == scenario } ? "warm" : "first"
            let start = CACurrentMediaTime()
            action()
            ghostty_surface_draw(surface)
            observations.append(["scenario": scenario, "metric": "action_to_draw_call_ms",
                                 "state": state, "scale": window.backingScaleFactor,
                                 "milliseconds": (CACurrentMediaTime() - start) * 1000])
        }
        for _ in 0..<5 { measure("idle") {} }
        view.performBinding("select_all")
        var text = ghostty_text_s()
        var receipt: UInt64 = 0
        #expect(ghostty_surface_read_selection_receipt(surface, &text, &receipt))
        let original = String(decoding: UnsafeRawBufferPointer(start: text.text, count: Int(text.text_len)), as: UTF8.self)
        ghostty_surface_free_text(surface, &text)
        #expect(original.contains("feedback-ready"))
        #expect(original.contains("界"))
        #expect(window.makeFirstResponder(view))
        let copy = try #require(NSEvent.keyEvent(with: .keyDown, location: .zero,
            modifierFlags: .command, timestamp: 0, windowNumber: window.windowNumber, context: nil,
            characters: "c", charactersIgnoringModifiers: "c", isARepeat: false, keyCode: 8))
        for _ in 0..<5 {
            measure("copy") { #expect(view.performKeyEquivalent(with: copy)) }
            #expect(NSPasteboard.general.string(forType: .string) == original)
        }
        #expect(ghostty_surface_read_selection_receipt(surface, &text, &receipt))
        ghostty_surface_free_text(surface, &text)
        #expect(ghostty_surface_confirm_copy(surface, receipt))
        view.performBinding("select_all")
        #expect(!ghostty_surface_confirm_copy(surface, receipt))
        view.searchBar.open(needle: "error")
        for _ in 0..<5 { measure("find") { view.performBinding("navigate_search:next") } }
        view.performBinding("end_search")
        view.searchBar.end()
        for _ in 0..<5 {
            measure("retained-return") {
                window.contentView = NSView()
                window.contentView = mount
                mount.layoutSubtreeIfNeeded()
            }
            #expect(view.surface == surface)
        }
        for _ in 0..<5 {
            measure("typing") { view.insertText("unsent-draft", replacementRange: NSRange(location: NSNotFound, length: 0)) }
        }
        ghostty_surface_set_feedback(surface, true, true)
        ghostty_surface_set_feedback(surface, false, true)
        #expect(!ghostty_surface_confirm_copy(surface, receipt))
        if let output = ProcessInfo.processInfo.environment["LOOPFLOW_TERMINAL_FEEDBACK_OUTPUT"] {
            let data = try JSONSerialization.data(withJSONObject: observations, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: URL(fileURLWithPath: output))
        }
    }
}
#endif
