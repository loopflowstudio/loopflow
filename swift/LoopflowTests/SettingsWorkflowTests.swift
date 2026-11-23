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
                                    appearance: appearance, checkout: nil, portfolio: PortfolioService())
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
        navigation.open(.machines)
        _ = try window.inspect().find(ScopedSettings.self)
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

    @Test func machineRowsShowTheServedIdentityCheckAndEndedRental() async throws {
        let source = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let wire = try JSONDecoder().decode([String: JSONValue].self, from: Data(contentsOf:
            source.appending(path: "tests/fixtures/dto/machine_status.json")))
        let sample = try #require(JSONDecoder().decode([MachineConnectionStatus].self, from: JSONEncoder().encode(wire["status"])).first)
        let id = sample.machine.id
        let read = ServedRead.machineStatus(machine: id)
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(read)) == wire["read"])
        let root = FileManager.default.temporaryDirectory.appending(path: "settings-machines-\(UUID())")
        let bin = root.appending(path: "bin")
        try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let ssh = bin.appending(path: "ssh")
        func answering(_ id: String) throws {
            try Data("#!/bin/sh\necho 'lf 0.16.0'\necho '\(id)'\n".utf8).write(to: ssh)
            try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: ssh.path)
        }
        try answering(id)
        let lf = try BuiltLf(home: root, overriding: ["PATH": bin.path + ":/usr/bin:/bin"])
        _ = try await lf.run(["machine", "add", "builder", "--json"])
        let server = LocalServerFixture(lf)
        let location = server.location()
        let checks = MachineConnectionChecks()
        let pane = MachineConnections(location: location, connections: checks)
        do {
            _ = try MachineConnectionRow(name: "builder", health: .checking).inspect().find(text: "Checking…")
            await checks.refresh(location: location)
            _ = try pane.inspect().find(text: "builder")
            _ = try pane.inspect().find(text: "Reachable")
            try answering("home_22222222222222222222222222222222")
            await checks.refresh(location: location)
            let wrongHost = try pane.inspect()
            _ = try wrongHost.find(text: "Connection error")
            #expect(try wrongHost.findAll(ViewType.Text.self).contains { try $0.string().contains("identity changed") })
            _ = try await lf.run([root.appending(path: "loopflow.db").path, """
                INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,cap_usd,deadline,state,created_at,machine_id)
                VALUES('rent_test','code','daytona','fixture',0,'{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null,"spot":false}',
                    '0.24','1.00',4000000000,'ended',1,'\(id)')
                """], executable: "/usr/bin/sqlite3")
            await checks.refresh(location: location)
            _ = try pane.inspect().find(text: "Ended rental")
        } catch { await server.stop(); throw error }
        await server.stop()
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
