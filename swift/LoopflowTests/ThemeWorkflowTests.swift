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
        try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Linen").button().tap()
        #expect(appearance.selectedName == "Linen")
        #expect(throws: (any Error).self) { try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-delete-theme") }
        try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-customize-theme").button().tap()
        #expect(appearance.selectedSource == .yours)
        let editor = ThemeEditor(appearance: appearance).modifier(AppAppearance(appearance: appearance))
        #expect(try editor.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-name").text().string() == "Linen")
        appearance.editTheme { palette, _ in palette.accent = ThemeRGB(0x224466) }
        let revert = try chooser.inspect().find(viewWithAccessibilityIdentifier: "appearance-delete-theme").button()
        #expect(try revert.labelView().text().string() == "Revert to built-in")
        try revert.tap()
        #expect(appearance.selectedSource == .builtin)
        #expect(appearance.selectedTheme.palette.definition == ThemeDefinition.builtins.first { $0.name == "Linen" }?.palette.definition)
    }

    @Test func chooserFiltersEachPickerToItsAppearanceAndWritesTheChoice() throws {
        let paths = ThemePaths.temporary()
        let appearance = AppearanceStore(paths: paths, systemDark: false)
        appearance.beginPreview()
        let view = ThemeChooser(appearance: appearance)
        let mode = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-mode").picker()
        for choice in [AppearanceMode.light, .dark, .system] {
            try mode.select(value: choice)
            for slot in [AppearanceMode.light, .dark] {
                let button = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-picker-\(slot.rawValue)").button()
                #expect(button.isDisabled() == (choice != .system && choice != slot))
            }
            #expect(appearance.picker == (choice == .dark ? .dark : .light))
            let showsDark = (try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Night Owl")) != nil
            let showsLight = (try? view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Celadon")) != nil
            #expect(showsDark == (choice != .light))
            #expect(showsLight == (choice != .dark))
        }
        #expect(appearance.entries(for: .light).builtins.count == 12)
        #expect(appearance.entries(for: .light).builtins.allSatisfy { !$0.theme.isDark })
        #expect(appearance.entries(for: .dark).builtins.count == 12)
        #expect(appearance.entries(for: .dark).builtins.allSatisfy { $0.theme.isDark })
        // System lists both sides at once; each tile assigns its own side.
        try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Celadon").button().tap()
        try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-Night Owl").button().tap()
        #expect(appearance.themeName(for: .light) == "Celadon")
        #expect(appearance.current.mode == .system)
        try view.inspect().find(button: "Apply").tap()
        #expect(ThemeConfig.read(paths.config) == "light:Celadon,dark:Night Owl")
        #expect(appearance.effectiveTheme.name == "Celadon")
        appearance.setSystemDark(true)
        #expect(appearance.effectiveTheme.name == "Night Owl")
        let restored = AppearanceStore(paths: paths, systemDark: false)
        #expect(restored.selection == appearance.selection)
        #expect(restored.effectiveTheme.name == "Celadon")
    }

    @Test func chooserExplainsAMismatchedImportAndKeepsApplyDisabled() throws {
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        appearance.beginPreview()
        let view = ThemeChooser(appearance: appearance)
        #expect(try !view.inspect().find(button: "Apply").isDisabled())
        try view.inspect().find(button: "Cancel").tap()
        #expect(!appearance.chooserOpen)
        let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            .appendingPathComponent("Midnight")
        defer { try? FileManager.default.removeItem(at: source.deletingLastPathComponent()) }
        try FileManager.default.createDirectory(at: source.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("background=101010\nforeground=EEEEEE".utf8).write(to: source)
        appearance.beginPreview(); appearance.importTheme(source)
        #expect(try view.inspect().find(button: "Apply").isDisabled())
        let refusal = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-import-refusal").text().string()
        #expect(refusal == "“Midnight” is a dark theme. Import it from the Dark picker.")
        #expect(appearance.effectiveTheme.name == "Loopflow Light")
        try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-picker-dark").button().tap()
        #expect(try !view.inspect().find(button: "Apply").isDisabled())
        appearance.importTheme(source)
        #expect(appearance.effectiveTheme.name == "Midnight")
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
            appearance.previewTheme("Night Owl")
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
