#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Foundation
import GhosttyKit
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite(.requiresDisplay, .serialized)
@MainActor
struct SessionConnectionProofTests {
    @Test("A mounted Session keeps its screen and keyboard input across server loss")
    func nativeOpeningAndRecovery() async throws {
        _ = NSApplication.shared
        try await withPreparedSession(provider: #"""
        #!/bin/bash
        if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
        printf '%s\n' "$$" >> "$HOME/launches"
        stty echo < /dev/tty
        printf 'session-proof-ready\n'
        while IFS= read -r -t 20 line < /dev/tty; do
          [ "$line" = quit ] && exit 0
          printf '%s\n' "$line" >> "$HOME/answers"
          printf 'received:%s\n' "$line"
        done
        """#) { lf, server, record in
            let manager = GhosttyManager.shared
            manager.initialize()
            try #require(manager.state == .ready)
            let view = GhosttyMetalView(terminal: .session(record.id, machineId: fixtureMachineId))
            view.workingDirectory = lf.cwd.path
            view.command = buildGhosttyShellCommand(argv: record.openArgv, env: lf.environment)
            let mount = GhosttyTerminalMount(terminal: view)
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 480),
                                  styleMask: [.titled], backing: .buffered, defer: false)
            window.contentView = mount
            defer { view.destroySurface(); window.contentView = nil }
            mount.layoutSubtreeIfNeeded()
            view.createSurface(manager: manager)
            let surface = try #require(view.surface)
            try #require(window.makeFirstResponder(view))
            try await waitForSession(view) { view.connectionAcceptsInput && terminalViewport(view).contains("session-proof-ready") }
            let frame = view.frame
            try type("before\r", into: view, window: window)
            try await waitForSession(view) { terminalViewport(view).contains("received:before") }
            await server.stop()
            try await waitForSession(view) { !view.connectionAcceptsInput }
            #expect(terminalViewport(view).contains("received:before"))
            #expect(view.frame == frame)
            try type("discarded\r", into: view, window: window)
            _ = try await server.status()
            try await waitForSession(view) { view.connectionAcceptsInput }
            #expect(view.surface == surface)
            #expect(view.frame == frame)
            try type("after\r", into: view, window: window)
            try await waitForSession(view) { terminalViewport(view).contains("received:after") }
            #expect(try String(contentsOf: lf.cwd.appendingPathComponent("answers"), encoding: .utf8) == "before\nafter\n")
            #expect(try String(contentsOf: lf.cwd.appendingPathComponent("launches"), encoding: .utf8).split(separator: "\n").count == 1)
            try type("quit\r", into: view, window: window)
            try await waitForSession(view) { view.childExited || ghostty_surface_process_exited(surface) }
        }
    }

    private func type(_ text: String, into view: GhosttyMetalView, window: NSWindow) throws {
        for character in text {
            let value = String(character)
            let key = try terminalKeyEvent(value, character == "\r" ? 36 : 0, windowNumber: window.windowNumber)
            view.keyDown(with: key)
        }
    }
}
#endif
