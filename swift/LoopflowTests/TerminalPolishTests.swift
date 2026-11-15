#if os(macOS)
import Foundation
import AppKit
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Terminal polish")
struct TerminalPolishTests {
    #if canImport(GhosttyKit)
    @Test("Connection feedback never changes terminal geometry")
    @MainActor
    func connectionFeedbackKeepsViewport() {
        let terminal = GhosttyMetalView(terminal: .shell("geometry", machineId: "fixture"))
        let mount = GhosttyTerminalMount(terminal: terminal)
        mount.frame = NSRect(x: 0, y: 0, width: 800, height: 600)
        mount.addSubview(terminal)
        mount.addSubview(terminal.searchBar)
        mount.addSubview(terminal.connectionBar)
        for searchVisible in [false, true] {
            terminal.searchBar.isHidden = !searchVisible
            terminal.connectionBar.isHidden = true
            mount.layout()
            let frame = terminal.frame
            terminal.connectionBar.isHidden = false
            mount.layout()
            #expect(terminal.frame == frame)
            #expect(mount.bounds.contains(terminal.connectionBar.frame))
            #expect(terminal.connectionBar.frame.maxY <= terminal.searchBar.frame.minY)
            terminal.connectionBar.isHidden = true
            mount.layout()
            #expect(terminal.frame == frame)
        }
    }

    @Test("An unused terminal releases without constructing Find during deinitialization")
    @MainActor
    func unusedTerminalRelease() {
        weak var released: GhosttyMetalView?
        autoreleasepool {
            let terminal = GhosttyMetalView(terminal: .shell("unused", machineId: "fixture"))
            released = terminal
        }
        #expect(released == nil)
    }
    #endif

    @Test("Copy confirmation counts empty lines and ignores the final line ending")
    func copyConfirmation() {
        #expect(terminalCopyMessage("") == nil)
        #expect(terminalCopyMessage("hello\n") == "Copied 1 line")
        #expect(terminalCopyMessage("hello\n\nworld\n") == "Copied 3 lines")
        #expect(terminalCopyMessage("hello\r\nworld") == "Copied 2 lines")
        #expect(terminalCopyMessage("hello\r\nworld\r\n") == "Copied 2 lines")
    }

    @Test("Find distinguishes pending, empty and selected results")
    func findResults() {
        #expect(TerminalSearchResults().label == "Searching…")
        #expect(TerminalSearchResults(total: 0, selected: nil).label == "No matches")
        #expect(TerminalSearchResults(total: 4, selected: nil).label == "4 matches")
        #expect(TerminalSearchResults(total: 4, selected: 2).label == "3 of 4")
        // Total and selected arrive separately; never display an impossible index.
        #expect(TerminalSearchResults(total: 1, selected: 2).label == "1 match")
    }
}
#endif
