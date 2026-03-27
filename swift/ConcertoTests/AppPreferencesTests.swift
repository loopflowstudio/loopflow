import Testing
@testable import LoopflowCore

@Suite("AppPreferences")
struct AppPreferencesTests {
    @Test("Ghostty is the first terminal option")
    func ghosttyLeadsTerminalOptions() {
        #expect(TerminalApp.allCases.first == .ghostty)
        #expect(TerminalApp.ghostty.displayName == "Ghostty")
    }
}
