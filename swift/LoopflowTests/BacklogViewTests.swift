#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave backlog")
@MainActor
struct BacklogViewTests {
    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private enum Holder { case you, someone, nobody }

    /// A backlog Task from the shared row fixture, in `wave` on `workflow`. `step`
    /// makes it a started Task standing at that step; `pr` puts its PR on GitHub;
    /// `blocked` is why its condition is blocked.
    private func task(_ id: String, rank: Int, holder: Holder = .nobody, wave: String = "wave-a",
                      workflow: String? = "feature", brief: String = "", section: String = "backlog",
                      step: String? = nil, pr: Int? = nil, unavailable: String? = nil,
                      blocked: String? = nil) throws -> RoadmapTask {
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
        row["graph_name"] = workflow.map { $0 as Any } ?? NSNull()
        row["lifecycle"] = step == nil ? "unstarted" : "idle"
        var condition = try #require(row["condition"] as? [String: Any])
        condition["state"] = blocked == nil ? "clear" : "blocked"
        condition["reason"] = blocked ?? ""
        row["condition"] = condition
        row["run_control"] = ["pausing": false, "unavailable": unavailable.map { $0 as Any } ?? NSNull()]
        row["execution"] = step.map { ["state": "idle", "reason": "Stopped at \($0)", "step": $0, "captured": NSNull(),
                                              "died": NSNull(), "outlook": NSNull()] as Any }
            ?? NSNull()
        row["pr"] = pr.map { number in
            ["id": "pr-\(id)", "sequence": 1, "slug": id, "branch": "you/\(id)", "base_commit": "1111111", "phase": "open",
             "empty": false, "merge_commit": NSNull(), "abandoned_at": NSNull(),
             "publication": ["requested_at": "2026-11-19T12:00:00Z", "presentation": NSNull(), "merge": NSNull(),
                             "github": ["number": number, "url": "https://github.com/example/repo/pull/\(number)"]]] as Any
        } ?? NSNull()
        row["owned_by_you"] = holder == .you
        let owner: Any = holder == .nobody ? NSNull() : [
            "person": holder == .you ? "You" : "Ada Lovelace", "person_id": NSNull(),
            "person_key": holder == .you ? "you@example.com" : "ada@example.com",
            "machine": ["id": NSNull(), "label": NSNull()]] as [String: Any]
        row["runtime"] = ["work_id": "work-\(id)", "status": step == nil ? "unstarted" : "idle", "reason": "Not started",
                          "updated_at": "2026-11-19T12:00:00Z", "provider": NSNull(), "completion_pending": NSNull(),
                          "owner": owner] as [String: Any]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    /// Six backlog Tasks: one yours, one someone else's, four unclaimed; Wave A runs
    /// feature, Wave B code. A-5 was started, stopped at demo with PR #12, and nobody holds it.
    private func tasks() throws -> [RoadmapTask] {
        [
            try task("A-1", rank: 0, holder: .you),
            try task("A-2", rank: 1, holder: .someone, brief: "Mentions the brief-word once."),
            try task("B-1", rank: 0, wave: "wave-b", workflow: "code"),
            try task("A-3", rank: 2),
            try task("A-4", rank: 3),
            try task("A-5", rank: 4, step: "demo", pr: 12),
            try task("A-DONE", rank: 5, section: "done"),
        ]
    }

    private func list(_ tasks: [RoadmapTask], wave: String? = "wave-a", _ change: (inout BacklogState) -> Void = { _ in })
        -> BacklogList {
        var state = BacklogState()
        change(&state)
        return BacklogList(tasks: tasks, truncated: false, wave: wave,
                           waveNames: ["wave-a": "Product", "wave-b": "Infrastructure"], state: state)
    }

    @Test func filtersCountOwnershipRustDecided() throws {
        let list = list(try tasks())
        #expect(BacklogFilter.allCases.map { "\($0.rawValue) \(list.count($0))" }.joined(separator: " · ")
            == "All 6 · Mine 1 · Available 5")
        // Finished work is not backlog; rows keep rank order within each Project.
        #expect(list.ids == ["A-1", "B-1", "A-2", "A-3", "A-4", "A-5"])
        #expect(list.waveLabel(list.rows[1]) == "Infrastructure" && list.waveLabel(list.rows[0]) == nil)
        #expect(self.list(try tasks()) { $0.filter = .available }.ids == ["A-1", "B-1", "A-3", "A-4", "A-5"])
        #expect(self.list(try tasks()) { $0.query = "brief-word" }.ids == ["A-2"])
        // Ownership is a word, and an unclaimed row says nothing.
        #expect(list.text(heading: ["Product"]).contains(
            "  A-1 Task A-1 · claimed by you\n  B-1 Task B-1 · Infrastructure\n  A-2 Task A-2 · claimed by Ada Lovelace\n  A-3 Task A-3\n"))
        // A started Task nobody holds leads with the word its row shows everywhere.
        #expect(list.text(heading: ["Product"]).contains("  A-5 Task A-5 · Idle, at demo, PR #12"))
        #expect(BacklogList.standing(list.rows[0]) == nil)
        // A Task in another Wave that someone claimed names both.
        let theirs = try task("B-2", rank: 1, holder: .someone, wave: "wave-b")
        #expect(list.ownership(theirs) == "Infrastructure, claimed by Ada Lovelace")
    }

    @Test func aRowSaysItsStandingOwnershipAndReasonAndItsChevronClosesTheBrief() throws {
        let stuck = try task("A-6", rank: 5, blocked: "merge the PR on GitHub")
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("passive") }, repoPath: try roadmap().wave.repo)
        let view = WorkNavigation.Board.wave(try roadmap().wave.id)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        // An unstarted Task that is blocked still says so, with the reason on its own line.
        #expect(BacklogList.standing(stuck) == "Blocked")
        #expect(TaskRowStatus(task: stuck).line == "merge the PR on GitHub")
        // The bar begins with what it acts on.
        let none = BacklogToolbar(model: model, view: view, list: list(try tasks()), perform: { _ in })
        #expect(try none.inspect().find(viewWithAccessibilityIdentifier: "backlog-selected").text().string() == "Nothing selected")
        let two = BacklogToolbar(model: model, view: view, list: list(try tasks()) { $0.selection = ["A-1", "A-3"] },
                                 perform: { _ in })
        #expect(try two.inspect().find(viewWithAccessibilityIdentifier: "backlog-selected").text().string() == "2 selected")
        #expect(throws: (any Error).self) { try two.inspect().find(viewWithAccessibilityIdentifier: "backlog-position") }
        // The chevron opens a row's brief and closes it again.
        let first = try #require(model.backlog(view).rows.first)
        let backlog = BacklogView(model: model, view: view)
        try backlog.inspect().find(viewWithAccessibilityIdentifier: "backlog-row-brief-\(first.id)").button().tap()
        #expect(model.backlog(view).opened?.id == first.id)
        try BacklogView(model: model, view: view).inspect()
            .find(viewWithAccessibilityIdentifier: "backlog-row-brief-\(first.id)").button().tap()
        #expect(model.backlog(view).opened == nil)
    }

    @Test func aStartedRowNobodyHoldsOpensAndStartReachesOnlyTheUnstarted() async throws {
        let tasks = try tasks()
        let started = list(tasks) { $0.select("A-5") }
        #expect(started.startLabel == "Open" && BacklogList.startUnavailable(started.selected) == nil)
        let runs = Recorder()
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.starts(with: ["task", "run"]) { await runs.add(args); return "{}" }
            throw RegistryQueryError("passive")
        }, repoPath: "/repo")
        let view = WorkNavigation.Board.wave("wave-a")
        // Open, with no workflow menu.
        let bar = BacklogStartButton(model: model, list: started, perform: { _ in })
        _ = try bar.inspect().find(text: "Open")
        #expect(throws: (any Error).self) { try bar.inspect().find(viewWithAccessibilityIdentifier: "backlog-start-menu") }
        _ = try BacklogStartButton(model: model, list: list(tasks) { $0.select("A-3") }, perform: { _ in })
            .inspect().find(viewWithAccessibilityIdentifier: "backlog-start-menu")
        // Open goes to the Task and runs nothing.
        model.openBacklog(started.selected[0])
        #expect(model.selection == .task(id: "A-5"))
        // A selection of both counts the unstarted, says why the other is left, and starts only those.
        let mixed = list(tasks) { $0.selection = ["A-3", "A-4", "A-5"] }
        #expect(mixed.startLabel == "Start 2 with feature")
        #expect(BacklogList.startedNote(mixed.selected) == "1 started Task: open it to continue")
        #expect(BacklogList.startUnavailable(mixed.selected) == nil)
        model.navigation.backlogs[view] = mixed.state
        model.startBacklog(mixed.selected, graph: "auto", in: view)
        // Started Tasks alone start nothing.
        model.startBacklog(started.selected, graph: nil, in: view)
        for _ in 0..<200 where await runs.calls.count < 2 { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await runs.calls.map { $0.joined(separator: " ") }.sorted() == [
            "task run A-3 auto --json", "task run A-4 auto --json"])
        #expect(model.navigation.backlogs[view]?.notice == .did("Started 2"))
        #expect(model.navigation.backlogs[view]?.selection == ["A-5"])
        // Several started Tasks leave Start off and say why.
        let both = [try task("A-6", rank: 5, step: "demo"), try task("A-7", rank: 6, step: "gate")]
        #expect(BacklogList.startLabel(both) == "Start")
        #expect(BacklogList.startUnavailable(both) == "2 started Tasks: open each to continue")
        // Rust's refusal for every unstarted Task turns Start off and says why.
        let stuck = [try task("A-9", rank: 0, unavailable: "Task checkout is on another Machine"), tasks[5]]
        #expect(BacklogList.startUnavailable(stuck) == "Task checkout is on another Machine")
        // Claiming a started Task says where it went.
        model.claimBacklog(started.selected, in: view)
        #expect(model.navigation.backlogs[view]?.notice == .did("Claimed A-5, now in Working"))
    }

    @Test func filterAndSearchDropHiddenRowsAndAbandonNamesWhatItRemoves() throws {
        let tasks = try tasks()
        var state = BacklogState()
        state.selection = ["A-1", "A-2", "A-5"]
        state.query = "brief-word"
        // Actions reach only what the list shows.
        let searched = list(tasks) { $0 = state }
        #expect(searched.selected.map(\.id) == ["A-2"] && searched.opened?.id == "A-2")
        // Clearing the search does not bring the hidden rows' selection back.
        state.keep(searched.ids)
        state.query = ""
        #expect(list(tasks) { $0 = state }.selected.map(\.id) == ["A-2"])
        #expect(list(tasks) { $0.selection = ["A-1"]; $0.filter = .available }.selected.map(\.id) == ["A-1"])
        #expect(list(tasks) { $0.selection = ["A-2"]; $0.filter = .mine }.selected.isEmpty)
        #expect(BacklogList.abandonMessage(Array(tasks.prefix(6))).hasPrefix(
            "A-1, A-2, B-1, A-3, A-4 and 1 more\nWith a PR: A-5 (PR #12)\n"))
        #expect(!BacklogList.abandonMessage([tasks[0]]).contains("A-1"))
    }

    @Test func severalSelectedShowNoBriefAndStartNamesEachWorkflow() throws {
        let tasks = try tasks()
        var state = BacklogState()
        state.select("A-1")
        let one = list(tasks) { $0 = state }
        #expect(one.opened?.id == "A-1" && one.startLabel == "Start with feature")
        state.step(1, extending: true, rows: one.ids)
        state.step(1, extending: true, rows: one.ids)
        let three = list(tasks) { $0 = state }
        #expect(three.selected.map(\.id) == ["A-1", "B-1", "A-2"])
        #expect(three.opened == nil)
        #expect(three.startLabel == "Start 3: feature 2, code 1")
        // Nothing selected: Start names no workflow, and the bar is off.
        #expect(list(tasks).startLabel == "Start")
        // A Task Rust sent no Graph for is started without a name.
        #expect(list([try task("A-8", rank: 0, workflow: nil)]) { $0.select("A-8") }.startLabel == "Start")
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

    @Test func anUndoneReorderThatIsRefusedSaysSo() async throws {
        let tasks = try tasks().filter { $0.task.waveId == "wave-a" }
        let writes = Recorder()
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.starts(with: ["task", "update"]) else { throw RegistryQueryError("passive") }
            await writes.add(args)
            if await writes.calls.count > 1 { throw RegistryQueryError("store is locked") }
            return "{}"
        }, repoPath: "/repo")
        let view = WorkNavigation.Board.wave("wave-a")
        let change = try #require(await model.rankBacklog(list(tasks).nudge(["A-1"], by: 1), in: view))
        await model.revertBacklog(change, in: view)
        #expect(model.navigation.backlogs[view]?.notice == .failed(Failure(what: "A-1's order not saved.", machine: "store is locked")))
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
        #expect(list([]).empty.step == .new && list([]).empty.title == "Nothing is waiting in this Wave.")
    }

    @Test func startFlowsReadStopsMergesAndPiecesFromTheCatalog() throws {
        func entry(_ name: String, _ description: String?, _ steps: [[String: Any]], includedBy: [String] = [],
                   runs: String = "task") throws -> GraphCatalogEntry {
            let stops = steps.filter { $0["interactive"] as? Bool == true }
                .map { ["name": $0["label"], "skill": $0["label"], "description": NSNull()] }
            let json: [String: Any] = ["name": name, "description": description ?? NSNull(), "source": NSNull(),
                                       "compiled": ["name": name, "steps": steps], "composition": NSNull(), "unavailable": NSNull(),
                                       "workflow": ["steps": [[String: Any]](), "interactive_steps": stops, "sentence": ""],
                                       "included_by": includedBy, "runs": runs]
            return try JSONDecoder().decode(GraphCatalogEntry.self, from: JSONSerialization.data(withJSONObject: json))
        }
        func node(_ key: Int, _ label: String, kind: String = "skill", interactive: Bool = false) -> [String: Any] {
            ["key": key, "id": NSNull(), "label": label, "kind": kind, "interactive": interactive, "returns_to": NSNull(),
             "sources": [], "paths": []]
        }
        let catalog = [
            try entry("code", "Build, deliver and verify. You try it.", [
                node(0, "implement"), node(1, "demo", interactive: true), node(2, "land --wait-and-fix", kind: "op"),
                node(3, "verify")]),
            try entry("auto", "Request to merged PR.", [node(0, "implement"), node(1, "land --wait-and-fix", kind: "op")]),
            try entry("incident", "Unbreak it.", [node(0, "unbreak")]),
            try entry("build", "Build the plan in chunks.", [node(0, "implement")], includedBy: ["code", "auto"]),
            try entry("mine", nil, [node(0, "implement")]),
            try entry("vsm-operate", "Operate a Wave.", [node(0, "s1")], runs: "wave"),
        ]
        let code = try #require(StartFlow(catalog[0]))
        #expect(code.stops == 1 && !code.merges && code.consequence == "1 stop" && code.summary == "Build, deliver and verify")
        #expect(StartFlow(catalog[1])?.consequence == "merges without asking")
        #expect(StartFlow(catalog[2])?.consequence == "never stops")
        // Start's menu and New's chooser read one layout: pieces and undescribed
        // Graphs under More, and a Wave's Graph not offered for a Task at all.
        let flows = StartFlows(catalog)
        #expect(flows.stopping.map(\.name) == ["code"])
        #expect(flows.unattended.map(\.name) == ["auto", "incident"])
        #expect(flows.more.map(\.name) == ["build", "mine"])
    }

    @Test func backlogRendersItsRowsAndOpensOneBrief() throws {
        let roadmap = try roadmap()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") },
                              repoPath: roadmap.wave.repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let view = WorkNavigation.Board.wave(roadmap.wave.id)
        let backlogView = BacklogView(model: model, view: view)
        #expect(try backlogView.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string() == roadmap.wave.displayName)
        // The Wave's one Project sits beside its name, linked to the tracker; the capture names it too.
        let project = try #require(roadmap.project.present)
        #expect(try backlogView.inspect().find(viewWithAccessibilityIdentifier: "wave-header-project").find(ViewType.Link.self).labelView().text().string() == project.name)
        #expect(BacklogList.heading(roadmap).last == "\(project.name) · Started")
        let backlog = model.backlog(view)
        #expect(!backlog.rows.isEmpty)
        for row in backlog.rows { _ = try backlogView.inspect().find(viewWithAccessibilityIdentifier: "wave-row-\(row.id)") }
        let first = try #require(backlog.rows.first)
        model.navigation.backlogs[view, default: BacklogState()].select(first.id)
        if !first.task.description.isEmpty {
            _ = try backlogView.inspect().find(viewWithAccessibilityIdentifier: "backlog-brief-\(first.id)")
        }
        #expect(model.backlog(view).text(heading: []).contains("› \(first.task.identifier)"))
    }

    @Test func aFailedBriefSaveIsAFailureLineAndAFiledTaskStaysText() async throws {
        let roadmap = try roadmap()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("store is locked") },
                              repoPath: roadmap.wave.repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let view = WorkNavigation.Board.wave(roadmap.wave.id)
        let task = try #require(model.backlog(view).rows.first)
        #expect(await model.saveBacklogBrief(task, text: "New brief", in: view) == nil)
        let failed = try BacklogView(model: model, view: view).inspect().find(FailureLine<EmptyView>.self)
        _ = try failed.find(text: "Brief not saved.")
        _ = try failed.find(text: "store is locked")

        model.navigation.backlogs[view]?.notice = .did("Filed \(task.task.identifier)")
        let filed = try BacklogView(model: model, view: view).inspect()
        #expect(try filed.find(viewWithAccessibilityIdentifier: "backlog-notice").find(ViewType.Text.self).string()
            == "Filed \(task.task.identifier)")
        #expect(throws: (any Error).self) { try filed.find(FailureLine<EmptyView>.self) }
    }

    @Test func wavePartsHideOnHoverAndTheBacklogHeaderShowsThem() throws {
        let repo = try roadmap().wave.repo
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") }, repoPath: repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture")
        model.bindWorkspaces(registry, repository: "repo")
        let conversation = registry.workspace(for: .init(repository: "repo", kind: .repository, id: "repo"),
                                              title: "repo", location: nil)
        let view = WorkNavigation.Board.repository
        #expect(model.navigation.waveView == view)
        // The Tasks strip starts hidden; the backlog header's Show… brings it back.
        #expect(model.desktopViewParts(repository: "repo").map(\.hidden) == [false, false, true])
        try BacklogView(model: model, view: view).inspect().find(viewWithAccessibilityIdentifier: "show-pane-tiles").button().tap()
        #expect(throws: (any Error).self) {
            try BacklogView(model: model, view: view).inspect().find(viewWithAccessibilityIdentifier: "show-hidden-panes")
        }
        conversation.stripHover("wave-tiles").isHovered = true
        let strip = HideablePart(part: .tiles, view: view, model: model, workspace: conversation) { Text("Tasks") }
        try strip.inspect().find(viewWithAccessibilityIdentifier: "wave-hide-tiles").button().tap()
        // `lf desktop hide -p session` hides the conversation as its pane; `-p backlog` names the part.
        let pane = try #require(conversation.multiplexer.sessionPane)
        conversation.setCollapsed(paneId: pane.id, collapsed: true)
        try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "hide",
                                                       args: ["part": "backlog"]), model: model, workspaces: registry)
        #expect(model.desktopViewParts(repository: "repo").map(\.hidden) == [true, true, true])
        #expect(throws: (any Error).self) {
            try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "hide",
                                                           args: ["part": "backlog"]), model: model, workspaces: registry)
        }
        // With the backlog hidden, the view's header row carries Show….
        let header = WaveViewHeader(model: model, view: view)
        #expect(try header.inspect().find(ShowHiddenMenu.self).actualView().items.map(\.title) == ["Backlog", "Session", "Tasks"])
        try header.inspect().find(viewWithAccessibilityIdentifier: "show-pane-session").button().tap()
        #expect(!conversation.multiplexer.collapsedPaneIds.contains(pane.id))
        try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "show",
                                                       args: ["part": "backlog"]), model: model, workspaces: registry)
        let backlogView = BacklogView(model: model, view: view)
        try backlogView.inspect().find(viewWithAccessibilityIdentifier: "show-pane-tiles").button().tap()
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
        // The hover button and `lf desktop zoom -p backlog` share one zoom; the crumb names it.
        conversation.stripHover("wave-backlog").isHovered = true
        let backlogView = HideablePart(part: .backlog, view: view, model: model, workspace: conversation) { Text("Backlog") }
        try backlogView.inspect().find(viewWithAccessibilityIdentifier: "wave-zoom-backlog").button().tap()
        #expect(parts().map(\.zoomed) == [true, false, false])
        try TitleBar(model: model).inspect().find(viewWithAccessibilityIdentifier: "zoom-out").button().tap()
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
        try perform("resize", ["part": "tiles", "toward": "backlog", "ratio": "0.3"])
        try perform("resize", ["part": "backlog", "toward": "session", "ratio": "0.7"])
        #expect(abs(parts()[2].ratio - 0.3) < 0.0001)
        #expect(abs(parts()[0].ratio - 0.7) < 0.0001 && abs(parts()[1].ratio - 0.3) < 0.0001)
        #expect(throws: (any Error).self) { try perform("resize", ["part": "backlog", "toward": "backlog", "ratio": "0.5"]) }
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
