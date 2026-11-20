#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave backlog")
@MainActor
struct BacklogBoardTests {
    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private enum Holder { case you, someone, nobody }

    /// A backlog Task from the shared row fixture, in `wave` with that Wave's default flow.
    private func task(_ id: String, rank: Int, holder: Holder = .nobody, wave: String = "wave-a",
                      flow: String = "feature", brief: String = "", section: String = "backlog") throws -> RoadmapTask {
        let rows = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_history_rows.json"))) as? [[String: Any]])
        var row = rows[0]
        var plan = try #require(row["task"] as? [String: Any])
        plan["id"] = id
        plan["identifier"] = id
        plan["name"] = "Task \(id)"
        plan["rank"] = rank
        plan["description"] = brief
        plan["wave_id"] = wave
        plan["project_id"] = "project-\(wave)"
        plan["state"] = NSNull()
        plan["completed"] = false
        plan["completed_at"] = NSNull()
        row["task"] = plan
        row["section"] = section
        row["graph_name"] = flow
        row["owned_by_you"] = holder == .you
        let owner: Any = holder == .nobody ? NSNull() : [
            "person": holder == .you ? "You" : "Ada Lovelace", "person_id": NSNull(),
            "person_key": holder == .you ? "you@example.com" : "ada@example.com",
            "machine": ["id": NSNull(), "label": NSNull()]] as [String: Any]
        row["runtime"] = ["work_id": "work-\(id)", "status": "unstarted", "reason": "Not started",
                          "updated_at": "2026-11-19T12:00:00Z", "provider": NSNull(), "completion_pending": NSNull(),
                          "owner": owner] as [String: Any]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    /// Five backlog Tasks: one yours, one someone else's, three unclaimed; Wave A runs feature, Wave B code.
    private func tasks() throws -> [RoadmapTask] {
        [
            try task("A-1", rank: 0, holder: .you),
            try task("A-2", rank: 1, holder: .someone, brief: "Mentions the brief-word once."),
            try task("B-1", rank: 0, wave: "wave-b", flow: "code"),
            try task("A-3", rank: 2),
            try task("A-4", rank: 3),
            try task("A-DONE", rank: 4, section: "done"),
        ]
    }

    private func list(_ tasks: [RoadmapTask], wave: String? = "wave-a", _ change: (inout BacklogState) -> Void = { _ in })
        -> BacklogList {
        var state = BacklogState()
        change(&state)
        return BacklogList(tasks: tasks, truncated: false, wave: wave, defaultFlow: "feature",
                           waveNames: ["wave-a": "Product", "wave-b": "Infrastructure"], state: state)
    }

    @Test func filtersCountOwnershipRustDecided() throws {
        let list = list(try tasks())
        #expect(BacklogFilter.allCases.map { "\($0.rawValue) \(list.count($0))" }.joined(separator: " · ")
            == "All 5 · Mine 1 · Available 4")
        // Finished work is not backlog; rows keep rank order within each Project.
        #expect(list.ids == ["A-1", "B-1", "A-2", "A-3", "A-4"])
        #expect(list.waveLabel(list.rows[1]) == "Infrastructure" && list.waveLabel(list.rows[0]) == nil)
        #expect(self.list(try tasks()) { $0.filter = .available }.ids == ["A-1", "B-1", "A-3", "A-4"])
        #expect(self.list(try tasks()) { $0.query = "brief-word" }.ids == ["A-2"])
        #expect(list.text(heading: ["Product"]).contains("  A-1 Task A-1 ●\n  B-1 Task B-1 · Infrastructure ◌\n  A-2 Task A-2 ○"))
    }

    @Test func severalSelectedShowNoBriefAndStartNamesEachFlow() throws {
        let tasks = try tasks()
        var state = BacklogState()
        state.select("A-1")
        let one = list(tasks) { $0 = state }
        #expect(one.opened?.id == "A-1" && one.startLabel == "Start · feature")
        state.step(1, extending: true, rows: one.ids)
        state.step(1, extending: true, rows: one.ids)
        let three = list(tasks) { $0 = state }
        #expect(three.selected.map(\.id) == ["A-1", "B-1", "A-2"])
        #expect(three.opened == nil)
        #expect(three.startLabel == "Start 3 · feature 2 · code 1")
        #expect(list(tasks).startLabel == "Start · feature")
    }

    @Test func reorderWritesRankAndUndoWritesItBack() async throws {
        let tasks = try tasks().filter { $0.task.waveId == "wave-a" }
        let writes = Recorder()
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.starts(with: ["task", "update"]) { await writes.add(args); return "{}" }
            throw RegistryQueryError("passive")
        }, repoPath: "/repo")
        let view = WorkNavigation.Board.wave("wave-a")
        let list = list(tasks)
        // ⌘↓ on the first row: A-1 goes below A-2.
        let moves = list.nudge(["A-1"], by: 1)
        #expect(moves.map { "\($0.task.id) \($0.rank)" } == ["A-1 1"])
        let change = try #require(await model.rankBacklog(moves, in: view))
        await model.revertBacklog(change, in: view)
        #expect(await writes.calls.map { $0.filter { $0.hasPrefix("--rank") } } == [["--rank=1"], ["--rank=0"]])
        // Over a whole repository order belongs to each Wave.
        #expect(!self.list(tasks, wave: nil).canReorder)
        // Several move together in list order, last first.
        #expect(list.toTop(["A-3", "A-4"]).map { "\($0.task.id) \($0.rank)" } == ["A-4 0", "A-3 0"])
        #expect(BacklogList.inverse(list.toTop(["A-3", "A-4"])).map { "\($0.task.id) \($0.rank)" } == ["A-3 2", "A-4 3"])
    }

    @Test func claimIsOffForSomeoneElsesTaskAndMineOffersAvailable() throws {
        let tasks = try tasks()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("passive") }, repoPath: "/repo")
        let theirs = list(tasks) { $0.select("A-2") }
        let toolbar = BacklogToolbar(model: model, view: .wave("wave-a"), list: theirs, perform: { _ in })
        #expect(try toolbar.inspect().find(viewWithAccessibilityIdentifier: "backlog-claim").button().isDisabled())
        let free = list(tasks) { $0.select("A-3") }
        let open = BacklogToolbar(model: model, view: .wave("wave-a"), list: free, perform: { _ in })
        #expect(try !open.inspect().find(viewWithAccessibilityIdentifier: "backlog-claim").button().isDisabled())
        let unclaimed = try tasks.filter { $0.id != "A-1" } + [task("A-1", rank: 0)]
        let mine = list(unclaimed) { $0.filter = .mine }
        #expect(mine.rows.isEmpty && mine.empty.step == .showAvailable)
        #expect(list(tasks) { $0.query = "nothing like this" }.empty.step == .clearSearch)
        #expect(list([]).empty.step == .new)
    }

    @Test func flowsReadStopsAndMergesFromTheCatalog() throws {
        func entry(_ name: String, _ description: String?, _ steps: [[String: Any]]) throws -> BacklogFlow {
            let json: [String: Any] = ["name": name, "description": description ?? NSNull(), "source": NSNull(),
                                       "compiled": ["name": name, "steps": steps], "composition": NSNull(), "unavailable": NSNull()]
            let decoded = try JSONDecoder().decode(GraphCatalogEntry.self, from: JSONSerialization.data(withJSONObject: json))
            return try #require(BacklogFlow(decoded))
        }
        func node(_ key: Int, _ label: String, kind: String = "skill", interactive: Bool = false) -> [String: Any] {
            ["key": key, "id": NSNull(), "label": label, "kind": kind, "interactive": interactive, "returns_to": NSNull(),
             "sources": [], "paths": []]
        }
        let code = try entry("code", "Build, deliver and verify. You try it.", [
            node(0, "implement"), node(1, "demo", interactive: true), node(2, "land --wait-and-fix", kind: "op"),
            node(3, "verify", interactive: true)])
        #expect(code.stops == 2 && !code.merges && code.consequence == "2 stops" && code.summary == "Build, deliver and verify")
        let auto = try entry("auto", "Request to merged PR.", [node(0, "implement"), node(1, "land --wait-and-fix", kind: "op")])
        #expect(auto.merges && auto.consequence == "merges without asking")
        #expect(try entry("incident", nil, [node(0, "unbreak")]).consequence == "never stops")
    }

    @Test func boardRendersBacklogRowsAndOpensOneBrief() throws {
        let roadmap = try roadmap()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") },
                              repoPath: roadmap.wave.repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let view = WorkNavigation.Board.wave(roadmap.wave.id)
        let board = BacklogBoard(model: model, view: view)
        #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string() == roadmap.wave.displayName)
        let backlog = model.backlog(view)
        #expect(!backlog.rows.isEmpty)
        for row in backlog.rows { _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-row-\(row.id)") }
        let first = try #require(backlog.rows.first)
        model.navigation.backlogs[view, default: BacklogState()].select(first.id)
        if !first.task.description.isEmpty {
            _ = try board.inspect().find(viewWithAccessibilityIdentifier: "backlog-brief-\(first.id)")
        }
        #expect(model.backlog(view).text(heading: []).contains("› \(first.task.identifier)"))
    }

    @Test func wavePartsHideOnHoverAndTheBoardHeaderShowsThem() throws {
        let repo = try roadmap().wave.repo
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") }, repoPath: repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture")
        model.bindWorkspaces(registry, repository: "repo")
        let conversation = registry.workspace(for: .init(repository: "repo", kind: .repository, id: "repo"),
                                              title: "repo", location: nil)
        let view = WorkNavigation.Board.repository
        #expect(model.navigation.waveView == view)
        // The Tasks strip starts hidden; the board header's Show… brings it back.
        #expect(model.desktopViewParts(repository: "repo").map(\.hidden) == [false, false, true])
        try BacklogBoard(model: model, view: view).inspect().find(viewWithAccessibilityIdentifier: "show-pane-tiles").button().tap()
        #expect(throws: (any Error).self) {
            try BacklogBoard(model: model, view: view).inspect().find(viewWithAccessibilityIdentifier: "show-hidden-panes")
        }
        conversation.stripHover("wave-tiles").isHovered = true
        let strip = HideablePart(part: .tiles, view: view, model: model, workspace: conversation) { Text("Tasks") }
        try strip.inspect().find(viewWithAccessibilityIdentifier: "wave-hide-tiles").button().tap()
        // `lf desktop hide -p session` hides the conversation as its pane; `-p board` names the part.
        let pane = try #require(conversation.multiplexer.sessionPane)
        conversation.setCollapsed(paneId: pane.id, collapsed: true)
        try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "hide",
                                                       args: ["part": "board"]), model: model, workspaces: registry)
        #expect(model.desktopViewParts(repository: "repo").map(\.hidden) == [true, true, true])
        #expect(throws: (any Error).self) {
            try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "hide",
                                                           args: ["part": "board"]), model: model, workspaces: registry)
        }
        // With the board hidden, the view's header row carries Show….
        let header = WaveViewHeader(model: model, view: view)
        #expect(try header.inspect().find(ShowHiddenMenu.self).actualView().items.map(\.title) == ["Backlog", "Session", "Tasks"])
        try header.inspect().find(viewWithAccessibilityIdentifier: "show-pane-session").button().tap()
        #expect(!conversation.multiplexer.collapsedPaneIds.contains(pane.id))
        try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "show",
                                                       args: ["part": "board"]), model: model, workspaces: registry)
        let board = BacklogBoard(model: model, view: view)
        try board.inspect().find(viewWithAccessibilityIdentifier: "show-pane-tiles").button().tap()
        #expect(model.desktopViewParts(repository: "repo").map(\.hidden) == [false, false, false])
    }

    @Test func wavePartsZoomAndResizeAsLfDesktopDoes() throws {
        let repo = try roadmap().wave.repo
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") }, repoPath: repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture")
        model.bindWorkspaces(registry, repository: "repo")
        let conversation = registry.workspace(for: .init(repository: "repo", kind: .repository, id: "repo"),
                                              title: "repo", location: nil)
        let view = WorkNavigation.Board.repository
        func perform(_ action: String, _ args: [String: String]) throws {
            try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: action,
                                                           args: args), model: model, workspaces: registry)
        }
        func parts() -> [DesktopViewPart] { model.desktopViewParts(repository: "repo") }
        model.setHidden(.tiles, false, in: view)
        // The hover button and `lf desktop zoom -p board` share one zoom; the crumb names it.
        conversation.stripHover("wave-board").isHovered = true
        let board = HideablePart(part: .board, view: view, model: model, workspace: conversation) { Text("Board") }
        try board.inspect().find(viewWithAccessibilityIdentifier: "wave-zoom-board").button().tap()
        #expect(parts().map(\.zoomed) == [true, false, false])
        try WorkBreadcrumbs(model: model).inspect().find(viewWithAccessibilityIdentifier: "zoom-out").button().tap()
        #expect(model.zoomedPart(in: view) == nil)
        // The conversation zooms as its session pane, so `-p session` agrees.
        try perform("zoom", ["part": "session", "on": "true"])
        let pane = try #require(conversation.multiplexer.sessionPane)
        #expect(conversation.multiplexer.zoomedPaneId == pane.id)
        #expect(parts().map(\.zoomed) == [false, true, false])
        try perform("zoom", ["part": "tiles"])
        #expect(conversation.multiplexer.zoomedPaneId == nil)
        #expect(parts().map(\.zoomed) == [false, false, true])
        try perform("hide", ["part": "tiles"])
        #expect(model.zoomedPart(in: view) == nil)
        // Dividers: the addressed part takes `ratio` of its split.
        try perform("resize", ["part": "tiles", "toward": "board", "ratio": "0.3"])
        try perform("resize", ["part": "board", "toward": "session", "ratio": "0.7"])
        #expect(abs(parts()[2].ratio - 0.3) < 0.0001)
        #expect(abs(parts()[0].ratio - 0.7) < 0.0001 && abs(parts()[1].ratio - 0.3) < 0.0001)
        #expect(throws: (any Error).self) { try perform("resize", ["part": "board", "toward": "board", "ratio": "0.5"]) }
        // The standup board's section and Done filter are gone.
        #expect(throws: (any Error).self) { try perform("section", ["section": "done"]) }
    }

    private func snapshot() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
    }

    private func roadmap() throws -> WaveRoadmap { try #require(try snapshot().waves.first) }
}

private actor Recorder {
    var calls: [[String]] = []
    func add(_ args: [String]) { calls.append(args) }
}
#endif
