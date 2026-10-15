#if os(macOS)
import AppKit
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
    private func imported(_ text: String = "background = #171924\nforeground = #CAD2EF") throws -> ThemeRecord {
        try ThemeImport.parse(Data(text.utf8), name: "Personal")
    }

    @Test func loopflowLightRetainsItsOriginalDesign() {
        let light = ThemeDefinition.light.palette.definition
        #expect(light.background.hex == 0xFAF8F5)
        #expect(light.surface.hex == 0xFFFDF9)
        #expect(light.surfaceMuted.hex == 0xF3EEE7)
        #expect(light.surfaceElement.hex == 0xECE6DD)
        #expect(light.text.hex == 0x2A2624)
        #expect(light.textSecondary.hex == 0x5F5752)
        #expect(light.accent.hex == 0x722F37)
        #expect(light.accentHover.hex == 0x5E2630)
        #expect(light.emphasisInk.hex == 0x722F37)
        #expect(light.workSelectedFill.hex == 0x7A333D)
        #expect(light.workSelectedText.hex == 0xFFFAF3)
        let dark = ThemeDefinition.dark.palette.definition
        #expect(dark.background.hex == 0x2B3036)
        #expect(dark.surface.hex == 0x343B44)
        #expect(dark.accent.hex == 0x722F37)
        #expect(dark.emphasisInk.hex == 0xCEBFA9)
        #expect(dark.workSelectedFill.hex == 0xAB9B86)
        // Loopflow's authored design is independent of the import conversion.
        #expect(light != GhosttyThemeMapping.palette(ThemeDefinition.light.terminal))
        #expect(dark != GhosttyThemeMapping.palette(ThemeDefinition.dark.terminal))
    }

    @Test func linenIsASeparateChoiceAndCompanionsKeepOrdinaryTextQuiet() throws {
        let linen = try #require(ThemeDefinition.builtins.first { $0.id == "neo-light" })
        #expect(linen.name == "Linen")
        #expect(!linen.isDark)
        #expect(linen.palette.definition != ThemeDefinition.light.palette.definition)
        for theme in ThemeDefinition.builtins where !["light", "dark"].contains(theme.id) {
            let palette = theme.palette.definition
            #expect(palette.emphasisInk == palette.text)
            #expect(palette.accentInk != palette.text)
            #expect(palette.accent != palette.accentHover)
            #expect(palette.accentHover != palette.accentPressed)
            // Terminal syntax may stay colorful; ordinary UI text should not look selected.
            let channels = palette.text.channels
            #expect(channels.max()! - channels.min()! < 0.09)
            if theme.isDark { #expect(palette.surfaceElement.luminance > palette.background.luminance) }
        }
        let defaults = defaults()
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        store.beginPreview(); store.previewTheme(linen.id); store.apply()
        #expect(AppearanceStore(defaults: defaults, systemDark: false).effectiveTheme.id == linen.id)
        store.beginPreview(); store.previewTheme("light"); store.cancel()
        #expect(store.effectiveTheme.id == linen.id)
    }

    @Test func versionThreePreservesImportedPaletteAndAssignments() throws {
        let defaults = defaults()
        let theme = try imported()
        var record = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(theme)) as? [String: Any])
        var palette = try #require(record["palette"] as? [String: Any])
        palette.removeValue(forKey: "accentInk"); palette.removeValue(forKey: "accentPressed")
        for key in ["terminalHover", "terminalSelection", "terminalFailure"] {
            palette[key] = ["rgb": ["hex": 0], "opacity": 0]
        }
        record["palette"] = palette
        let choice = AppearanceSelection(mode: .dark, lightThemeID: "light", darkThemeID: theme.id)
        defaults.set(try JSONSerialization.data(withJSONObject: ["version": 3,
            "selection": JSONSerialization.jsonObject(with: JSONEncoder().encode(choice)), "imports": [record]]),
            forKey: "appearancePreferences")
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(store.notice == nil)
        #expect(store.selection.mode == choice.mode)
        #expect(store.selection.lightThemeID == "light")
        #expect(store.effectiveTheme.terminal == theme.terminal)
        let restored = store.effectiveTheme.palette.definition
        #expect(restored.accentInk == theme.palette.emphasisInk)
        #expect(restored.accentPressed == theme.palette.accentHover)
        #expect(restored.workSelectedFill == theme.palette.workSelectedFill)
        #expect(restored.text == theme.palette.text)
        #expect(AppearanceStore(defaults: defaults, systemDark: false).effectiveTheme.palette.definition == restored)
    }

    @Test func completePalettesRejectMissingRolesAndKeepExplicitValues() throws {
        for theme in ThemeDefinition.builtins {
            let data = try JSONEncoder().encode(theme)
            let restored = try JSONDecoder().decode(ThemeDefinition.self, from: data)
            #expect(restored.palette.definition == theme.palette.definition)
            #expect(restored.readabilityErrors.isEmpty)
            let record = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let palette = try #require(record["palette"] as? [String: Any])
            for key in palette.keys {
                var incomplete = palette
                incomplete.removeValue(forKey: key)
                var changed = record
                changed["palette"] = incomplete
                let invalid = try JSONSerialization.data(withJSONObject: changed)
                #expect(throws: (any Error).self) { try JSONDecoder().decode(ThemeDefinition.self, from: invalid) }
            }
        }
    }

    @Test func ghosttyMappingUsesAnsiAndSelectionBeyondBackgroundAndForeground() throws {
        let first = try imported("background=ffffff\nforeground=000000\npalette=4=000088\npalette=1=880000\nselection-background=222222\nselection-foreground=ffffff")
        let second = try imported("background=ffffff\nforeground=000000\npalette=4=006600\npalette=1=880066\nselection-background=444444\nselection-foreground=ffffff")
        #expect(first.palette.background == second.palette.background)
        #expect(first.palette.accent != second.palette.accent)
        #expect(first.palette.emphasisInk == second.palette.emphasisInk)
        #expect(first.palette.accentInk != second.palette.accentInk)
        #expect(first.palette.workingTone != second.palette.workingTone)
        #expect(first.palette.failureTone != second.palette.failureTone)
        #expect(first.palette.workSelectedFill != second.palette.workSelectedFill)
        #expect(first.definition.readabilityErrors.isEmpty)
        #expect(second.definition.readabilityErrors.isEmpty)
    }

    @Test func previousImportsConvertOnceAndIncompleteCurrentRecordsStayRecoverable() throws {
        let defaults = defaults()
        let theme = try imported()
        var old = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(theme)) as? [String: Any])
        old.removeValue(forKey: "palette")
        let selection = AppearanceSelection(mode: .system, lightThemeID: theme.id, darkThemeID: theme.id)
        let data = try JSONSerialization.data(withJSONObject: ["version": 2, "selection": JSONSerialization.jsonObject(with: JSONEncoder().encode(selection)), "imports": [old]])
        defaults.set(data, forKey: "appearancePreferences")
        let migrated = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(migrated.selection.mode == selection.mode)
        #expect(migrated.selection.lightThemeID == migrated.selection.darkThemeID)
        #expect(migrated.overrides.first?.palette == theme.palette)
        #expect(migrated.overrides.first?.terminal == theme.terminal)
        let saved = try #require(defaults.data(forKey: "appearancePreferences"))
        #expect(saved != data)
        #expect(AppearanceStore(defaults: defaults, systemDark: true).overrides == migrated.overrides)
        var invalid = try #require(JSONSerialization.jsonObject(with: saved) as? [String: Any])
        invalid["overrides"] = [old]
        let broken = try JSONSerialization.data(withJSONObject: invalid)
        defaults.set(broken, forKey: "appearancePreferences")
        #expect(AppearanceStore(defaults: defaults, systemDark: false).notice != nil)
        #expect(defaults.data(forKey: "appearancePreferences") == broken)
        var terminal = try #require(old["terminal"] as? [String: Any])
        terminal["indexed"] = []
        old["terminal"] = terminal
        invalid["version"] = 2; invalid["imports"] = [old]
        defaults.set(try JSONSerialization.data(withJSONObject: invalid), forKey: "appearancePreferences")
        #expect(AppearanceStore(defaults: defaults, systemDark: false).notice != nil)
    }

    @Test func unchangedDiffRecolorsWithoutReplacingSelection() throws {
        let patch = "@@ -1 +1 @@\n-old\n+new\n"
        let lines = TaskPatchLine.changes(in: patch)
        let text = TaskPatchTextView(usingTextLayoutManager: true)
        text.update(patch: patch, lines: lines, palette: .dark)
        let selection = NSRange(location: 12, length: 3)
        text.setSelectedRange(selection)
        text.update(patch: patch, lines: lines, palette: .light)
        #expect(text.string == patch)
        #expect(text.selectedRange() == selection)
        #expect(text.backgroundColor == NSColor(LoopflowPalette.light.background))
        #expect(text.textColor == NSColor(LoopflowPalette.light.text))
        #expect(text.insertionPointColor == NSColor(LoopflowPalette.light.keyboardFocus))
        #expect(text.selectedTextAttributes[.backgroundColor] as? NSColor == NSColor(LoopflowPalette.light.workSelectedFill))
        for line in lines {
            let color = text.textStorage?.attribute(.backgroundColor, at: line.range.location, effectiveRange: nil) as? NSColor
            #expect(color == NSColor(line.added ? LoopflowPalette.light.successTone.fill : LoopflowPalette.light.failureTone.fill))
        }
    }

    @Test func assignedThemesFollowModeAndRestoreTogether() throws {
        let defaults = defaults()
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(store.themes.count == 24)
        #expect(Set(store.themes.map(\.id)).count == 24)
        let night = "bundled:TokyoNight"
        let day = "bundled:Gruvbox Light"
        store.beginPreview()
        store.editSlot(.light); store.previewTheme(night)
        store.editSlot(.dark); store.previewTheme(day)
        #expect(store.current.mode == .system)
        #expect(store.effectiveTheme.id == day)
        store.apply()
        #expect(store.effectiveTheme.id == night)
        let restored = AppearanceStore(defaults: defaults, systemDark: true)
        #expect(restored.selection.lightThemeID == night)
        #expect(restored.selection.darkThemeID == day)
        #expect(restored.effectiveTheme.id == day)
        restored.applyMode(.light)
        restored.setSystemDark(true)
        #expect(restored.effectiveTheme.id == night)
        restored.applyMode(.dark)
        restored.setSystemDark(false)
        #expect(restored.effectiveTheme.id == day)
        restored.applyMode(.system)
        #expect(restored.effectiveTheme.id == night)
        restored.beginPreview(); restored.previewTheme("dark"); restored.previewMode(.dark)
        restored.cancel()
        #expect(restored.selection == store.selection)
        #expect(restored.effectiveTheme.id == night)
    }

    @Test func bothImportedAssignmentsPersistAndPreviousSelectionMigrates() throws {
        let defaults = defaults()
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: path) }
        store.beginPreview()
        var ids: [String] = []
        for slot in [AppearanceMode.light, .dark] {
            store.editSlot(slot)
            try Data("background=171924\nforeground=CAD2EF".utf8).write(to: path)
            store.importTheme(path)
            ids.append(store.selectedID)
        }
        store.apply()
        let restored = AppearanceStore(defaults: defaults, systemDark: true)
        #expect(restored.overrides.count == 2)
        #expect(restored.selection.lightThemeID == ids[0])
        #expect(restored.selection.darkThemeID == ids[1])
        #expect(restored.effectiveTheme.id == ids[1])
        let legacy = try imported()
        let previous = try JSONSerialization.data(withJSONObject: [
            "version": 1, "selection": legacy.id,
            "imports": JSONSerialization.jsonObject(with: JSONEncoder().encode([legacy])),
        ])
        defaults.set(previous, forKey: "appearancePreferences")
        let migrated = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(migrated.selection.mode == .dark)
        #expect(migrated.effectiveTheme.name == legacy.name)
        #expect(migrated.effectiveTheme.terminal == legacy.terminal)
        #expect(migrated.overrides.first?.palette == legacy.palette)
        let saved = try #require(defaults.data(forKey: "appearancePreferences"))
        #expect((try JSONSerialization.jsonObject(with: saved) as? [String: Any])?["version"] as? Int == 5)
    }

    @Test func migrationPreviewCancelApplyAndRestore() throws {
        let defaults = defaults()
        defaults.set("dark", forKey: "appearanceMode")
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(store.selection.mode == .dark)
        #expect(defaults.object(forKey: "appearanceMode") == nil)
        #expect(store.effectiveTheme.terminal == ThemeDefinition.dark.terminal)
        store.beginPreview(); store.previewMode(.light)
        #expect(store.effectiveTheme.id == "light")
        #expect(AppearanceStore(defaults: defaults, systemDark: false).selection.mode == .dark)
        store.cancel()
        #expect(store.effectiveTheme.id == "dark")
        store.beginPreview(); store.previewMode(.light); store.apply()
        #expect(AppearanceStore(defaults: defaults, systemDark: true).effectiveTheme.id == "light")
        store.applyMode(.system); store.setSystemDark(true)
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
        store.applyMode(.dark)
        let saved = defaults.data(forKey: "appearancePreferences")
        reject = true; store.beginPreview(); store.previewMode(.light); store.apply()
        #expect(store.effectiveTheme.id == "dark")
        #expect(defaults.data(forKey: "appearancePreferences") == saved)
        #expect(store.notice == "Allocation failed")
        reject = false; store.cancel()
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: path) }
        try Data("background=171924\nforeground=CAD2EF\ncommand=touch /never\n".utf8).write(to: path)
        store.beginPreview(); store.importTheme(path)
        let id = store.selectedID
        #expect(store.overrides.isEmpty)
        #expect(store.diagnostics.contains { $0.contains("omitted command") })
        store.apply()
        try FileManager.default.removeItem(at: path)
        let restored = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(restored.selection.darkThemeID == id)
        #expect(restored.effectiveTheme.terminal.background.hex == 0x171924)
        #expect(restored.overrides.count == 1)
    }

    @Test func editingImportAndResetStayInsideTwentyFourSlots() throws {
        let defaults = defaults()
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        let original = ThemeDefinition.light
        let path = FileManager.default.temporaryDirectory.appendingPathComponent("replacement-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: path) }
        store.beginPreview()
        for theme in ThemeDefinition.builtins {
            store.previewTheme(theme.id)
            store.editTheme { $0.name = "My " + theme.name }
        }
        #expect(store.themes.count == 24)
        store.previewTheme("light")
        store.editTheme { $0.palette.accentInk = ThemeRGB(0x4C665B) }
        #expect(store.effectiveTheme.palette.keyboardFocus == ThemeRGB(0x4C665B).color)
        store.editSlot(.dark); store.previewTheme("light")
        try Data("background=171924\nforeground=CAD2EF".utf8).write(to: path)
        store.importTheme(path)
        #expect(store.selectedID == "light")
        #expect(store.themes.count == 24)
        #expect(store.current.lightThemeID == store.current.darkThemeID)
        store.apply()
        let restored = AppearanceStore(defaults: defaults, systemDark: true)
        #expect(restored.themes.count == 24)
        #expect(restored.overrides.count == 24)
        #expect(restored.effectiveTheme.terminal.background.hex == 0x171924)
        #expect(restored.effectiveTheme.name == path.lastPathComponent)
        restored.beginPreview(); restored.resetTheme()
        #expect(restored.selectedTheme.name == original.name)
        #expect(restored.selectedTheme.palette.definition == original.palette.definition)
        #expect(restored.selectedTheme.terminal == original.terminal)
        restored.cancel()
        #expect(restored.effectiveTheme.terminal.background.hex == 0x171924)
        restored.beginPreview(); restored.resetTheme(); restored.apply()
        let reset = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(reset.effectiveTheme.name == original.name)
        #expect(reset.effectiveTheme.palette.definition == original.palette.definition)
        #expect(reset.themes.count == 24)
        #expect(reset.overrides.count == 23)
    }

    @Test func invalidEditsAndFailedImportsCannotReplaceSavedLibrary() throws {
        let defaults = defaults()
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        store.applyMode(.light)
        let saved = defaults.data(forKey: "appearancePreferences")
        store.beginPreview(); store.editTheme { $0.name = "" }
        #expect(!store.canApply)
        store.apply()
        #expect(defaults.data(forKey: "appearancePreferences") == saved)
        store.editTheme { $0.name = "Mine"; $0.palette.text = $0.palette.background }
        #expect(!store.canApply)
        #expect(store.effectiveTheme.palette.definition == ThemeDefinition.light.palette.definition)
        store.cancel()
        #expect(store.selectedTheme.name == "Loopflow Light")
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: path) }
        try JSONEncoder().encode(ThemeDefinition.dark).write(to: path)
        store.beginPreview(); store.importTheme(path)
        #expect(store.selectedID == "light")
        #expect(store.selectedTheme.palette.definition == ThemeDefinition.dark.palette.definition)
        try Data("broken".utf8).write(to: path)
        store.importTheme(path)
        #expect(store.notice != nil)
        #expect(store.selectedTheme.name == "Loopflow Dark")
        store.cancel()
        #expect(defaults.data(forKey: "appearancePreferences") == saved)
    }

    @Test func retiredAssignmentsMigrateWithoutAddingSlotsOrLosingColors() throws {
        let defaults = defaults()
        let oldID = "bundled:Catppuccin Frappe"
        let old = try #require(try ThemeDefinition.retiredBuiltin(oldID))
        let selection = AppearanceSelection(mode: .system, lightThemeID: oldID, darkThemeID: "dark")
        defaults.set(try JSONSerialization.data(withJSONObject: ["version": 4,
            "selection": JSONSerialization.jsonObject(with: JSONEncoder().encode(selection)), "imports": []]),
            forKey: "appearancePreferences")
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(store.notice == nil)
        #expect(store.themes.count == 24)
        #expect(store.effectiveTheme.terminal == old.terminal)
        #expect(store.effectiveTheme.palette.definition == old.palette.definition)
        #expect(store.selection.darkThemeID == "dark")
        #expect(AppearanceStore(defaults: defaults, systemDark: false).effectiveTheme.name == old.name)
    }

    @Test func oversizedLegacyLibraryIsRetainedWithoutDroppingThemes() throws {
        let defaults = defaults()
        let records = try (0..<25).map { _ in try imported() }
        let data = try JSONSerialization.data(withJSONObject: ["version": 4,
            "selection": JSONSerialization.jsonObject(with: JSONEncoder().encode(AppearanceSelection())),
            "imports": JSONSerialization.jsonObject(with: JSONEncoder().encode(records))])
        defaults.set(data, forKey: "appearancePreferences")
        let store = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(store.notice?.contains("cannot fit in 24 slots") == true)
        #expect(defaults.data(forKey: "appearancePreferences") == data)
        #expect(store.themes.count == 24)
        #expect(store.overrides.isEmpty)
    }

    @Test func corruptAndUnavailablePreferencesRemainRecoverable() throws {
        for data in [Data("broken".utf8), Data(#"{"version":99,"selection":"dark","imports":[]}"#.utf8),
                     Data(#"{"version":1,"selection":"missing","imports":[]}"#.utf8)] {
            let defaults = defaults(); defaults.set(data, forKey: "appearancePreferences")
            let store = AppearanceStore(defaults: defaults, systemDark: true)
            #expect(store.effectiveTheme.id == "dark")
            #expect(store.notice != nil)
            #expect(defaults.data(forKey: "appearancePreferences") == data)
            store.applyMode(.light)
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
        #expect(throws: ThemeError.self) { try ThemeImport.parse(Data([0xFF]), name: "Invalid") }
        #expect(throws: ThemeError.self) { try ThemeImport.parse(Data(repeating: 32, count: 262145), name: "Huge") }
        #expect(try imported("background=777777\nforeground=888888").terminal.readabilityErrors.first?.contains("Default text") == true)
        #expect(try imported("background=000000\nforeground=ffffff\nselection-background=888888\nselection-foreground=999999").terminal.readabilityErrors.first?.contains("Selection text") == true)
    }

    #if canImport(GhosttyKit)
    @Test func defaultConfigurationLoadsAfterInitialAppearance() throws {
        // This crosses the formerly crashing boundary without a surface or PTY.
        let manager = GhosttyManager.shared
        try manager.applyTheme(.light)
        manager.initialize()
        #expect(manager.state == .ready)
        try manager.applyTheme(.dark)
        #expect(manager.theme.id == "dark")
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
        #expect(store.overrides.count == 1)
    }
}
#endif
