#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

@Suite("Unified workspace navigation proof")
@MainActor
struct WorkspaceNavigationProofTests {
#if canImport(GhosttyKit)
    @Test("A hidden retained terminal relinquishes focus and keeps its unfinished PTY input")
    func hiddenTerminalPreservesDraft() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        try #require(manager.state == .ready)
        let pool = GhosttySurfacePool()
        let identity = TerminalIdentity.session("navigation-draft-proof")
        let terminal = pool.view(for: identity)
        let window = NSWindow(
            contentRect: CGRect(x: 0, y: 0, width: 500, height: 300),
            styleMask: [.titled], backing: .buffered, defer: false
        )
        let view = GhosttyTerminalView(
            workingDirectory: NSTemporaryDirectory(),
            argv: ["/bin/cat"],
            terminal: identity, surfacePool: pool, isFocused: true
        )
        terminal.setFrameSize(window.contentLayoutRect.size)
        terminal.workingDirectory = NSTemporaryDirectory()
        terminal.command = buildGhosttyShellCommand(argv: view.argv, env: [:])
        terminal.createSurface(manager: manager)
        let surface = try #require(terminal.surface)
        let host = NSHostingView(rootView: view.disabled(false))
        host.frame = window.contentLayoutRect
        window.contentView = host
        defer { pool.release(identity) }
        host.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try #require(terminal.surface == surface)
        #expect(window.makeFirstResponder(terminal))
        let draft = "retained-draft"
        draft.withCString { ghostty_surface_text(surface, $0, UInt(draft.utf8.count)) }
        let search = NSTextField(frame: CGRect(x: 0, y: 0, width: 200, height: 24))
        host.addSubview(search)
        #expect(window.makeFirstResponder(search))
        let editor = try #require(search.currentEditor())
        window.setContentSize(CGSize(width: 520, height: 300))
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(50))
        #expect(window.firstResponder === editor)
        search.removeFromSuperview()
        pool.focus(identity)
        #expect(window.firstResponder === terminal)
        host.rootView = view.disabled(true)
        window.layoutIfNeeded()
        let hiddenDeadline = ContinuousClock.now + .seconds(3)
        while window.firstResponder === terminal, ContinuousClock.now < hiddenDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(window.firstResponder !== terminal)
        #expect(terminal.surface == surface)
        host.rootView = view.disabled(false)
        window.layoutIfNeeded()
        let focusDeadline = ContinuousClock.now + .seconds(3)
        while window.firstResponder !== terminal, ContinuousClock.now < focusDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(window.firstResponder === terminal)
        #expect(pool.view(for: identity) === terminal)
        "\n".withCString { ghostty_surface_text(surface, $0, 1) }
        let outputDeadline = ContinuousClock.now + .seconds(3)
        while _terminalText(surface).components(separatedBy: draft).count < 3, ContinuousClock.now < outputDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(surface).components(separatedBy: draft).count == 3)
    }

    private func _terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
            rectangle: false
        )
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
#endif

}
#endif
