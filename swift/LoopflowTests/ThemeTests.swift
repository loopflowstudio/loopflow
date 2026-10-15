#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
import GhosttyKit
@testable import Loopflow
@testable import LoopflowMac

@Suite("Whole-app themes", .serialized)
@MainActor
struct ThemeTests {
    private func defaults() -> UserDefaults { UserDefaults(suiteName: "theme-test-\(UUID().uuidString)")! }
    private func imported(_ text: String = "background = #171924\nforeground = #CAD2EF") throws -> ImportedTheme {
        try GhosttyThemeImport.parse(Data(text.utf8), name: "Personal")
    }

    @Test func migrationPreviewCancelApplyAndRestore() throws {
        let defaults = defaults()
        defaults.set("dark", forKey: "appearanceMode")
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(store.selection == "dark")
        #expect(defaults.object(forKey: "appearanceMode") == nil)
        #expect(store.effectiveTheme.terminal == ThemeDefinition.dark.terminal)
        store.beginPreview(); store.preview("light")
        #expect(store.effectiveTheme.id == "light")
        #expect(AppearanceStore(defaults: defaults, systemDark: false).selection == "dark")
        store.cancel()
        #expect(store.effectiveTheme.id == "dark")
        store.beginPreview(); store.preview("light"); store.apply()
        #expect(AppearanceStore(defaults: defaults, systemDark: true).effectiveTheme.id == "light")
        store.apply("system"); store.setSystemDark(true)
        #expect(store.effectiveTheme.id == "dark")
        store.setSystemDark(false)
        #expect(store.effectiveTheme.id == "light")
    }

    @Test func snapshotImportAndFailedPublicationPreservePreferences() throws {
        let defaults = defaults()
        var reject = false
        let store = AppearanceStore(defaults: defaults, systemDark: false, publish: { _ in
            if reject { throw ThemeError("Allocation failed") }
        })
        store.apply("dark")
        let saved = defaults.data(forKey: "appearancePreferences")
        reject = true; store.beginPreview(); store.preview("light"); store.apply()
        #expect(store.effectiveTheme.id == "dark")
        #expect(defaults.data(forKey: "appearancePreferences") == saved)
        #expect(store.notice == "Allocation failed")
        reject = false; store.cancel()
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: path) }
        try Data("background=171924\nforeground=CAD2EF\ncommand=touch /never\n".utf8).write(to: path)
        store.beginPreview(); store.importTheme(path)
        let id = store.selectedID
        #expect(store.imports.isEmpty)
        #expect(store.diagnostics.contains { $0.contains("omitted command") })
        store.apply()
        try FileManager.default.removeItem(at: path)
        let restored = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(restored.selection == id)
        #expect(restored.effectiveTheme.terminal.background.hex == 0x171924)
        #expect(restored.imports.count == 1)
    }

    @Test func corruptAndUnavailablePreferencesRemainRecoverable() throws {
        for data in [Data("broken".utf8), Data(#"{"version":99,"selection":"dark","imports":[]}"#.utf8),
                     Data(#"{"version":1,"selection":"missing","imports":[]}"#.utf8)] {
            let defaults = defaults(); defaults.set(data, forKey: "appearancePreferences")
            let store = AppearanceStore(defaults: defaults, systemDark: true)
            #expect(store.effectiveTheme.id == "dark")
            #expect(store.notice != nil)
            #expect(defaults.data(forKey: "appearancePreferences") == data)
            store.apply("light")
            #expect(AppearanceStore(defaults: defaults, systemDark: true).notice == nil)
        }
    }

    @Test func boundedImportRetainsColorsAndReportsOmissions() throws {
        let theme = try imported("""
        # concrete RGB snapshot
        background=171924
        foreground=CAD2EF
        palette=8=101010
        palette=8=#121212
        palette=255=ABCDEF
        font-size=80
        config-file=/never
        theme=light:foo,dark:bar
        """)
        #expect(theme.terminal.indexed.count == 256)
        #expect(theme.terminal.indexed[8].hex == 0x121212)
        #expect(theme.terminal.indexed[255].hex == 0xABCDEF)
        #expect(theme.terminal.ansiWarnings.contains { $0.hasPrefix("ANSI 8:") })
        #expect(theme.diagnostics.filter { $0.contains("omitted") }.count == 3)
        #expect(theme.diagnostics.contains { $0.contains("repeated palette 8") })
        #expect(!theme.definition.ghosttyOverlay.contains("/never"))
        #expect(try imported().id != imported().id)
        for source in ["foreground=ffffff", "background=red\nforeground=ffffff", "background=000000\nforeground=ffffff\npalette=256=ffffff", "background=000000\nforeground=ffffff\npalette=-1=ffffff", "background=000000\nforeground='ffffff'"] {
            #expect(throws: ThemeError.self) { try imported(source) }
        }
        #expect(throws: ThemeError.self) { try GhosttyThemeImport.parse(Data([0xFF]), name: "Invalid") }
        #expect(throws: ThemeError.self) { try GhosttyThemeImport.parse(Data(repeating: 32, count: 262145), name: "Huge") }
        #expect(try imported("background=777777\nforeground=888888").terminal.readabilityErrors.first?.contains("Default text") == true)
        #expect(try imported("background=000000\nforeground=ffffff\nselection-background=888888\nselection-foreground=999999").terminal.readabilityErrors.first?.contains("Selection text") == true)
    }

    @Test func compiledOverlayOverridesPersonalColors() throws {
        try #require(GhosttyManager.libraryReady)
        let baseline = try #require(ghostty_config_new())
        defer { ghostty_config_free(baseline) }
        try GhosttyThemeConfiguration.load("background=112233\nforeground=445566\nfont-size=19\npalette=8=123456\n", into: baseline)
        ghostty_config_finalize(baseline)
        for theme in ThemeDefinition.builtins + [try imported().definition] {
            let candidate = try GhosttyThemeConfiguration.candidate(theme, baseline: baseline)
            defer { ghostty_config_free(candidate) }
            try GhosttyThemeConfiguration.validate(theme, config: candidate)
            var size: Float = 0
            #expect(ghostty_config_get(candidate, &size, "font-size", 9))
            #expect(size == 19)
            #expect(ghostty_config_diagnostics_count(candidate) == 0)
        }
    }
}
#endif
