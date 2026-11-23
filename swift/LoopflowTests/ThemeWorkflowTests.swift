#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Appearance settings", .serialized)
@MainActor
struct ThemeWorkflowTests {
    /// A button launches its asynchronous save on the main actor.
    private func waitFor(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(10)
        while !condition(), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        try #require(condition())
    }

    @Test func themeReadIncludesTheSlotForACheckedEdit() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let result = try JSONDecoder().decode(ThemeDeclaration.self, from: Data(contentsOf:
            root.appending(path: "tests/fixtures/dto/theme_declaration.json")))
        #expect(result.entries.map(\.path) == [["theme"]])
        #expect(result.entries.map(\.value) == [result.file.value])
        #expect(result.file.value == .string("dark"))
    }

    @Test(arguments: [false, true])
    func pickingACardWritesImmediatelyAndUndoRestoresTheDeclaration(existing: Bool) async throws {
        try await withThemeServer { location, paths in
            let original = existing ? "light:Folio" : nil
            try paths.writeConfig("agent: claude\n# retained\n" + (original.map { "theme: '\($0)'\n" } ?? ""))
            let appearance = try await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            let notices = NoticeCenter()
            let view = ThemeChooser(appearance: appearance, notices: notices)
            let cards = try view.inspect().findAll(ThemeCard.self).map { try $0.actualView().theme.name }
            #expect(cards == ThemeDefinition.builtins.filter { !$0.isDark }.map(\.name) + ThemeDefinition.builtins.filter(\.isDark).map(\.name))
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-mode")
            #expect((try? view.inspect().find(button: "Apply")) == nil)
            let card = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Fjord Light").button()
            try card.tap()
            try await waitFor { notices.current?.action != nil }
            #expect(try await savedTheme(location) == "light:Fjord Light,dark:Loopflow Dark")
            #expect(appearance.effectiveTheme.name == "Fjord Light")
            try #require(notices.current?.action).perform()
            try await waitFor { appearance.effectiveTheme.name == (existing ? "Folio" : "Loopflow Light") }
            #expect(try await savedTheme(location) == original)
            #expect(paths.configText()?.contains("# retained") == true)
            if !existing { #expect(paths.configText()?.contains("theme:") == false) }
        }
    }

    @Test func modeChangesKeepBothSystemSidesAndUndoChecksForHandEdits() async throws {
        try await withThemeServer { location, paths in
            let appearance = try await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            let notices = NoticeCenter()
            let manager = UndoManager()
            manager.groupsByEvent = false
            manager.beginUndoGrouping()
            await appearance.applyMode(.dark, notices: notices, manager: manager)
            manager.endUndoGrouping()
            let view = ThemeChooser(appearance: appearance, notices: notices)
            #expect((try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Fjord Light")) == nil)
            #expect(try await savedTheme(location) == "Loopflow Dark")
            try paths.writeConfig("theme: Signal\n")
            manager.undo()
            try await waitFor { appearance.effectiveTheme.name == "Signal" }
            #expect(try await savedTheme(location) == "Signal")
            #expect(notices.current?.action == nil)
            await appearance.applyMode(.system, notices: notices)
            await appearance.selectTheme("Fig", notices: notices, manager: nil)
            #expect(appearance.selectedName == "Fig")
            #expect(appearance.effectiveTheme.name == "Loopflow Light")
            appearance.setSystemDark(true)
            #expect(appearance.effectiveTheme.name == "Fig")
        }
    }

    @Test func anExplicitNullAddedAfterReadingAnAbsentThemeSurvivesAStalePick() async throws {
        try await withThemeServer { location, paths in
            let store = try await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            try paths.writeConfig("theme: null\n")
            await store.applyMode(.dark)
            #expect(store.readFailure?.contains("changed") == true)
            #expect(paths.configText() == "theme: null\n")
            let notices = NoticeCenter()
            await store.applyMode(.dark, notices: notices)
            try #require(notices.current?.action).perform()
            try await waitFor { store.selection.mode == .system }
            let restored = try await ServerClient.configuration(.configTheme(value: nil), as: ThemeDeclaration.self, location: location)
            #expect(restored.entries.map(\.value) == [.null])
        }
    }

    @Test func aFailedSelectionSaveKeepsTheImportedFileAndEditorAvailable() async throws {
        try await withThemeServer { location, paths in
            let store = try await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            let source = paths.config.deletingLastPathComponent().appending(path: "Midnight")
            try Data("background=171924\nforeground=CAD2EF".utf8).write(to: source)
            store.importTheme(source)
            try paths.writeConfig("theme: Signal\n")
            await store.apply()
            #expect(store.editing && store.canApply)
            #expect(store.library.yours["Midnight"] != nil)
            #expect(try await savedTheme(location) == "Signal")
            #expect(store.notice?.contains("changed") == true)
            await store.apply()
            #expect(!store.editing)
            #expect(try await savedTheme(location) == "light:Loopflow Light,dark:Midnight")
        }
    }

    @Test func customizeCancelApplyAndRevertUseOnlyTheEditorDraft() async throws {
        try await withThemeServer { location, paths in
            try paths.writeConfig("theme: Folio\n")
            let appearance = try await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            let chooser = ThemeChooser(appearance: appearance, notices: NoticeCenter())
            @MainActor func customize() throws {
                try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-customize-theme").button().tap()
                appearance.editTheme { palette, _ in palette.accent = ThemeRGB(0x224466) }
            }
            let file = paths.themes.appendingPathComponent("Folio")
            try customize()
            try chooser.inspect().find(button: "Cancel").tap()
            #expect(!appearance.editing && !FileManager.default.fileExists(atPath: file.path))
            try customize()
            try chooser.inspect().find(button: "Apply").tap()
            try await waitFor { !appearance.editing && !appearance.saving }
            #expect(try ThemeFile.read(file).theme.palette.definition.accent.hex == 0x224466)
            try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-customize-theme").button().tap()
            try chooser.inspect().find(button: "Revert Folio to built-in").tap()
            #expect(FileManager.default.fileExists(atPath: file.path))
            await appearance.apply()
            #expect(!FileManager.default.fileExists(atPath: file.path))
            #expect(appearance.selectedSource == .builtin)
        }
    }
}

/// Stage a theme file in the color editor to exercise window and terminal previews.
@MainActor
enum ThemeFixture: String, CaseIterable {
    case light, dark, lightImport, darkImport, bundled

    func preview(in appearance: AppearanceStore) throws {
        let contents: String
        switch self {
        case .light: contents = ThemeFile.text(.light)
        case .dark: contents = ThemeFile.text(.dark)
        case .bundled: contents = ThemeFile.text(try #require(ThemeDefinition.builtins.first { $0.name == "Signal" }))
        case .lightImport: contents = "background=fff4df\nforeground=34291f"
        case .darkImport: contents = "background=171924\nforeground=CAD2EF"
        }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let source = directory.appendingPathComponent(rawValue)
        try Data(contents.utf8).write(to: source)
        appearance.importTheme(source)
        #expect(appearance.notice == nil)
    }
}
#endif
