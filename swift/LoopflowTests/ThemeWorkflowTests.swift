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
    @Test func themeChangesKeepHiddenWorkAndFutureNativeViewsCoherent() throws {
        let defaults = UserDefaults(suiteName: "theme-workflow-\(UUID().uuidString)")!
        defaults.set("dark", forKey: "appearanceMode")
        let manager = GhosttyManager.shared
        let previous = manager.theme
        defer { try? manager.applyTheme(previous) }
        let appearance = AppearanceStore(defaults: defaults, systemDark: false, publish: { try manager.applyTheme($0, retaining: $1) })
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let location = fixtureWorkspace("/theme-fixture")
        let keys = [WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo"),
                    WorkWorkspaceKey(repository: "repo", kind: .wave, id: "wave"),
                    WorkWorkspaceKey(repository: "repo", kind: .task, id: "task")]
        for key in keys { registry.open(key, title: key.id, location: location) }
        let work = try #require(registry.retained(keys[2]))
        let shell = try #require(try work.newShell())
        let identity = TerminalIdentity.shell(shell.id, machineId: fixtureMachineId)
        let native = registry.surfaces.view(for: identity)
        _ = work.multiplexer.split(shell.id, axis: .vertical, location: location)
        work.multiplexer.setFocusedPane(shell.id)
        let document = work.files(taskId: "task", issue: "TEST", cwd: location.worktree).document("draft.md")
        document.editor.string = "unsaved draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 2))
        let layout = work.multiplexer.layout
        let focused = work.multiplexer.focusedPaneId
        let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: source) }
        try Data("background=fff4df\nforeground=34291f".utf8).write(to: source)
        registry.open(keys[0], title: "Repo", location: location)
        appearance.beginPreview(); appearance.importTheme(source)
        #expect(appearance.effectiveTheme.terminal.background.hex == 0xFFF4DF)
        #expect(native.layer?.backgroundColor == appearance.effectiveTheme.terminal.background.nsColor.cgColor)
        #expect(native.searchBar.appearance?.name == .aqua)
        #expect(native.surface == nil) // Appearance never launches a client.
        for key in [keys[1], keys[0], keys[2]] { registry.open(key, title: key.id, location: location) }
        work.multiplexer.setCollapsed(paneId: shell.id, collapsed: true)
        work.multiplexer.setCollapsed(paneId: shell.id, collapsed: false)
        work.multiplexer.setFocusedPane(shell.id)
        appearance.cancel()
        #expect(native.layer?.backgroundColor == ThemeDefinition.dark.terminal.background.nsColor.cgColor)
        #expect(appearance.imports.isEmpty)
        try Data("background=171924\nforeground=CAD2EF".utf8).write(to: source)
        appearance.beginPreview(); appearance.importTheme(source); appearance.apply()
        let restored = AppearanceStore(defaults: defaults, systemDark: false)
        #expect(restored.effectiveTheme.terminal == manager.theme.terminal)
        let future = registry.surfaces.view(for: .shell("future", machineId: fixtureMachineId))
        #expect(future.layer?.backgroundColor == native.layer?.backgroundColor)
        #expect(registry.retained(keys[2]) === work)
        #expect(registry.surfaces.view(for: identity) === native)
        #expect(work.multiplexer.layout == layout)
        #expect(work.multiplexer.focusedPaneId == focused)
        #expect(document.editor.string == "unsaved draft")
        #expect(document.editor.selectedRange() == NSRange(location: 3, length: 2))
        #expect(future.surface == nil)
    }

    #endif

    @Test func chooserUsesPreviewAndKeepsApplyDisabledForUnreadableText() throws {
        let appearance = AppearanceStore(defaults: UserDefaults(suiteName: UUID().uuidString)!, systemDark: false)
        appearance.beginPreview()
        let view = ThemeChooser(appearance: appearance)
        try view.inspect().find(button: "Cancel").tap()
        #expect(!appearance.chooserOpen)
        #expect(appearance.effectiveTheme.id == "light")
        appearance.beginPreview(); appearance.preview("dark")
        try view.inspect().find(button: "Apply").tap()
        #expect(appearance.selection == "dark")
        let source = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: source) }
        try Data("background=777777\nforeground=888888".utf8).write(to: source)
        appearance.beginPreview(); appearance.importTheme(source)
        #expect(try view.inspect().find(button: "Apply").isDisabled())
        #expect(appearance.effectiveTheme.id == "dark")
        #expect(appearance.notice?.contains("Default text") == true)
    }
}
#endif
