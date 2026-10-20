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

    #if canImport(GhosttyKit)
    @Test("Navigation and retained work survive preview, Cancel and Apply", arguments: ThemeFixture.allCases)
    func themeChangesKeepHiddenWorkAndFutureNativeViewsCoherent(theme: ThemeFixture) async throws {
        let paths = ThemePaths.temporary()
        let applied = theme == .dark ? ThemeDefinition.light : ThemeDefinition.dark
        try paths.writeConfig("theme: \(applied.name)\n")
        let manager = GhosttyManager.shared
        let previous = manager.theme
        defer { try? manager.applyTheme(previous) }
        let appearance = AppearanceStore(paths: paths, systemDark: false,
                                         publish: { try manager.applyTheme($0, retaining: $1) })
        let fixtureURL = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/work_first_entry.json")
        let fixture = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as? [String: Any])
        let roadmapData = try JSONSerialization.data(withJSONObject: #require(fixture["roadmap"]))
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: roadmapData)
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first)
        var wire = try #require(fixture["task_primary"] as? [String: Any])
        wire["availability"] = "available"
        // Retain a provider conversation without executing the capture's historical argv.
        wire["open_argv"] = ["must-not-launch"]
        let recordData = try JSONSerialization.data(withJSONObject: wire)
        let record = try JSONDecoder().decode(SessionRecord.self, from: recordData)
        let recordJSON = String(decoding: recordData, as: UTF8.self)
        let roadmapJSON = String(decoding: roadmapData, as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "wave" where args.dropFirst().first == "show": return roadmapJSON
            case "wave" where args.dropFirst().first == "list": return "[]"
            case "session" where args.dropFirst().first == "list": return "{\"entries\":[\(recordJSON)],\"next\":null}"
            case "session": return recordJSON
            case "history": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected operation in theme navigation: \(args)")
            }
        }
        let location = try #require(record.workspace?.identity)
        let registry = SessionsWorkspaceRegistry(localMachineId: location.machineId)
        let model = WorkModel(query: query, repoPath: wave.wave.repo)
        await model.refresh()
        model.bindWorkspaces(registry, repository: "repo")
        model.navigation.showsNavigator = true
        let repoKey = try #require(registry.selectedWork)
        let repoWork = try #require(registry.retained(repoKey))
        model.select(.wave(id: wave.wave.id))
        let waveKey = try #require(registry.selectedWork)
        let waveWork = try #require(registry.retained(waveKey))
        model.select(.task(id: task.id))
        let key = try #require(registry.selectedWork)
        let work = try #require(registry.retained(key))
        registry.revealSession(record, in: work)
        let conversation = try #require(work.multiplexer.pane(forSessionId: record.id))
        let sessionNative = registry.surfaces.view(for: .session(record.id, machineId: location.machineId))
        let shell = try #require(try work.newShell())
        let identity = try #require(shell.terminal)
        let native = registry.surfaces.view(for: identity)
        _ = work.multiplexer.newShell(location: location)
        work.multiplexer.setFocusedPane(shell.id)
        let files = work.files(issue: task.task.identifier, cwd: location.worktree)
        let document = files.document("draft.md")
        document.editor.string = "unsaved draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 2))
        let layout = work.multiplexer.layout
        let selectedSession = model.navigation.selectedSessionId
        let store = work.sessionStore(repoPath: wave.wave.repo, query: query)
        let member = fixtureViewContent(work: key, title: task.task.name)
        model.applySelectionPlans([
            fixtureViewPlan(owner: repoKey, members: [member, fixtureViewContent(work: waveKey,
                title: wave.wave.name)]),
            fixtureViewPlan(owner: waveKey, members: [member])
        ], reason: "")
        func exerciseNavigation() throws {
            let navigator = WorkNavigator(model: model, onOpenSession: { model.openSession($0, store: store) })
                .modifier(AppAppearance(appearance: appearance))
            model.navigation.resizeSidebar(460, availableWidth: 1200)
            model.navigation.toggleNavigator()
            #expect(!model.navigation.showsNavigator)
            #expect(registry.selectedWork == key)
            model.navigation.toggleNavigator()
            #expect(model.navigation.showsNavigator)
            #expect(model.navigation.sidebarWidth == 460)
            model.select(nil)
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-wave-\(wave.wave.id)").button().tap()
            #expect(model.selection == .wave(id: wave.wave.id))
            #expect(registry.retained(waveKey) === waveWork)
            model.select(nil)
            #expect(registry.selectedWork == repoKey)
            #expect(registry.retained(repoKey) === repoWork)
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-task-\(task.id)").button().tap()
            #expect(model.selection == .task(id: task.id))
            #expect(model.navigation.selectedSessionId == selectedSession)
            #expect(work.multiplexer.focusedPaneId == shell.id)
            #expect(work.multiplexer.pane(forSessionId: record.id)?.id == conversation.id)
            model.goBack()
            #expect(model.selection == nil)
            model.select(nil)
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-wave-\(wave.wave.id)").button().tap()
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-task-\(task.id)").button().tap()
            work.multiplexer.setFocusedPane(shell.id)
            #expect(registry.selectedWork == key)
            #expect(registry.retained(key) === work)
            #expect(registry.surfaces.view(for: identity) === native)
            #expect(registry.surfaces.view(for: .session(record.id, machineId: location.machineId)) === sessionNative)
            #expect(work.multiplexer.layout == layout)
            #expect(work.multiplexer.focusedPaneId == shell.id)
            #expect(files.document("draft.md") === document)
            #expect(document.editor.string == "unsaved draft")
            #expect(document.editor.selectedRange() == NSRange(location: 3, length: 2))
            for view in [native, sessionNative] {
                #expect(view.layer?.backgroundColor == appearance.effectiveTheme.terminal.background.nsColor.cgColor)
                #expect(view.searchBar.appearance?.name == (appearance.effectiveTheme.isDark ? .darkAqua : .aqua))
                #expect(view.surface == nil) // No native client is launched by a theme or navigation inspection.
            }
        }

        model.select(nil) // The Task and its native views are hidden during publication.
        try theme.preview(in: appearance)
        #expect(registry.selectedWork == repoKey)
        #expect(work.multiplexer.focusedPaneId == shell.id)
        #expect(work.multiplexer.layout == layout)
        let candidate = appearance.effectiveTheme
        #expect(AppearanceStore(paths: paths, systemDark: false).effectiveTheme.name == applied.name)
        model.select(.task(id: task.id))
        try exerciseNavigation()
        let focusBeforeCancel = work.multiplexer.focusedPaneId
        appearance.cancel()
        #expect(work.multiplexer.focusedPaneId == focusBeforeCancel)
        #expect(appearance.effectiveTheme.name == applied.name)
        #expect(appearance.library.yours.isEmpty)
        try exerciseNavigation()
        model.select(.wave(id: wave.wave.id))
        let outerLayout = registry.layout
        try theme.preview(in: appearance)
        let focusBeforeApply = work.multiplexer.focusedPaneId
        appearance.apply()
        #expect(work.multiplexer.focusedPaneId == focusBeforeApply)
        #expect(registry.layout == outerLayout)
        #expect(registry.selectedWork == waveKey)
        #expect(registry.openWork == [waveKey])
        #expect(appearance.effectiveTheme.terminal == candidate.terminal)
        model.select(key.reference)
        try exerciseNavigation()
        let restored = AppearanceStore(paths: paths, systemDark: false)
        #expect(restored.effectiveTheme.terminal == manager.theme.terminal)
        let future = registry.surfaces.view(for: .shell("future", machineId: location.machineId))
        #expect(future.layer?.backgroundColor == native.layer?.backgroundColor)
        #expect(future.appearance?.name == native.appearance?.name)
        #expect(future.searchBar.appearance?.name == native.searchBar.appearance?.name)
        #expect(future.searchBar.layer?.backgroundColor == native.searchBar.layer?.backgroundColor)
        #expect(future.surface == nil)
    }

    #endif

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
