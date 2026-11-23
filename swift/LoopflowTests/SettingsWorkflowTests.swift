#if os(macOS)
import AppKit
import Foundation
import Loopflow
import SwiftUI
import Testing
import ViewInspector
@testable import LoopflowMac

@Suite("Settings navigation and preferences")
@MainActor
struct SettingsWorkflowTests {
    @Test func sidebarSeparatesPersonalPreferencesFromProjectConfiguration() throws {
        var navigation = SettingsNavigation()
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        let window = SettingsWindow(navigation: Binding(get: { navigation }, set: { navigation = $0 }),
                                    appearance: appearance, checkout: nil, portfolio: PortfolioService())
        let split = try window.inspect().find(ViewType.NavigationSplitView.self)
        let sidebar = try split.sidebarView()
        #expect(try sidebar.findAll(ViewType.Label.self).map { try $0.title().find(ViewType.Text.self).string() } ==
            ["General", "Appearance", "Shortcuts", "Agents", "Machines", "Mobile", "Project & Wave Config"])
        #expect(sidebar.findAll(ViewType.Divider.self).count == 2)
        #expect(try split.detailView().find(ViewType.Toggle.self).labelView().text().string() == "Autosave Task Files")

        navigation.search = "  CuTs "
        let results = try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView()
        #expect(try results.find(text: "Shortcuts").string() == "Shortcuts")
        #expect(throws: (any Error).self) { try results.find(text: "Appearance") }
        try results.find(ViewType.TextField.self).callOnSubmit()
        let detail = try window.inspect().find(ViewType.NavigationSplitView.self).detailView()
        _ = try detail.find(KeyboardShortcutsView.self)
        navigation.open(.mobile)
        _ = try window.inspect().find(MobileSettings.self)
        navigation.open(.machines)
        _ = try window.inspect().find(ScopedSettings.self)
        navigation.open(.project)
        _ = try window.inspect().find(text: "Choose a checkout to edit its settings.")
        navigation.open(.appearance)
        #expect(navigation.search.isEmpty)
        _ = try window.inspect().find(ThemeChooser.self)
    }

    @Test func appSearchFindsAutosaveAppearanceAndPairingWithoutWaitingForTheCatalog() throws {
        var navigation = SettingsNavigation()
        let window = SettingsWindow(navigation: Binding(get: { navigation }, set: { navigation = $0 }),
                                    appearance: AppearanceStore(paths: .temporary(), systemDark: false),
                                    checkout: nil, portfolio: PortfolioService())
        for (query, result) in [("automatically", SettingsMatch.autosave), ("theme", .theme), ("QR", .mobileAccess)] {
            navigation.search = query
            let sidebar = try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView()
            try sidebar.find(ViewType.TextField.self).callOnSubmit()
            #expect(navigation.pane == result.pane)
            let detail = try window.inspect().find(ViewType.NavigationSplitView.self).detailView()
            _ = try detail.find(viewWithId: result.id)
        }
        navigation.search = "appear"
        navigation.move(1, in: navigation.matches(in: []))
        let sidebar = try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView()
        try sidebar.find(ViewType.TextField.self).callOnSubmit()
        #expect(navigation.highlightedKey == SettingsMatch.appearanceMode.id)

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
        let view = AutosaveToggle(notices: notices, undoManager: manager, defaults: defaults)
        let toggle = try view.inspect().find(ViewType.Toggle.self)
        #expect(try toggle.isOn())
        try toggle.tap()
        manager.endUndoGrouping()
        #expect(!defaults.savesTaskFilesAutomatically)
        #expect(try !AutosaveToggle(notices: notices, undoManager: manager, defaults: defaults).inspect().find(ViewType.Toggle.self).isOn())
        let undo = try #require(notices.current?.action)
        #expect(undo.title == "Undo")
        if keyboard { manager.undo() } else { undo.perform() }
        #expect(defaults.savesTaskFilesAutomatically)
        #expect(defaults.object(forKey: "taskFilesAutosave") == nil)
    }

    @Test func sharedCheckoutKeepsItsDestinationThroughValidationAndGlobalMachineEdits() async throws {
        let root = URL(fileURLWithPath: "/tmp/lf-form-\(UUID().uuidString.prefix(8))").resolvingSymlinksInPath()
        let repo = root.appending(path: "project")
        let home = root.appending(path: "home")
        for folder in [repo.appending(path: ".lf"), home] {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        }
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("chrome: null\n".utf8).write(to: repo.appending(path: ".lf/config.yaml"))
        let lf = try BuiltLf(home: home, cwd: repo)
        _ = try await lf.run(["desktop", "portfolio", "--json"])
        let server = LocalServerFixture(lf)
        let model = ScopedSettingsModel(location: server.location())
        model.select(checkout: repo.path)
        let notices = NoticeCenter()
        let watch = Task { await model.observe() }
        let personal = ScopedSettingsModel(location: server.location())
        let personalWatch = Task { await personal.observe() }
        var navigation = SettingsNavigation()
        let window = SettingsWindow(navigation: Binding(get: { navigation }, set: { navigation = $0 }),
            appearance: AppearanceStore(paths: .temporary(), systemDark: false), checkout: repo.path,
            portfolio: PortfolioService(), personal: personal, project: model)
        func pane(_ pane: SettingsPane) throws -> ScopedSettings {
            navigation.open(pane)
            return try window.inspect().find(ScopedSettings.self).actualView()
        }
        do {
            let deadline = ContinuousClock.now + .seconds(10)
            while model.file == nil || personal.file == nil {
                try #require(model.reading.failure == nil && ContinuousClock.now < deadline,
                             "\(model.reading.failure ?? "Waiting for the checkout")")
                try await Task.sleep(for: .milliseconds(20))
            }
            try pane(.project).inspect().find(ViewType.Picker.self).select(value: ConfigurationDestination.sharedRepository)
            let shared = try #require(model.file)
            let agents = try pane(.project).inspect()
            _ = try agents.find(text: "Effective agent order")
            let chrome = try agents.find(viewWithId: "chrome").find(ViewType.Picker.self)
            #expect(try chrome.selectedValue(ConfigurationScope.Authored.self) == .present(.null))

            #expect(await !model.apply(.set("no-such-graph"), key: "wave.graph", to: shared, notices: notices, manager: nil))
            let graph = try pane(.project).inspect().find(viewWithId: "wave.graph")
            let typo = try #require(model.errors[.init(shared, "wave.graph")])
            _ = try graph.find(text: typo.message)
            #expect(typo.recovery == nil)
            #expect(try !graph.find(ViewType.TextField.self).isDisabled())

            _ = try agents.find(button: "Switch Save in to My repository settings")
            _ = try agents.find(button: "Open file…")
            let config = try shared.prepareForOpening()
            #expect(try String(contentsOf: config, encoding: .utf8) == "chrome: null\n")
            let global = try #require(personal.file)
            let globalURL = try global.prepareForOpening()
            #expect(globalURL.resolvingSymlinksInPath() == home.appending(path: "config.yaml").resolvingSymlinksInPath())
            #expect(FileManager.default.fileExists(atPath: globalURL.path))
            navigation.search = "logs.linear_requests"
            let logResult = try window.inspect().find(viewWithId: "logs").find(ViewType.Toggle.self)
            #expect(try !logResult.isOn())
            try logResult.tap()
            let logDeadline = ContinuousClock.now + .seconds(10)
            while personal.file?.slots["logs"]?.declared.value?["linear_requests"] != .bool(true) {
                try #require(ContinuousClock.now < logDeadline)
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(try pane(.general).inspect().find(viewWithId: "logs").find(ViewType.Toggle.self).isOn())
            navigation.search = "yolo"
            let results = try window.inspect().find(ViewType.NavigationSplitView.self).sidebarView()
            let globalResult = try results.find(viewWithId: "yolo")
            _ = try globalResult.find(text: ConfigurationDestination.personalGlobal.title)
            try globalResult.find(ViewType.Button.self).tap()
            #expect(navigation.pane == .agents && navigation.highlightedKey == "yolo")
            navigation.search = "yolo"
            try results.find(viewWithId: "project.yolo").find(ViewType.Button.self).tap()
            #expect(navigation.pane == .project && navigation.highlightedKey == "yolo")
            #expect(model.file?.owner == .sharedRepository)
            let machines = try pane(.machines).inspect()
            _ = try machines.find(button: "Open ~/.lf/config.yaml")
            #expect(machines.findAll(ViewType.TextEditor.self).isEmpty)
            #expect(try pane(.agents).model.file?.owner == .personalGlobal)
            let hub = try machines.find(viewWithId: "hub").find(ScopedSettingRow.self).actualView()
            #expect(hub.file.owner == .personalGlobal)
            let machineID = "home_11111111111111111111111111111111"
            #expect(await personal.apply(.set(machineID), key: "hub", to: hub.file, notices: notices, manager: nil))
            #expect(model.destination == .sharedRepository)
            #expect(model.file?.hasOverrides == true)
            #expect(try String(contentsOf: home.appending(path: "config.yaml"), encoding: .utf8).contains(machineID))
            #expect(model.errors[.init(shared, "wave.graph")] != nil)
            #expect(await model.apply(.set("code"), key: "wave.graph", to: shared, notices: notices, manager: nil))
            #expect(model.errors.isEmpty)

            for enabled in [true, false] {
                let file = try #require(model.file)
                #expect(await model.apply(.set(String(enabled)), key: "chrome", to: file, notices: notices, manager: nil))
                let picker = try pane(.project).inspect().find(viewWithId: "chrome").find(ViewType.Picker.self)
                #expect(try picker.selectedValue(ConfigurationScope.Authored.self) == .present(.bool(enabled)))
            }
            let onDisk = try #require(model.file)
            #expect(await model.apply(.set("null"), key: "chrome", to: onDisk, notices: notices, manager: nil))
            #expect(model.file?.slots["chrome"]?.declared == .present(.null))
            let fresh = try #require(model.file)
            #expect(await model.apply(.reset, key: "chrome", to: fresh, notices: notices, manager: nil))
            #expect(model.file?.slots["chrome"]?.declared == .absent)

            // An unset control shows the value in effect and where it comes from.
            // A null default names what decides instead, from the catalogue.
            _ = try pane(.project).inspect().find(viewWithId: "chrome").find(text: "Claude's own setting (Default)")
            #expect(try pane(.general).inspect().find(viewWithId: "repo_root").find(ScopedSettingRow.self).actualView().unsetText == "~/src (Default)")
            let agentsPane = try pane(.agents)
            #expect(try agentsPane.inspect().find(viewWithId: "default_agent").find(ScopedSettingRow.self).actualView().unsetText == "None (Default)")
            #expect(try agentsPane.inspect().find(viewWithId: "yolo").find(ScopedSettingRow.self).actualView().unsetText == "On (Default)")
            for settings in [SettingsPane.general, .agents, .machines, .project] {
                let rows = try pane(settings).inspect().findAll(ScopedSettingRow.self).map { try $0.actualView() }
                #expect(!rows.isEmpty)
                #expect(rows.allSatisfy { !$0.unsetText.contains("Inherited") })
                #expect(throws: (any Error).self) { try pane(settings).inspect().find(text: "Inherited") }
            }
        } catch {
            watch.cancel()
            await watch.value
            personalWatch.cancel()
            await personalWatch.value
            await server.stop()
            throw error
        }
        watch.cancel()
        await watch.value
        personalWatch.cancel()
        await personalWatch.value
        await server.stop()
    }

    @Test(.requiresDisplay) func searchScrollsBetweenProjectSectionsInACompactWindow() async throws {
        try await withThemeServer { location, paths in
            _ = NSApplication.shared
            let model = ScopedSettingsModel(location: location)
            model.select(checkout: paths.config.deletingLastPathComponent().path)
            let notices = NoticeCenter()
            func pane(_ key: String?) -> ScopedSettings {
                ScopedSettings(model: model, pane: .project, checkouts: [], waves: [], notices: notices, highlightedKey: key)
            }
            let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 520, height: 240),
                                  styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.alphaValue = 0
            let host = NSHostingView(rootView: pane(nil))
            window.contentView = host
            window.orderBack(nil)
            defer { window.contentView = nil; window.close() }

            @MainActor func scrollView(in view: NSView) -> NSScrollView? {
                if let scroll = view as? NSScrollView { return scroll }
                return view.subviews.lazy.compactMap { scrollView(in: $0) }.first
            }
            @MainActor func waitFor(_ condition: @MainActor () -> Bool) async throws {
                let deadline = ContinuousClock.now + .seconds(5)
                while !condition() {
                    try #require(ContinuousClock.now < deadline)
                    try await Task.sleep(for: .milliseconds(20))
                    window.layoutIfNeeded()
                }
            }
            let watch = Task { await model.observe() }
            do {
                try await waitFor {
                    guard let scroll = scrollView(in: host), let document = scroll.documentView else { return false }
                    return model.file != nil && document.bounds.height > scroll.contentSize.height
                }
                let scroll = try #require(scrollView(in: host))
                let document = try #require(scroll.documentView)
                host.rootView = pane("configuration-file")
                try await waitFor { scroll.documentVisibleRect.maxY >= document.bounds.maxY - 45 }
                host.rootView = pane("agent")
                try await waitFor { scroll.documentVisibleRect.maxY < document.bounds.midY + scroll.contentSize.height / 2 }
            } catch {
                watch.cancel()
                await watch.value
                throw error
            }
            watch.cancel()
            await watch.value
        }
    }

    @Test func machineRowsShowTheServedIdentityCheckAndEndedRental() async throws {
        let wire = try JSONDecoder().decode([String: JSONValue].self, from: loadFixtureData("machine_status.json"))
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
        do {
            let scope = try await ServerClient.read(.configScope(directory: nil, wave: nil, target: .general), location: location)
                .decode(ConfigurationScope.self)
            let declaration = try #require(scope.files.first?.slots["machines"]?.declared)
            let pane = MachineConnections(location: location, declaration: declaration, connections: checks)
            _ = try MachineConnectionRow(name: "builder", health: .checking).inspect().find(text: "Checking…")
            await checks.refresh(declaration, location: location)
            _ = try pane.inspect().find(text: "builder")
            _ = try pane.inspect().find(text: "Reachable")
            try answering("home_22222222222222222222222222222222")
            await checks.refresh(declaration, location: location)
            let wrongHost = try pane.inspect()
            _ = try wrongHost.find(text: "Connection error")
            #expect(try wrongHost.findAll(ViewType.Text.self).contains { try $0.string().contains("identity changed") })
            _ = try await lf.run([root.appending(path: "loopflow.db").path, """
                INSERT INTO rentals(id,ondemand,provider,provider_name,by_hand,hardware,usd_per_hour,cap_usd,deadline,state,created_at,machine_id)
                VALUES('rent_test','code','daytona','fixture',0,'{"vcpus":4,"memory_gb":8,"disk_gb":10,"gpu":null,"spot":false}',
                    '0.24','1.00',4000000000,'ended',1,'\(id)')
                """], executable: "/usr/bin/sqlite3")
            await checks.refresh(declaration, location: location)
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
