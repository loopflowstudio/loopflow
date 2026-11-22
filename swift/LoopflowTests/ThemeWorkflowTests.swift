#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Retained work through theme preview", .serialized)
@MainActor
struct ThemeWorkflowTests {
    @Test func customizeOpensTheEditorInPlaceAndRevertRestoresTheBuiltin() throws {
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        appearance.beginPreview()
        let chooser = ThemeChooser(appearance: appearance).modifier(AppAppearance(appearance: appearance))
        try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Folio").button().tap()
        #expect(appearance.selectedName == "Folio")
        #expect(throws: (any Error).self) { try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-delete-theme") }
        try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-customize-theme").button().tap()
        #expect(appearance.selectedSource == .yours)
        let editor = ThemeEditor(appearance: appearance).modifier(AppAppearance(appearance: appearance))
        #expect(try editor.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-name").text().string() == "Folio")
        appearance.editTheme { palette, _ in palette.accent = ThemeRGB(0x224466) }
        let revert = try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-delete-theme").button()
        #expect(try revert.labelView().text().string() == "Revert to built-in")
        try revert.tap()
        #expect(appearance.selectedSource == .builtin)
        #expect(appearance.selectedTheme.palette.definition == ThemeDefinition.builtins.first { $0.name == "Folio" }?.palette.definition)
    }

    @Test func everyCardDrawsItsOwnThemeAndClickingOnePreviewsIt() throws {
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        appearance.beginPreview()
        appearance.previewMode(.system)
        let chooser = ThemeChooser(appearance: appearance)
        let cards = try chooser.inspect().findAll(ThemeCard.self).map { try $0.actualView().theme.name }
        // Light then dark, each in family order, so a family's two halves sit in the same column.
        #expect(cards == ThemeDefinition.builtins.filter { !$0.isDark }.map(\.name) + ThemeDefinition.builtins.filter(\.isDark).map(\.name))
        let fig = try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Fig")
        #expect(try fig.find(ThemeCard.self).findAll(ViewType.Text.self).map { try $0.string() }
            == ["Fig", "Busy", "Blocked", "Error", "Selected row", "Run"])
        try fig.button().tap()
        #expect(appearance.selectedTheme.name == "Fig")
        #expect(appearance.effectiveTheme.name == "Fig")
        #expect(appearance.current.dark == "Fig")
    }

    @Test func chooserFiltersEachSideToItsAppearanceAndWritesTheChoice() async throws {
        try await withThemeServer { location, paths in
            let appearance = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            appearance.beginPreview()
            let view = ThemeChooser(appearance: appearance)
            let mode = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-mode").picker()
            for choice in [AppearanceMode.light, .dark, .system] {
                try mode.select(value: choice)
                for side in [AppearanceMode.light, .dark] {
                    let heading = try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-heading-\(side.rawValue)").text().string()
                    let shown = choice == .system || choice == side
                    #expect(heading == (shown ? "\(side.menuTitle) themes · Loopflow \(side.menuTitle)" : nil))
                }
                #expect(appearance.previewSide == (choice == .dark ? .dark : .light))
                let showsDark = (try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Signal")) != nil
                let showsLight = (try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Fjord Light")) != nil
                #expect(showsDark == (choice != .light))
                #expect(showsLight == (choice != .dark))
            }
            #expect(appearance.entries(for: .light).builtins.count == 12)
            #expect(appearance.entries(for: .light).builtins.allSatisfy { !$0.theme.isDark })
            #expect(appearance.entries(for: .dark).builtins.count == 12)
            #expect(appearance.entries(for: .dark).builtins.allSatisfy { $0.theme.isDark })
            // System lists both sides at once; each card assigns its own side and takes the Customize row.
            func customizeShown(under name: String) -> Bool {
                (try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-\(name)").parent()
                    .find(viewWithAccessibilityIdentifier: "appearance-customize-theme")) != nil
            }
            try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Fjord Light").button().tap()
            #expect(customizeShown(under: "Fjord Light"))
            try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Signal").button().tap()
            #expect(customizeShown(under: "Signal"))
            #expect(!customizeShown(under: "Fjord Light"))
            #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-heading-light").text().string() == "Light themes · Fjord Light")
            #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-heading-dark").text().string() == "Dark themes · Signal")
            #expect(appearance.themeName(for: .light) == "Fjord Light")
            #expect(appearance.current.mode == .system)
            try view.inspect().find(button: "Apply").tap()
            let deadline = ContinuousClock.now + .seconds(10)
            // Apply closes the chooser, then reads the saved choice back; the
            // applied theme is in effect once that read ends.
            while appearance.chooserOpen || appearance.saving, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(10))
            }
            #expect(!appearance.chooserOpen)
            #expect(!appearance.saving)
            let savedTheme1 = try await savedTheme(location)
            #expect(savedTheme1 == "light:Fjord Light,dark:Signal")
            #expect(appearance.effectiveTheme.name == "Fjord Light")
            appearance.setSystemDark(true)
            #expect(appearance.effectiveTheme.name == "Signal")
            let restored = await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            #expect(restored.selection == appearance.selection)
            #expect(restored.effectiveTheme.name == "Fjord Light")
        }
    }

    @Test func aDarkFileImportedFromTheLightSectionPreviewsInDarkWithApplyEnabled() throws {
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        appearance.beginPreview()
        appearance.previewMode(.light)
        let view = ThemeChooser(appearance: appearance)
        #expect((try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-heading-dark")) == nil)
        let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            .appendingPathComponent("Midnight")
        defer { try? FileManager.default.removeItem(at: source.deletingLastPathComponent()) }
        try FileManager.default.createDirectory(at: source.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("background=101010\nforeground=EEEEEE".utf8).write(to: source)
        appearance.importTheme(source)
        #expect(appearance.current.mode == .dark)
        #expect(appearance.current.light == "Loopflow Light")
        #expect(appearance.effectiveTheme.name == "Midnight")
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-mode").picker().selectedValue(AppearanceMode.self) == .dark)
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-heading-dark").text().string() == "Dark themes · Midnight")
        #expect(try !view.inspect().find(button: "Apply").isDisabled())
        appearance.cancel()
        #expect(appearance.current.mode == .system)
        #expect(appearance.effectiveTheme.name == "Loopflow Light")
    }
}
/// The accepted appearances, exercised through the same preview transaction as the chooser.
@MainActor
enum ThemeFixture: String, CaseIterable {
    case light, dark, lightImport, darkImport, bundled

    func preview(in appearance: AppearanceStore) throws {
        appearance.beginPreview()
        switch self {
        case .light, .dark: appearance.previewMode(AppearanceMode(rawValue: rawValue)!)
        case .bundled:
            appearance.previewMode(.dark)
            appearance.previewTheme("Signal")
        case .lightImport, .darkImport:
            appearance.previewMode(self == .lightImport ? .light : .dark)
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            let source = directory.appendingPathComponent(rawValue)
            defer { try? FileManager.default.removeItem(at: directory) }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let colors = self == .lightImport ? "background=fff4df\nforeground=34291f" : "background=171924\nforeground=CAD2EF"
            try Data(colors.utf8).write(to: source)
            appearance.importTheme(source)
        }
        #expect(appearance.notice == nil)
    }
}
#endif
