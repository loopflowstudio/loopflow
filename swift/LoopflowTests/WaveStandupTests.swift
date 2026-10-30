#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave standup")
@MainActor
struct WaveStandupTests {
    private let now = ISO8601DateFormatter().date(from: "2026-10-26T12:00:00Z")!
    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private func task(_ id: String, section: String = "working", state: String? = nil, completedAt: String? = nil,
                      rank: Int = 0, status: String? = nil, owner: String? = nil, email: String?? = .none,
                      pr: String? = nil, reason: String = "Next move") throws -> RoadmapTask {
        let rows = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_history_rows.json"))) as? [[String: Any]])
        var row = rows[0]
        var plan = try #require(row["task"] as? [String: Any])
        plan["id"] = id
        plan["identifier"] = id
        plan["name"] = "Task \(id)"
        plan["rank"] = rank
        plan["state"] = state.map { $0 as Any } ?? NSNull()
        plan["completed"] = state == "completed"
        plan["completed_at"] = completedAt.map { $0 as Any } ?? NSNull()
        row["task"] = plan
        row["section"] = section
        row["next_move"] = ["owner": "task", "reason": reason]
        let key = email ?? owner.map { "\($0.lowercased().replacingOccurrences(of: " ", with: "."))@example.com" }
        let ownerJSON: Any = owner.map { person -> Any in
            ["person": person, "person_key": key.map { $0 as Any } ?? NSNull(),
             "machine": ["id": NSNull(), "label": NSNull()]] as [String: Any]
        } ?? NSNull()
        row["runtime"] = (status ?? owner.map { _ in "moving" }).map { status -> Any in
            ["work_id": "work-\(id)", "status": status, "reason": "Running", "updated_at": "2026-10-26T12:00:00Z",
             "provider": "claude", "completion_pending": NSNull(), "owner": ownerJSON] as [String: Any]
        } ?? NSNull()
        row["pr"] = pr.map { phase -> Any in
            ["id": "pr-\(id)", "sequence": 1, "slug": id, "branch": id, "base_commit": "abc", "phase": phase, "empty": NSNull(),
             "publication": NSNull(), "merge_commit": NSNull(), "abandoned_at": NSNull()] as [String: Any]
        } ?? NSNull()
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    private func standup(_ tasks: [RoadmapTask], closed: [RoadmapTask]? = nil, doneDays: Int = 7,
                         github: [String: GitHubPerson] = [:],
                         sessions: [String: WaveStandup.SessionAttention] = [:]) -> WaveStandup {
        WaveStandup(tasks: tasks, closed: closed ?? tasks, doneDays: doneDays, now: now, github: github, sessions: sessions)
    }

    @Test func columnsAreRustSections() throws {
        let tasks = [
            try task("DONE-1", section: "done", state: "completed", completedAt: "2026-10-25T09:00:00Z"),
            try task("DONE-OLD", section: "done", state: "completed", completedAt: "2026-10-01T09:00:00Z"),
            try task("CANCELED", section: "abandoned", state: "canceled", completedAt: "2026-10-25T09:00:00Z"),
            try task("IDLE", rank: 2, status: "idle", owner: "Jack Heart"),
            try task("MOVING", rank: 1, status: "moving", owner: "Jack Heart", reason: "Waiting on CI"),
            try task("MERGED", rank: 3, status: "active", owner: "Jack Heart", pr: "merged"),
            try task("SHIP", section: "autonomous", rank: 4, status: "shipping", owner: "Jack Heart"),
            try task("FRESH", section: "backlog", rank: 5),
            // Started, then unassigned: Rust files it under Backlog, whatever its state.
            try task("HELD", section: "backlog", rank: 0, status: "idle"),
        ]
        let standup = standup(tasks, sessions: ["IDLE": .available, "SHIP": .busy])
        #expect(standup.done.map(\.id) == ["DONE-1"])
        #expect(standup.working.map(\.id) == ["MOVING", "IDLE", "MERGED"])
        #expect(standup.working[0].note == "Waiting on CI")
        #expect(standup.working[2].note == "merged")
        #expect(standup.working[1].session == .available)
        #expect(standup.autonomous.map(\.id) == ["SHIP"])
        #expect(standup.autonomous[0].session == .busy)
        #expect(standup.backlog.map(\.id) == ["HELD", "FRESH"])
        // One person owns everything: a solo Wave shows no pictures.
        #expect(!standup.showsPeople)
        #expect(self.standup(tasks, doneDays: 0).done.map(\.id).sorted() == ["DONE-1", "DONE-OLD"])
    }

    @Test func oneEmailIsOnePersonAndANameAloneJoinsIt() throws {
        let tasks = [
            try task("A", owner: "Jack Heart", email: "Jack@Loopflow.studio"),
            try task("B", owner: "Jack Heart", email: .some(nil)),
        ]
        #expect(!standup(tasks).showsPeople)
        #expect(Set(standup(tasks).working.compactMap(\.person?.key)) == ["jack@loopflow.studio"])
        let ada = try task("C", owner: "Ada Lovelace")
        let user = GitHubPerson(login: "jackheart", name: "Jack H.", avatarUrl: URL(string: "https://avatars.example/jack")!)
        let shared = standup(tasks + [ada], github: ["jack@loopflow.studio": user])
        #expect(shared.showsPeople)
        #expect(shared.working[0].person?.name == "Jack H.")
        #expect(shared.working[1].person?.github?.avatarUrl == user.avatarUrl)
        #expect(shared.working[2].person?.github?.avatarUrl == nil)
    }

    @Test func doneWindowCountsWholeDaysBackFromNow() throws {
        let edge = try task("EDGE", section: "done", state: "completed", completedAt: "2026-10-19T12:00:00Z")
        let past = try task("PAST", section: "done", state: "completed", completedAt: "2026-10-19T11:59:59Z")
        let undated = try task("UNDATED", section: "done", state: "completed")
        #expect(standup([edge, past, undated]).done.map(\.id) == ["EDGE"])
        #expect(standup([edge, past], doneDays: 14).done.count == 2)
        // Done reads only the closed source, so it can move to `lf task list --done`.
        #expect(standup([edge], closed: [], doneDays: 0).done.isEmpty)
        #expect(standup([], closed: [edge]).done.map(\.id) == ["EDGE"])
    }

    @Test func peopleAppearOnceTwoPeopleShareTheWave() throws {
        let tasks = [
            try task("A", owner: "Jack Heart"),
            try task("B", owner: "Ada Lovelace"),
        ]
        let standup = standup(tasks, sessions: ["A": .busy])
        #expect(standup.showsPeople)
        #expect(standup.working.compactMap(\.person?.initials) == ["JH", "AL"])
        let text = standup.text(heading: WaveStandup.heading(try roadmap()), section: .working)
        #expect(text.contains("Backlog 0 · [Working 2] · Autonomous 0 · Done 0"))
        #expect(text.contains("A Task A · Jack Heart · Busy"))
        #expect(standup.text(heading: [], section: .done).hasSuffix("Done 0]\n  last 7 days\n  None"))
    }

    @Test func boardRendersHeaderColumnsAndMetrics() throws {
        let roadmap = try roadmap()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") },
                              repoPath: roadmap.wave.repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let board = WaveStandupBoard(model: model, view: .wave(roadmap.wave.id))
        #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string() == roadmap.wave.displayName)
        #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-metric-headline").text().string()
            == WaveMetricPortfolioPresentation(portfolio: roadmap.metricPortfolio).headline)
        // One section at a time, Backlog first; `lf desktop do section` turns it.
        _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-section-backlog")
        model.navigation.boardSections[.wave(roadmap.wave.id)] = .done
        _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-section-done")
        _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-done-days")
        for notice in roadmap.unavailableTasks {
            _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-notice-\(notice.taskId)")
        }
        if let project = roadmap.currentProject {
            #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-current-project").text().string()
                .hasPrefix(project.name))
        }
    }

    @Test func repositoryPageIsTheSameBoardOverEveryRepositoryTask() throws {
        let snapshot = try snapshot()
        let repo = try roadmap().wave.repo
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") }, repoPath: repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let board = WaveStandupBoard(model: model, view: .repository)
        #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string() == model.repositoryName)
        // Unstarted backlog from every Wave, and none, is on the repository's board.
        let standup = standup(model.visibleTasks, doneDays: 0)
        #expect(!standup.backlog.isEmpty)
        for row in standup.backlog.prefix(WaveBoardList.pageSize) {
            _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-row-\(row.id)")
        }
        #expect(throws: (any Error).self) { try board.inspect().find(viewWithAccessibilityIdentifier: "wave-current-project") }
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
        try WaveStandupBoard(model: model, view: view).inspect().find(viewWithAccessibilityIdentifier: "show-pane-tiles").button().tap()
        #expect(throws: (any Error).self) {
            try WaveStandupBoard(model: model, view: view).inspect().find(viewWithAccessibilityIdentifier: "show-hidden-panes")
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
        #expect(try header.inspect().find(ShowHiddenMenu.self).actualView().items.map(\.title) == ["Board", "Session", "Tasks"])
        try header.inspect().find(viewWithAccessibilityIdentifier: "show-pane-session").button().tap()
        #expect(!conversation.multiplexer.collapsedPaneIds.contains(pane.id))
        try DesktopAction.perform(DesktopActionRequest(repository: "repo", window: "window", pane: nil, action: "show",
                                                       args: ["part": "board"]), model: model, workspaces: registry)
        let board = WaveStandupBoard(model: model, view: view)
        try board.inspect().find(viewWithAccessibilityIdentifier: "show-pane-tiles").button().tap()
        #expect(model.desktopViewParts(repository: "repo").map(\.hidden) == [false, false, false])
    }

    @Test func wavePartsZoomResizeAndTurnSectionsAsLfDesktopDoes() throws {
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
        #expect(parts()[0].section == "backlog")
        try perform("section", ["section": "autonomous"])
        #expect(parts()[0].section == "autonomous")
        #expect(throws: (any Error).self) { try perform("section", ["section": "active"]) }
    }

    private func snapshot() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
    }

    private func roadmap() throws -> WaveRoadmap { try #require(try snapshot().waves.first) }
}
#endif
