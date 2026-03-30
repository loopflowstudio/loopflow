import Testing
@testable import LoopflowCore

@Suite("AppPreferences")
struct AppPreferencesTests {
    @Test("Ghostty is the first terminal option")
    func ghosttyLeadsTerminalOptions() {
        #expect(TerminalApp.defaultExternal == .ghostty)
        #expect(TerminalApp.allCases.first == TerminalApp.defaultExternal)
        #expect(TerminalApp.ghostty.displayName == "Ghostty")
    }
}
