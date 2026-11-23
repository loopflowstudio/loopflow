#if os(macOS)
import Foundation
import Loopflow
import SwiftUI
import Testing
import ViewInspector
@testable import LoopflowMac

@Suite("Settings navigation and preferences")
@MainActor
struct SettingsWorkflowTests {
    @Test func settingsOpensOnGeneralAndKeepsTheExistingPanesReachable() throws {
        var navigation = SettingsNavigation()
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        let window = SettingsWindow(navigation: Binding(get: { navigation }, set: { navigation = $0 }),
                                    appearance: appearance)
        let split = try window.inspect().find(ViewType.NavigationSplitView.self)
        let sidebar = try split.sidebarView()
        for pane in SettingsPane.allCases {
            #expect(try sidebar.find(text: pane.rawValue).string() == pane.rawValue)
        }
        #expect(try split.detailView().find(ViewType.Toggle.self).labelView().text().string() == "Autosave Task Files")

        navigation.search = "  CuTs "
        let results = try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView()
        #expect(try results.find(text: "Shortcuts").string() == "Shortcuts")
        #expect(throws: (any Error).self) { try results.find(text: "Appearance") }
        navigation.open(.shortcuts)
        let detail = try window.inspect().find(ViewType.NavigationSplitView.self).detailView()
        _ = try detail.find(KeyboardShortcutsView.self)
        navigation.open(.mobile)
        _ = try window.inspect().find(MobileSettings.self)
        navigation.open(.configuration)
        _ = try window.inspect().find(ConfigurationSettingsView.self)
        navigation.search = "missing"
        #expect(try window.inspect().find(text: "No matching panes").string() == "No matching panes")
        navigation.open(.appearance)
        #expect(navigation.search.isEmpty)
        _ = try window.inspect().find(ThemeChooser.self)
    }

    @Test(arguments: [true, false])
    func autosaveWritesTheTaskFilePreferenceAndUndoRestoresItsPreviousState(keyboard: Bool) throws {
        let suite = "loopflow-settings-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let notices = NoticeCenter()
        let manager = UndoManager()
        manager.groupsByEvent = false
        manager.beginUndoGrouping()
        let view = GeneralSettings(notices: notices, undoManager: manager, defaults: defaults)
        let toggle = try view.inspect().find(ViewType.Toggle.self)
        #expect(try toggle.isOn())
        try toggle.tap()
        manager.endUndoGrouping()
        #expect(!defaults.savesTaskFilesAutomatically)
        #expect(try !GeneralSettings(notices: notices, undoManager: manager, defaults: defaults).inspect().find(ViewType.Toggle.self).isOn())
        let undo = try #require(notices.current?.action)
        #expect(undo.title == "Undo")
        if keyboard { manager.undo() } else { undo.perform() }
        #expect(defaults.savesTaskFilesAutomatically)
        #expect(defaults.object(forKey: "taskFilesAutosave") == nil)
    }

    @Test func shortcutsShowTheTaskFileCommandsAndBoundDesktopActions() throws {
        let reference = try KeyboardShortcutsView().inspect()
        let text = try reference.findAll(ViewType.Text.self).map { try $0.string() }
        for label in ["Navigation", "App", "Task Files"] {
            #expect(text.contains(label))
        }
        let rows = try reference.findAll(ViewType.LabeledContent.self).map {
            try ($0.labelView().text().string(), $0.text(0).string())
        }
        let shortcuts = Dictionary(uniqueKeysWithValues: rows)
        #expect(shortcuts["Save Task file"] == "⌘S")
        #expect(shortcuts["Settings"] == "⌘,")
        for action in DesktopAction.all where !action.keys.isEmpty {
            let value = try #require(shortcuts[action.title])
            #expect(value.contains(action.keys[0].display))
            #expect(value.contains(action.keys[action.keys.count - 1].display))
        }
    }
}
#endif
