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
    #if canImport(GhosttyKit)
    @Test("Navigation and retained work survive preview, Cancel and Apply", arguments: ThemeFixture.allCases)
    func themeChangesKeepHiddenWorkAndFutureNativeViewsCoherent(theme: ThemeFixture) async throws {
        let suite = "theme-workflow-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let appliedID = theme == .dark ? "light" : "dark"
        defaults.set(appliedID, forKey: "appearanceMode")
        let manager = GhosttyManager.shared
        let previous = manager.theme
        defer { try? manager.applyTheme(previous) }
        let appearance = AppearanceStore(defaults: defaults, systemDark: false,
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
        wire["attention"] = "waiting"
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
        let identity = TerminalIdentity.shell(shell.id, machineId: location.machineId)
        let native = registry.surfaces.view(for: identity)
        _ = work.multiplexer.split(shell.id, axis: .vertical, location: location)
        work.multiplexer.setFocusedPane(shell.id)
        let files = work.files(taskId: task.id, issue: task.task.identifier, cwd: location.worktree)
        let document = files.document("draft.md")
        document.editor.string = "unsaved draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 2))
        let layout = work.multiplexer.layout
        let store = work.sessionStore(repoPath: wave.wave.repo, query: query)
        let content = SessionsContentView(model: model, repoPath: wave.wave.repo, workspaces: registry,
                                          machineId: location.machineId, repository: "repo", query: query)
            .modifier(AppAppearance(appearance: appearance))

        func exerciseNavigation() throws {
            let navigator = WorkNavigator(model: model, onOpenSession: { model.openSession($0, store: store) })
                .modifier(AppAppearance(appearance: appearance))
            model.navigation.resizeSidebar(460, availableWidth: 1200)
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-toggle-sidebar").button().tap()
            #expect(!model.navigation.showsNavigator)
            #expect(registry.selectedWork == key)
            try content.inspect().find(viewWithAccessibilityIdentifier: "work-toggle-sidebar").button().tap()
            #expect(model.navigation.showsNavigator)
            #expect(model.navigation.sidebarWidth == 460)
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-wave-conversation-\(wave.wave.id)").button().tap()
            #expect(model.selection == .wave(id: wave.wave.id))
            #expect(registry.retained(waveKey) === waveWork)
            model.select(nil)
            #expect(registry.selectedWork == repoKey)
            #expect(registry.retained(repoKey) === repoWork)
            try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-attention-task-\(task.id)").button().tap()
            #expect(model.selection == .task(id: task.id))
            #expect(model.navigation.selectedSessionId == record.id)
            #expect(work.multiplexer.focusedPaneId == conversation.id)
            #expect(work.multiplexer.pane(forSessionId: record.id)?.id == conversation.id)
            model.goBack()
            #expect(model.selection == nil)
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
        #expect(AppearanceStore(defaults: defaults, systemDark: false).selection.mode.rawValue == appliedID)
        model.select(.task(id: task.id))
        try exerciseNavigation()
        let focusBeforeCancel = work.multiplexer.focusedPaneId
        appearance.cancel()
        #expect(work.multiplexer.focusedPaneId == focusBeforeCancel)
        #expect(appearance.effectiveTheme.id == appliedID)
        #expect(appearance.imports.isEmpty)
        try exerciseNavigation()
        model.select(.wave(id: wave.wave.id), besideFocused: true)
        let outerLayout = registry.layout
        try theme.preview(in: appearance)
        let focusBeforeApply = work.multiplexer.focusedPaneId
        appearance.apply()
        #expect(work.multiplexer.focusedPaneId == focusBeforeApply)
        #expect(registry.layout == outerLayout)
        #expect(registry.selectedWork == waveKey)
        #expect(registry.openWork == [key, waveKey])
        #expect(appearance.effectiveTheme.terminal == candidate.terminal)
        model.hideWorkspace(waveKey)
        try exerciseNavigation()
        let restored = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(restored.effectiveTheme.terminal == manager.theme.terminal)
        let future = registry.surfaces.view(for: .shell("future", machineId: location.machineId))
        #expect(future.layer?.backgroundColor == native.layer?.backgroundColor)
        #expect(future.appearance?.name == native.appearance?.name)
        #expect(future.searchBar.appearance?.name == native.searchBar.appearance?.name)
        #expect(future.searchBar.layer?.backgroundColor == native.searchBar.layer?.backgroundColor)
        #expect(future.surface == nil)
    }

    #endif

    @Test func chooserAssignsBothSlotsWithoutChangingMode() throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let appearance = AppearanceStore(defaults: defaults, systemDark: false)
        appearance.beginPreview()
        let view = ThemeChooser(appearance: appearance)
        let mode = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-mode").picker()
        for choice in [AppearanceMode.light, .dark, .system] {
            try mode.select(value: choice)
            for slot in [AppearanceMode.light, .dark] {
                let button = try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-slot-\(slot.rawValue)").button()
                #expect(button.isDisabled() == (choice != .system && choice != slot))
            }
            #expect(appearance.editingSlot == (choice == .dark ? .dark : .light))
        }
        try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-bundled:TokyoNight").button().tap()
        try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-slot-dark").button().tap()
        try view.inspect().find(viewWithAccessibilityIdentifier: "appearance-theme-bundled:Gruvbox Light").button().tap()
        #expect(appearance.current.mode == .system)
        try view.inspect().find(button: "Apply").tap()
        #expect(appearance.effectiveTheme.id == "bundled:TokyoNight")
        appearance.setSystemDark(true)
        #expect(appearance.effectiveTheme.id == "bundled:Gruvbox Light")
        let restored = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(restored.selection == appearance.selection)
        #expect(restored.effectiveTheme.id == "bundled:TokyoNight")
    }

    @Test func chooserUsesPreviewAndKeepsApplyDisabledForUnreadableText() throws {
        let appearance = AppearanceStore(defaults: UserDefaults(suiteName: UUID().uuidString)!, systemDark: false)
        appearance.beginPreview()
        let view = ThemeChooser(appearance: appearance)
        let dimWarning = try #require(appearance.selectedTheme.terminal.dimWarnings.first)
        #expect(try view.inspect().find(text: dimWarning).string() == dimWarning)
        #expect(try !view.inspect().find(button: "Apply").isDisabled())
        try view.inspect().find(button: "Cancel").tap()
        #expect(!appearance.chooserOpen)
        #expect(appearance.effectiveTheme.id == "light")
        appearance.beginPreview(); appearance.previewMode(.dark)
        try view.inspect().find(button: "Apply").tap()
        #expect(appearance.selection.mode == .dark)
        let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: source) }
        try Data("background=777777\nforeground=888888".utf8).write(to: source)
        appearance.beginPreview(); appearance.importTheme(source)
        #expect(try view.inspect().find(button: "Apply").isDisabled())
        #expect(appearance.effectiveTheme.id == "dark")
        #expect(appearance.notice?.contains("Default text") == true)
        appearance.editSlot(.light)
        #expect(try view.inspect().find(button: "Apply").isDisabled())
        let error = try #require(appearance.assignmentErrors.first)
        #expect(error.hasPrefix("Dark theme:"))
        #expect(try view.inspect().find(text: error).string() == error)
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
            appearance.previewTheme("bundled:TokyoNight")
        case .lightImport, .darkImport:
            appearance.previewMode(self == .lightImport ? .light : .dark)
            let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            defer { try? FileManager.default.removeItem(at: source) }
            let colors = self == .lightImport ? "background=fff4df\nforeground=34291f" : "background=171924\nforeground=CAD2EF"
            try Data(colors.utf8).write(to: source)
            appearance.importTheme(source)
        }
        #expect(appearance.notice == nil)
    }
}
#endif
