#if os(macOS)
import Foundation
import Testing
#if canImport(GhosttyKit)
import GhosttyKit
#endif
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
        let store = AppearanceStore(defaults: defaults, systemDark: false, publish: { _, _ in
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

    #if canImport(GhosttyKit)
    @Test func compiledOverlayOverridesPersonalColors() throws {
        try #require(GhosttyManager.libraryReady)
        let baseline = try #require(ghostty_config_new())
        defer { ghostty_config_free(baseline) }
        try GhosttyThemeConfiguration.load("""
        background=112233
        foreground=445566
        font-size=19
        \((0..<256).map { "palette=\($0)=123456" }.joined(separator: "\n"))
        cursor-color=123456
        cursor-text=654321
        selection-background=123456
        selection-foreground=654321
        search-background=123456
        search-foreground=654321
        search-selected-background=123456
        search-selected-foreground=654321
        theme=light:/unread-light,dark:/unread-dark

        """, into: baseline)
        try GhosttyThemeConfiguration.load("theme=\n", into: baseline)
        ghostty_config_finalize(baseline)
        let darkImport = try imported("""
        background=171924
        foreground=CAD2EF
        cursor-color=AABBCC
        cursor-text=222222
        selection-background=304060
        selection-foreground=FFFFFF
        """).definition
        for theme in ThemeDefinition.builtins + [darkImport, try imported("background=FFF4DF\nforeground=34291F").definition] {
            let candidate = try GhosttyThemeConfiguration.candidate(theme, baseline: baseline)
            defer { ghostty_config_free(candidate) }
            try GhosttyThemeConfiguration.validate(theme, config: candidate)
            var size: Float = 0
            #expect(ghostty_config_get(candidate, &size, "font-size", 9))
            #expect(size == 19)
            #expect(ghostty_config_diagnostics_count(candidate) == 0)

            let expected = ["cursor-color": theme.terminal.cursor, "cursor-text": theme.terminal.cursorText,
                            "selection-background": theme.terminal.selectionBackground,
                            "selection-foreground": theme.terminal.selectionForeground,
                            "search-background": theme.terminal.selectionBackground,
                            "search-foreground": theme.terminal.selectionForeground,
                            "search-selected-background": theme.terminal.selectionBackground,
                            "search-selected-foreground": theme.terminal.selectionForeground]
            for (key, expectedColor) in expected {
                var color = ghostty_config_color_s()
                #expect(key.withCString { ghostty_config_get(candidate, &color, $0, UInt(key.utf8.count)) })
                #expect(ThemeRGB(UInt32(color.r) << 16 | UInt32(color.g) << 8 | UInt32(color.b)) == expectedColor)
            }
        }
    }

    @Test(arguments: [
        "background", "foreground", "cursor-color", "cursor-text",
        "selection-background", "selection-foreground", "search-background", "search-foreground",
        "search-selected-background", "search-selected-foreground", "palette",
    ])
    func validationRejectsEachConflictingCompiledColor(key: String) throws {
        try #require(GhosttyManager.libraryReady)
        let baseline = try #require(ghostty_config_new())
        defer { ghostty_config_free(baseline) }
        let candidate = try GhosttyThemeConfiguration.candidate(.dark, baseline: baseline)
        defer { ghostty_config_free(candidate) }
        let value = key == "palette" ? "255=123456" : "123456"
        try GhosttyThemeConfiguration.load("\(key)=\(value)\n", into: candidate)
        ghostty_config_finalize(candidate)
        #expect(throws: ThemeError.self) { try GhosttyThemeConfiguration.validate(.dark, config: candidate) }
    }

    @Test(arguments: ["cell-foreground", "cell-background", ""])
    func unresolvedCompiledColorCannotPassValidation(value: String) throws {
        try #require(GhosttyManager.libraryReady)
        let baseline = try #require(ghostty_config_new())
        defer { ghostty_config_free(baseline) }
        let candidate = try GhosttyThemeConfiguration.candidate(.dark, baseline: baseline)
        defer { ghostty_config_free(candidate) }
        try GhosttyThemeConfiguration.load("cursor-color=\(value)\n", into: candidate)
        ghostty_config_finalize(candidate)
        #expect(throws: ThemeError.self) { try GhosttyThemeConfiguration.validate(.dark, config: candidate) }
    }
    #endif

    @Test func dimTextWarnsWithoutChangingOrRejectingTheTheme() throws {
        let theme = try imported("background=FFFFFF\nforeground=000000\npalette=1=000000")
        #expect(theme.terminal.readabilityErrors.isEmpty)
        #expect(!theme.terminal.ansiWarnings.contains { $0.hasPrefix("ANSI 1:") })
        #expect(theme.terminal.dimWarnings.contains { $0.hasPrefix("Dim Default:") })
        #expect(theme.terminal.dimWarnings.contains { $0.hasPrefix("Dim ANSI 1:") })
        #expect(theme.terminal.foreground.hex == 0)
        #expect(theme.terminal.indexed[1].hex == 0)
        let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: source) }
        try Data("background=FFFFFF\nforeground=000000".utf8).write(to: source)
        let store = AppearanceStore(defaults: defaults(), systemDark: false)
        store.beginPreview(); store.importTheme(source); store.apply()
        #expect(store.notice == nil)
        #expect(store.effectiveTheme.terminal.foreground.hex == 0)
        #expect(store.imports.count == 1)
    }
}
#endif
