import Testing
import Foundation
@testable import LoopflowCore

@Suite("MultiplexerLayout")
struct MultiplexerLayoutTests {

    @Test("default layout is a single terminal leaf")
    func defaultLayout() {
        let layout = LayoutNode.defaultLayout()
        let panes = layout.allPanes
        #expect(panes.count == 1)
        #expect(panes.first?.type == .terminal)
    }

    @Test("splitting a leaf produces a split with two leaves")
    func splitLeaf() {
        let layout = LayoutNode.defaultLayout()
        let originalId = layout.allPanes.first!.id
        let newPane = PaneState(type: .markdown)
        let split = layout.splitting(originalId, axis: .horizontal, newPane: newPane)

        let panes = split.allPanes
        #expect(panes.count == 2)
        #expect(panes[0].id == originalId)
        #expect(panes[0].type == .terminal)
        #expect(panes[1].id == newPane.id)
        #expect(panes[1].type == .markdown)
    }

    @Test("removing one pane from a split collapses to the remaining leaf")
    func removeCollapses() {
        let paneA = PaneState(type: .terminal)
        let paneB = PaneState(type: .diff)
        let layout = LayoutNode.split(.horizontal, first: .leaf(paneA), second: .leaf(paneB), ratio: 0.5)

        let result = layout.removing(paneB.id)
        #expect(result != nil)
        #expect(result?.allPanes.count == 1)
        #expect(result?.allPanes.first?.id == paneA.id)
    }

    @Test("removing the only pane returns nil")
    func removeOnlyPane() {
        let layout = LayoutNode.defaultLayout()
        let onlyId = layout.allPanes.first!.id
        #expect(layout.removing(onlyId) == nil)
    }

    @Test("nextPane wraps around")
    func nextPaneWraps() {
        let paneA = PaneState(type: .terminal)
        let paneB = PaneState(type: .markdown)
        let layout = LayoutNode.split(.horizontal, first: .leaf(paneA), second: .leaf(paneB), ratio: 0.5)

        #expect(layout.nextPane(after: paneA.id)?.id == paneB.id)
        #expect(layout.nextPane(after: paneB.id)?.id == paneA.id)
    }

    @Test("previousPane wraps around")
    func previousPaneWraps() {
        let paneA = PaneState(type: .terminal)
        let paneB = PaneState(type: .markdown)
        let layout = LayoutNode.split(.horizontal, first: .leaf(paneA), second: .leaf(paneB), ratio: 0.5)

        #expect(layout.previousPane(before: paneB.id)?.id == paneA.id)
        #expect(layout.previousPane(before: paneA.id)?.id == paneB.id)
    }

    @Test("updating pane config preserves tree structure")
    func updateConfig() {
        let pane = PaneState(type: .markdown)
        let layout = LayoutNode.leaf(pane)
        let newConfig = PaneConfig(filePath: "scratch/pane-routing-spec.md")
        let updated = layout.updatingPane(pane.id, config: newConfig)

        let result = updated.allPanes.first
        #expect(result?.config.filePath == "scratch/pane-routing-spec.md")
        #expect(result?.type == .markdown)
    }

    @Test("deep split operations work correctly")
    func deepSplit() {
        let paneA = PaneState(type: .terminal)
        let paneB = PaneState(type: .diff)
        let paneC = PaneState(type: .markdown)

        var layout = LayoutNode.split(.horizontal, first: .leaf(paneA), second: .leaf(paneB), ratio: 0.5)
        layout = layout.splitting(paneB.id, axis: .vertical, newPane: paneC)

        #expect(layout.allPanes.count == 3)
        #expect(layout.pane(for: paneA.id) != nil)
        #expect(layout.pane(for: paneB.id) != nil)
        #expect(layout.pane(for: paneC.id) != nil)

        let afterRemove = layout.removing(paneB.id)!
        #expect(afterRemove.allPanes.count == 2)
        #expect(afterRemove.pane(for: paneA.id) != nil)
        #expect(afterRemove.pane(for: paneC.id) != nil)
    }

    @Test("updating ratio changes the split that contains the target pane")
    func updateRatioContainingPane() {
        let paneA = PaneState(type: .terminal)
        let paneB = PaneState(type: .diff)
        let paneC = PaneState(type: .markdown)

        let nested = LayoutNode.split(.vertical, first: .leaf(paneB), second: .leaf(paneC), ratio: 0.25)
        let layout = LayoutNode.split(.horizontal, first: .leaf(paneA), second: nested, ratio: 0.5)
        let updated = layout.updatingRatio(containing: paneC.id, ratio: 0.8)

        guard case .split(_, _, let updatedNested, let outerRatio) = updated else {
            Issue.record("expected outer split")
            return
        }
        guard case .split(_, _, _, let nestedRatio) = updatedNested else {
            Issue.record("expected nested split")
            return
        }

        #expect(outerRatio == 0.5)
        #expect(nestedRatio == 0.8)
    }

    @Test("layout round-trips through JSON")
    func codable() throws {
        let paneA = PaneState(
            id: "a",
            type: .terminal,
            config: PaneConfig(
                terminalSessionName: "lf-wave-a",
                launchCommand: "lf design && lf op commit --push"
            )
        )
        let paneB = PaneState(id: "b", type: .markdown, config: PaneConfig(filePath: "wave/test/README.md"))
        let layout = LayoutNode.split(.horizontal, first: .leaf(paneA), second: .leaf(paneB), ratio: 0.6)

        let data = try JSONEncoder().encode(layout)
        let decoded = try JSONDecoder().decode(LayoutNode.self, from: data)

        #expect(decoded.allPanes.count == 2)
        #expect(decoded.allPanes[0].id == "a")
        #expect(decoded.allPanes[0].type == .terminal)
        #expect(decoded.allPanes[0].config.terminalSessionName == "lf-wave-a")
        #expect(decoded.allPanes[0].config.launchCommand == "lf design && lf op commit --push")
        #expect(decoded.allPanes[1].id == "b")
        #expect(decoded.allPanes[1].config.filePath == "wave/test/README.md")
    }
}

@Suite("MultiplexerStore")
struct MultiplexerStoreTests {

    @MainActor
    private func makeStore() -> MultiplexerStore {
        MultiplexerStore(userDefaults: UserDefaults(suiteName: "test-\(UUID().uuidString)")!)
    }

    private func makeWave(id: String = "wave-1") -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: "Demo Wave",
                repo: "/tmp/repo",
                flow: "build",
                direction: [],
                area: ["."]
            )
        )
    }

    @Test("default layout for unknown wave is a single terminal")
    @MainActor
    func defaultForUnknownWave() {
        let store = makeStore()
        let layout = store.layout(for: "wave-1")
        #expect(layout.allPanes.count == 1)
        #expect(layout.allPanes.first?.type == .terminal)
        #expect(layout.allPanes.first?.config.terminalSessionName == "lf-wave-1-\(layout.allPanes.first?.id ?? "")")
    }

    @Test("workspace default layout uses roadmap runs and terminal panes")
    @MainActor
    func workspaceDefaultLayout() {
        let store = makeStore()
        let wave = makeWave()
        let layout = store.layout(for: wave)
        let paneTypes = layout.allPanes.map(\.type)

        #expect(layout.allPanes.count == 3)
        #expect(paneTypes.contains(.roadmap))
        #expect(paneTypes.contains(.runs))
        #expect(paneTypes.contains(.terminal))
        #expect(store.pane(ofType: .terminal, for: wave.id)?.config.terminalSessionName?.hasPrefix("lf-\(wave.id)-") == true)
    }

    @Test("split creates new pane and focuses it")
    @MainActor
    func splitAndFocus() {
        let store = makeStore()
        let original = store.layout(for: "wave-1").firstPane!

        let newPane = store.splitPane(original.id, axis: .horizontal, newPaneType: .diff, for: "wave-1")

        #expect(store.layout(for: "wave-1").allPanes.count == 2)
        #expect(store.focusedPaneId(for: "wave-1") == newPane.id)
        #expect(store.layout(for: "wave-1").allPanes.first(where: { $0.type == .terminal })?.id == original.id)
    }

    @Test("terminal split creates a second terminal with its own tmux session")
    @MainActor
    func splitTerminalCreatesDistinctSession() {
        let store = makeStore()
        let terminal = store.layout(for: "wave-1").firstPane!

        let newPane = store.splitPane(terminal.id, axis: .horizontal, newPaneType: .terminal, for: "wave-1")

        #expect(store.layout(for: "wave-1").allPanes.count == 2)
        #expect(newPane.type == .terminal)
        #expect(newPane.config.terminalSessionName == "lf-wave-1-\(newPane.id)")
        #expect(terminal.config.terminalSessionName == "lf-wave-1-\(terminal.id)")
        #expect(newPane.config.terminalSessionName != terminal.config.terminalSessionName)
    }

    @Test("closing a non-terminal pane keeps the terminal")
    @MainActor
    func closeMoveFocus() {
        let store = makeStore()
        let terminal = store.layout(for: "wave-1").firstPane!
        let newPane = store.splitPane(terminal.id, axis: .horizontal, newPaneType: .markdown, for: "wave-1")

        _ = store.closePane(newPane.id, for: "wave-1")

        #expect(store.layout(for: "wave-1").allPanes.count == 1)
        #expect(store.focusedPaneId(for: "wave-1") == terminal.id)
        #expect(store.layout(for: "wave-1").allPanes.first?.type == .terminal)
    }

    @Test("closing one terminal pane preserves the others")
    @MainActor
    func closeOneTerminalPane() {
        let store = makeStore()
        let terminal = store.layout(for: "wave-1").firstPane!
        let secondTerminal = store.splitPane(terminal.id, axis: .horizontal, newPaneType: .terminal, for: "wave-1")

        let closed = store.closePane(terminal.id, for: "wave-1")

        #expect(closed?.id == terminal.id)
        #expect(store.layout(for: "wave-1").allPanes.count == 1)
        #expect(store.layout(for: "wave-1").allPanes.first?.id == secondTerminal.id)
    }

    @Test("wave layouts are independent")
    @MainActor
    func independentWaves() {
        let store = makeStore()

        let pane1 = store.layout(for: "wave-1").firstPane!
        _ = store.splitPane(pane1.id, axis: .horizontal, newPaneType: .markdown, for: "wave-1")

        #expect(store.layout(for: "wave-1").allPanes.count == 2)
        #expect(store.layout(for: "wave-2").allPanes.count == 1)
        #expect(store.layout(for: "wave-2").allPanes.first?.type == .terminal)
        #expect(store.layout(for: "wave-1").firstPane?.config.terminalSessionName != store.layout(for: "wave-2").firstPane?.config.terminalSessionName)
    }

    @Test("focus cycles through panes")
    @MainActor
    func focusCycle() {
        let store = makeStore()
        let paneA = store.layout(for: "w").firstPane!
        let paneB = store.splitPane(paneA.id, axis: .horizontal, newPaneType: .diff, for: "w")

        #expect(store.focusedPaneId(for: "w") == paneB.id)

        store.moveFocus(.next, for: "w")
        #expect(store.focusedPaneId(for: "w") == paneA.id)

        store.moveFocus(.next, for: "w")
        #expect(store.focusedPaneId(for: "w") == paneB.id)
    }

    @Test("terminal session names are reported for a wave")
    @MainActor
    func terminalSessionNames() {
        let store = makeStore()
        let terminal = store.layout(for: "wave-1").firstPane!
        let secondTerminal = store.splitPane(terminal.id, axis: .horizontal, newPaneType: .terminal, for: "wave-1")

        #expect(Set(store.terminalSessionNames(for: "wave-1")) == [
            "lf-wave-1-\(terminal.id)",
            "lf-wave-1-\(secondTerminal.id)",
        ])
    }

    @Test("replacing a launchpad with a terminal preserves pane id and assigns session")
    @MainActor
    func replacePaneWithTerminal() {
        let store = makeStore()
        let terminal = store.layout(for: "wave-1").firstPane!
        let launchpad = store.splitPane(terminal.id, axis: .horizontal, newPaneType: .launchpad, for: "wave-1")

        let replacement = store.replacePane(
            launchpad.id,
            with: .terminal,
            config: PaneConfig(launchCommand: "lf design && lf op commit --push"),
            for: "wave-1"
        )

        #expect(replacement?.id == launchpad.id)
        #expect(replacement?.type == .terminal)
        #expect(replacement?.config.terminalSessionName == "lf-wave-1-\(launchpad.id)")
        #expect(replacement?.config.launchCommand == "lf design && lf op commit --push")
        #expect(store.layout(for: "wave-1").pane(for: launchpad.id)?.type == .terminal)
    }
}
