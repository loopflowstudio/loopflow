#if os(macOS)
import AppKit
import Foundation
import Testing
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

extension ThemePaths {
    /// A private `~/.lf` and Ghostty theme folder for one test.
    static func temporary() -> Self {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("loo488-\(UUID().uuidString)", isDirectory: true)
        return Self(config: root.appendingPathComponent("lf/config.yaml"),
                    themes: root.appendingPathComponent("lf/themes", isDirectory: true),
                    ghostty: root.appendingPathComponent("ghostty/themes", isDirectory: true))
    }

    func write(_ text: String, theme name: String, ghostty: Bool = false) throws {
        let directory = ghostty ? self.ghostty! : themes
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data(text.utf8).write(to: directory.appendingPathComponent(name))
    }

    func configText() -> String? { try? String(contentsOf: config, encoding: .utf8) }

    func writeConfig(_ text: String) throws {
        try FileManager.default.createDirectory(at: config.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: config)
    }
}

@MainActor
func withThemeServer(_ run: @MainActor (ServerLocation) async throws -> Void) async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("theme-server-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let server = LocalServerFixture(try BuiltLf(home: root))
    do {
        try await run(server.location())
        await server.stop()
    } catch {
        await server.stop()
        throw error
    }
}

extension AppearanceStore {
    static func loaded(paths: ThemePaths, systemDark: Bool, location: ServerLocation) async -> AppearanceStore {
        let store = AppearanceStore(paths: paths, systemDark: systemDark, location: location)
        await store.reload()
        return store
    }
}

@Suite("Whole-app themes", .serialized)
@MainActor
struct ThemeTests {
    private func imported(_ text: String = "background = #171924\nforeground = #CAD2EF") throws -> ThemeFile {
        try ThemeFile.parse(Data(text.utf8), name: "Personal")
    }
    private func file(_ text: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            .appendingPathComponent("Personal")
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: url)
        return url
    }

    @Test func loopflowUsesNeutralWorkSelection() {
        let light = ThemeDefinition.light.palette.definition
        #expect(ThemeDefinition.light.name == "Loopflow Light")
        #expect(light.background.hex == 0xFAF8F5)
        #expect(light.surface.hex == 0xFFFDF9)
        #expect(light.surfaceMuted.hex == 0xF3EEE7)
        #expect(light.surfaceElement.hex == 0xECE6DD)
        #expect(light.text.hex == 0x2A2624)
        #expect(light.textSecondary.hex == 0x5F5752)
        #expect(light.accent.hex == 0x722F37)
        #expect(light.accentHover.hex == 0x5E2630)
        #expect(light.emphasisInk.hex == 0x722F37)
        #expect(light.workSelectedFill.hex == 0xE1D9CE)
        #expect(light.workSelectedText.hex == 0x2A2624)
        let dark = ThemeDefinition.dark.palette.definition
        #expect(ThemeDefinition.dark.name == "Loopflow Dark")
        #expect(dark.background.hex == 0x2B2D30)
        #expect(dark.surface.hex == 0x323437)
        #expect(dark.accent.hex == 0x8A3A44)
        #expect(dark.emphasisInk.hex == 0xEBA8AC)
        #expect(dark.workSelectedFill.hex == 0x5A5C5F)
        // One burgundy bar tops both defaults.
        #expect(light.titlebar.hex == 0x722F37)
        #expect(dark.titlebar == light.titlebar)
        // Loopflow's authored design is independent of the import conversion.
        #expect(light != GhosttyThemeMapping.palette(ThemeDefinition.light.terminal))
        #expect(dark != GhosttyThemeMapping.palette(ThemeDefinition.dark.terminal))
    }

    @Test func catalogueIsThirteenFamiliesWithALightAndADarkTheme() throws {
        let light = ThemeDefinition.builtins.filter { !$0.isDark }.map(\.name)
        let dark = ThemeDefinition.builtins.filter(\.isDark).map(\.name)
        #expect(light == ["Loopflow Light", "Arcade Day", "Contrast Light", "Fig Leaf", "Fjord Light", "Folio", "Graphite Light",
                          "Harbor Light", "Signal Light", "Sorbet Day", "Tannin Light", "Terracotta Light", "Verdigris Light"])
        #expect(dark == ["Loopflow Dark", "Arcade", "Contrast", "Fig", "Fjord", "Folio Night", "Graphite",
                         "Harbor", "Signal", "Sorbet", "Tannin", "Terracotta", "Verdigris"])
        for theme in ThemeDefinition.builtins {
            let palette = theme.palette.definition
            // Headings are the text or the accent ink, never a third color.
            #expect(palette.emphasisInk == palette.text || palette.emphasisInk == palette.accentInk, "\(theme.name)")
            #expect(palette.accent != palette.accentHover)
            #expect(palette.accentHover != palette.accentPressed)
            if theme.isDark { #expect(palette.surfaceElement.luminance > palette.background.luminance) }
            #expect(theme.readabilityErrors.isEmpty, "\(theme.name)")
        }
    }

    @Test func builtinFilesAreCompleteAndRoundTrip() throws {
        for theme in ThemeDefinition.builtins {
            let text = ThemeFile.text(theme)
            let restored = try ThemeFile.parse(Data(text.utf8), name: theme.name, complete: true).theme
            #expect(restored.palette.definition == theme.palette.definition)
            #expect(restored.terminal == theme.terminal)
            let lines = text.split(separator: "\n").filter { !$0.hasPrefix("#") }
            for index in [0, 7, lines.count - 5, lines.count - 1] {
                var incomplete = lines
                incomplete.remove(at: index)
                #expect(throws: ThemeError.self) {
                    try ThemeFile.parse(Data(incomplete.joined(separator: "\n").utf8), name: theme.name, complete: true)
                }
            }
        }
    }

    @Test func ghosttyMappingUsesAnsiAndSelectionBeyondBackgroundAndForeground() throws {
        let first = try imported("background=ffffff\nforeground=000000\npalette=4=000088\npalette=1=880000\nselection-background=222222\nselection-foreground=ffffff").theme.palette.definition
        let second = try imported("background=ffffff\nforeground=000000\npalette=4=006600\npalette=1=880066\nselection-background=444444\nselection-foreground=ffffff").theme.palette.definition
        #expect(first.background == second.background)
        #expect(first.accent != second.accent)
        #expect(first.emphasisInk == second.emphasisInk)
        #expect(first.accentInk != second.accentInk)
        #expect(first.workingTone != second.workingTone)
        #expect(first.failureTone != second.failureTone)
        #expect(first.workSelectedFill != second.workSelectedFill)
        #expect(first.readabilityErrors.isEmpty)
        #expect(second.readabilityErrors.isEmpty)
    }

    @Test func titleBarFollowsTheFilesOwnAccentUntilItIsStated() throws {
        let ground = "background=171924\nforeground=CAD2EF"
        // Dark: the accent mixed into the ground, not the bar ANSI 4 would give.
        let dark = try imported("\(ground)\nloopflow-accent=#224466").theme.palette.definition
        #expect(dark.titlebar.hex == 0x1B283B)
        #expect(dark.titlebar != (try imported(ground).theme.palette.definition.titlebar))
        // Light: the accent itself, lightened here until black text's second level reads.
        let light = try imported("background=ffffff\nforeground=000000\nloopflow-accent=#C06000").theme.palette.definition
        #expect(light.titlebar.hex == 0xCB7B2B)
        #expect(!light.readabilityErrors.contains { $0.contains("title bar") })
        // An appearance stated against the ground's own decides the mix.
        let asLight = try imported("\(ground)\nloopflow-accent=#224466\nloopflow-appearance=light").theme.palette.definition
        #expect(asLight.titlebar.hex == 0x224466)
        let stated = try imported("\(ground)\nloopflow-accent=#224466\nloopflow-titlebar=#102030").theme.palette.definition
        #expect(stated.titlebar.hex == 0x102030)
        #expect(try imported("\(ground)\nloopflow-titlebar=#808080").theme.readabilityErrors.contains { $0.contains("title bar") })
    }

    @Test func loopflowKeysOverrideDerivedAppRoles() throws {
        let derived = try imported().theme.palette.definition
        let theme = try imported("background=171924\nforeground=CAD2EF\nloopflow-accent = #224466\nloopflow-working-ink=#99CCFF\nloopflow-selection-opacity=0.5").theme
        #expect(theme.palette.definition.accent.hex == 0x224466)
        #expect(theme.palette.definition.workingTone.inkRGB.hex == 0x99CCFF)
        #expect(theme.palette.definition.selectionFill.opacity == 0.5)
        #expect(theme.palette.definition.surface == derived.surface)
        #expect(theme.isDark)
        #expect(try imported("background=171924\nforeground=CAD2EF\nloopflow-appearance=light").theme.isDark == false)
        #expect(throws: ThemeError.self) { try imported("background=171924\nforeground=CAD2EF\nloopflow-accent=blue") }
    }

    @Test func applyChangesOnlyTheThemeAndCancelLeavesTheFileUntouched() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            try paths.writeConfig("agent: claude\n# keep me\ntheme: Graphite\nyolo: true\n")
            let store = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(store.selection.mode == .dark)
            #expect(store.effectiveTheme.name == "Graphite")
            store.beginPreview(); store.previewMode(.system); store.showPicker(.light); store.previewTheme("Folio")
            #expect(paths.configText() == "agent: claude\n# keep me\ntheme: Graphite\nyolo: true\n")
            store.cancel()
            #expect(paths.configText() == "agent: claude\n# keep me\ntheme: Graphite\nyolo: true\n")
            #expect(store.effectiveTheme.name == "Graphite")
            store.beginPreview(); store.previewMode(.system); store.showPicker(.light); store.previewTheme("Folio"); await store.apply()
            let savedTheme1 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme1 == "light:Folio,dark:Graphite")
            #expect(paths.configText()?.split(separator: "\n").filter { !$0.hasPrefix("theme:") } == ["agent: claude", "# keep me", "yolo: true"])
            store.beginPreview(); store.previewMode(.light); await store.apply()
            let savedTheme2 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme2 == "Folio")
            #expect(paths.configText()?.split(separator: "\n").filter { !$0.hasPrefix("theme:") } == ["agent: claude", "# keep me", "yolo: true"])
            // Returning to System restores the dark side remembered in this session.
            await store.applyMode(.system)
            let savedTheme3 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme3 == "light:Folio,dark:Graphite")
            await store.applyMode(.dark)
            let savedTheme4 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme4 == "Graphite")
            let relaunched = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(relaunched.selection.mode == .dark)
            #expect(relaunched.effectiveTheme.name == "Graphite")
            await relaunched.applyMode(.system)
            let savedTheme5 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme5 == "light:Loopflow Light,dark:Graphite")
        }
    }

    @Test func absentConfigMeansLoopflowLightAndDarkAndIsCreatedOnApply() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            let store = await AppearanceStore.loaded(paths: paths, systemDark: true, location: location)
            #expect(store.selection == AppearanceSelection())
            #expect(store.effectiveTheme.name == "Loopflow Dark")
            #expect(paths.configText() == nil)
            await store.applyMode(.light)
            let savedTheme6 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme6 == "Loopflow Light")
        }
    }

    @Test func outsideEditsReloadAndUnknownNamesKeepTheLastGoodTheme() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            try paths.writeConfig("theme: light:Folio,dark:Harbor\n")
            let store = await AppearanceStore.loaded(paths: paths, systemDark: true, location: location)
            #expect(store.effectiveTheme.name == "Harbor")
            try paths.writeConfig("theme: 'light:Folio,dark:Contrast' # by hand\n")
            await store.reload()
            #expect(store.effectiveTheme.name == "Contrast")
            #expect(store.notice == nil)
            try paths.writeConfig("theme: Onxy\n")
            await store.reload()
            #expect(store.effectiveTheme.name == "Contrast")
            #expect(store.notice?.contains("No theme named “Onxy”") == true)
            #expect(store.notice?.contains(paths.config.path) == true)
            try paths.writeConfig("theme: Sorbet Day\n")
            await store.reload()
            #expect(store.effectiveTheme.name == "Sorbet Day")
            #expect(store.selection.mode == .light)
            #expect(store.notice == nil)
        }
    }

    @Test func aRemovedBuiltinShowsLoopflowsThemeForItsSideAndSaysWhy() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            try paths.writeConfig("theme: Gruvbox Dark\n")
            let store = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(store.effectiveTheme.name == "Loopflow Dark")
            #expect(store.selection.mode == .dark)
            #expect(store.notice?.contains("“Gruvbox Dark” is no longer built in, so Loopflow Dark is showing") == true)
            try paths.writeConfig("theme: light:Linen,dark:Harbor\n")
            await store.reload()
            #expect(store.selection.light == "Loopflow Light")
            #expect(store.selection.dark == "Harbor")
            #expect(store.notice?.contains("“Linen”") == true)
            // The file keeps its words until the person applies a choice.
            #expect(paths.configText()?.contains("light:Linen,dark:Harbor") == true)
        }
    }

    @Test func customizingABuiltinWritesYourCopyAndDeletingRevertsIt() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            let store = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            store.beginPreview(); store.previewTheme("Folio")
            #expect(store.selectedSource == .builtin)
            #expect(!store.canDelete)
            store.editTheme { palette, _ in palette.accent = ThemeRGB(0x224466) }
            #expect(store.selectedSource == .yours)
            #expect(store.deleteRevertsToBuiltin)
            #expect(store.effectiveTheme.palette.definition.accent.hex == 0x224466)
            #expect(!FileManager.default.fileExists(atPath: paths.themes.appendingPathComponent("Folio").path))
            await store.apply()
            let saved = try ThemeFile.read(paths.themes.appendingPathComponent("Folio"), complete: true).theme
            #expect(saved.palette.definition.accent.hex == 0x224466)
            let relaunched = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(relaunched.effectiveTheme.palette.definition.accent.hex == 0x224466)
            #expect(relaunched.entries(for: relaunched.picker).builtins.first { $0.name == "Folio" }?.source == .yours)
            relaunched.beginPreview(); relaunched.deleteTheme(); await relaunched.apply()
            #expect(!FileManager.default.fileExists(atPath: paths.themes.appendingPathComponent("Folio").path))
            #expect(relaunched.effectiveTheme.name == "Folio")
            #expect(relaunched.effectiveTheme.palette.definition == ThemeDefinition.builtins.first { $0.name == "Folio" }?.palette.definition)
        }
    }

    @Test func importFilesAThemeOnItsOwnSide() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            let store = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            store.beginPreview(); store.showPicker(.light)
            store.importTheme(try file("background=171924\nforeground=CAD2EF"))
            #expect(store.canApply)
            #expect(store.current.mode == .system)
            #expect(store.picker == .dark)
            #expect(store.entries(for: .dark).yours.map(\.name) == ["Personal"])
            #expect(store.entries(for: .light).yours.isEmpty)
            #expect(store.effectiveTheme.name == "Personal")
            await store.apply()
            let savedTheme7 = try await ThemeConfig.read(paths.config, location: location)
            #expect(savedTheme7 == "light:Loopflow Light,dark:Personal")
            let saved = try ThemeFile.read(paths.themes.appendingPathComponent("Personal"), complete: true).theme
            #expect(saved.terminal.background.hex == 0x171924)
            store.beginPreview(); store.showPicker(.dark); store.deleteTheme()
            #expect(store.current.dark == "Loopflow Dark")
            await store.apply()
            #expect(store.library.yours.isEmpty)
        }
    }

    @Test func ghosttyThemesResolveAfterYoursAndBuiltinsAndStayReadOnly() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            try paths.write("background=101010\nforeground=EEEEEE", theme: "Midnight", ghostty: true)
            try paths.write("background=FFFFFF\nforeground=000000", theme: "Folio", ghostty: true)
            try paths.writeConfig("theme: Midnight\n")
            let store = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(store.effectiveTheme.name == "Midnight")
            #expect(store.selectedSource == .ghostty)
            #expect(store.entries(for: store.picker).yours.map(\.name) == ["Midnight"])
            #expect(store.library.resolve("Folio")?.palette.definition == ThemeDefinition.builtins.first { $0.name == "Folio" }?.palette.definition)
            #expect(!store.canDelete)
            store.customize(); await store.apply()
            #expect(FileManager.default.fileExists(atPath: paths.themes.appendingPathComponent("Midnight").path))
            #expect(store.selectedSource == .yours)
        }
    }

    /// `waiting` once served warnings and availability alike. Your files keep
    /// those colors as the warning role; availability is never reinterpreted.
    @Test func yourRetiredWaitingRoleMigratesToWarning() throws {
        let paths = ThemePaths.temporary()
        try paths.write("background=FFF4DF\nforeground=34291F\nloopflow-waiting-ink = #7A4A00\nloopflow-waiting-fill=#FFF0D0",
                        theme: "Paper")
        try paths.write("background=101010\nforeground=EEEEEE\nloopflow-waiting-ink = #E0B050", theme: "Midnight", ghostty: true)
        let library = ThemeLibrary.load(paths)
        let paper = try #require(library.yours["Paper"]).palette.definition
        #expect(paper.warningTone.inkRGB == ThemeRGB(0x7A4A00))
        #expect(paper.warningTone.fillRGB == ThemeRGB(0xFFF0D0))
        #expect(paper.availableTone.inkRGB != ThemeRGB(0x7A4A00))
        let migrated = try String(contentsOf: paths.themes.appendingPathComponent("Paper"), encoding: .utf8)
        #expect(migrated.contains("loopflow-warning-ink = #7A4A00\nloopflow-warning-fill=#FFF0D0"))
        #expect(!migrated.contains("waiting"))
        // Ghostty's folder is not Loopflow's to rewrite; its retired key is reported, not applied.
        let ghostty = paths.ghostty!.appendingPathComponent("Midnight")
        #expect(try String(contentsOf: ghostty, encoding: .utf8).contains("loopflow-waiting-ink"))
        #expect(try ThemeFile.read(ghostty).diagnostics.contains("Line 3: loopflow-waiting-ink is now loopflow-warning-ink; not applied."))
    }

    @Test func invalidFilesAreReportedAndDoNotHideOtherThemes() async throws {
        try await withThemeServer { location in
            let paths = ThemePaths.temporary()
            try paths.write("background=red\nforeground=000000", theme: "Broken")
            try paths.write("background=FFF4DF\nforeground=34291F", theme: "Paper")
            let store = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(store.notice?.contains("Broken") == true)
            #expect(store.entries(for: store.picker).yours.map(\.name) == ["Paper"])
        }
    }

    @Test func unreadableEditsKeepTheLastAppearanceAndDisableApply() throws {
        let store = AppearanceStore(paths: .temporary(), systemDark: false)
        store.beginPreview(); store.previewTheme("Folio")
        store.editTheme { _, terminal in terminal.foreground = terminal.background }
        #expect(!store.canApply)
        #expect(store.draftErrors.first?.hasPrefix("Folio:") == true)
        #expect(store.effectiveTheme.terminal.foreground != store.effectiveTheme.terminal.background)
        #expect(store.notice?.contains("Default text") == true)
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
        #expect(theme.theme.terminal.indexed.count == 256)
        #expect(theme.theme.terminal.indexed[8].hex == 0x121212)
        #expect(theme.theme.terminal.indexed[255].hex == 0xABCDEF)
        #expect(theme.diagnostics.filter { $0.contains("omitted") }.count == 3)
        #expect(theme.diagnostics.contains { $0.contains("repeated palette 8") })
        #expect(!theme.theme.ghosttyOverlay.contains("/never"))
        for source in ["foreground=ffffff", "background=red\nforeground=ffffff", "background=000000\nforeground=ffffff\npalette=256=ffffff", "background=000000\nforeground=ffffff\npalette=-1=ffffff", "background=000000\nforeground='ffffff'"] {
            #expect(throws: ThemeError.self) { try imported(source) }
        }
        #expect(throws: ThemeError.self) { try ThemeFile.parse(Data([0xFF]), name: "Invalid") }
        #expect(throws: ThemeError.self) { try ThemeFile.parse(Data(repeating: 32, count: 262145), name: "Huge") }
        #expect(try imported("background=777777\nforeground=888888").theme.terminal.readabilityErrors.first?.contains("Default text") == true)
        #expect(try imported("background=000000\nforeground=ffffff\nselection-background=888888\nselection-foreground=999999").theme.terminal.readabilityErrors.first?.contains("Selection text") == true)
    }

    #if canImport(GhosttyKit)
    @Test func defaultConfigurationLoadsAfterInitialAppearance() throws {
        // This crosses the formerly crashing boundary without a surface or PTY.
        let manager = GhosttyManager.shared
        try manager.applyTheme(.light)
        manager.initialize()
        #expect(manager.state == .ready)
        try manager.applyTheme(.dark)
        #expect(manager.theme.name == "Loopflow Dark")
    }

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
        """).theme
        for theme in ThemeDefinition.builtins + [darkImport, try imported("background=FFF4DF\nforeground=34291F").theme] {
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

    @Test func weakDimTextKeepsTheThemeAsImported() async throws {
        try await withThemeServer { location in
            let theme = try imported("background=FFFFFF\nforeground=000000\npalette=1=000000").theme
            #expect(theme.terminal.readabilityErrors.isEmpty)
            #expect(theme.terminal.foreground.hex == 0)
            #expect(theme.terminal.indexed[1].hex == 0)
            let store = await AppearanceStore.loaded(paths: .temporary(), systemDark: false, location: location)
            store.beginPreview(); store.importTheme(try file("background=FFFFFF\nforeground=000000")); await store.apply()
            #expect(store.notice == nil)
            #expect(store.effectiveTheme.terminal.foreground.hex == 0)
            #expect(store.library.yours.count == 1)
        }
    }
}
#endif
