#if os(macOS)
import Testing
@testable import LoopflowMac

@Suite("Terminal polish")
struct TerminalPolishTests {
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
