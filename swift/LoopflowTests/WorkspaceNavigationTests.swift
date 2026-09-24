#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Unified Work and Session navigation")
@MainActor
struct WorkspaceNavigationTests {
    @Test("Autonomous, upcoming and human work share stable ranked planning rows")
    func workDoesNotRequireSessions() throws {
        let roadmap = try roadmap()
        let empty = WorkspaceProjection(roadmaps: roadmap.waves, sessions: [])
        let interactive = try session("human", work: .task(id: "ts_review00000000000000000000000000"))
        let joined = WorkspaceProjection(roadmaps: roadmap.waves, sessions: [interactive])
        let before = empty.waves[0].projects[0].tasks
        let after = joined.waves[0].projects[0].tasks

        #expect(before.map(\.id) == after.map(\.id))
        #expect(after.map(\.task.id) == ["issue-review", "issue-now", "issue-available"])
        #expect(after[1].task.condition.state == .clear)
        #expect(after[2].task.runtime == nil)
        #expect(after[0].sessions.map(\.id) == ["human"])
        #expect(joined.sessions(for: .task(id: "issue-available")).isEmpty)
        #expect(joined.subject(for: "human") == .task(id: "issue-review"))
    }

    @Test("Typed durable joins preserve top-level, multiple, completed and unmatched Sessions")
    func everyHumanBoundaryRemainsReachable() throws {
        let records = try [
            session("repo", work: nil),
            session("wave", work: .wave(id: "wave-1")),
            session("project", work: .project(id: "ps_11111111111111111111111111111111")),
            session("completed", work: .task(id: "ts_later000000000000000000000000000")),
            session("another", work: .task(id: "ts_later000000000000000000000000000")),
            session("wrong-kind", work: .project(id: "ts_review00000000000000000000000000")),
            session("planning-id-is-not-work", work: .task(id: "issue-review")),
        ]
        let projection = WorkspaceProjection(roadmaps: try roadmap().waves, sessions: records)
        #expect(projection.waves[0].sessions.map(\.id) == ["wave"])
        #expect(projection.waves[0].projects[0].sessions.map(\.id) == ["project"])
        let completed = try #require(projection.waves[0].projects[0].tasks.first)
        #expect(completed.task.task.completed)
        #expect(completed.sessions.map(\.id) == ["completed", "another"])
        #expect(projection.unmatchedSessions.map(\.id) == ["repo", "wrong-kind", "planning-id-is-not-work"])
        let missing = WorkspaceProjection(roadmaps: [], sessions: records)
        #expect(missing.unmatchedSessions == records)
    }

    @Test("Presentation and repo changes preserve selection, expansion, search and terminal splits")
    func navigationRetainsWorkspace() async throws {
        let model = try model()
        await model.refresh()
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/src/loopflow")
        workspace.multiplexer.load(sessionId: "human")
        let sessionPane = workspace.multiplexer.focusedPaneId
        _ = workspace.multiplexer.split(sessionPane, axis: .vertical)
        workspace.multiplexer.newShell()
        let layout = workspace.multiplexer.layout
        let focused = workspace.multiplexer.focusedPaneId
        model.select(.task(id: "issue-review"))
        let navigation = model.navigation
        #expect(navigation.selection == .task(id: "issue-review"))
        let group = model.workspace.waves[0].id
        navigation.toggle(group)
        navigation.search = "upcoming"
        #expect(navigation.isExpanded(group))
        navigation.toggle(group)
        navigation.search = ""
        #expect(!navigation.isExpanded(group))
        navigation.content = .terminals
        navigation.showsList = true
        navigation.showsList = false
        model.setRepoPath("/src/context")
        model.select(.wave(id: "wave-2"))
        model.setRepoPath("/src/loopflow")

        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation === navigation)
        #expect(model.navigation.content == .terminals)
        #expect(!model.navigation.isExpanded(group))
        #expect(registry.workspace(for: "/src/loopflow") === workspace)
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == focused)
        workspace.multiplexer.load(sessionId: "human")
        workspace.multiplexer.load(sessionId: "human")
        #expect(workspace.multiplexer.focusedPaneId == sessionPane)
        #expect(workspace.multiplexer.layout == layout)
    }

    @Test("Failed reads keep last-good work and exact Sessions with a visible error")
    func unavailableIsNotEmpty() async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        let keys = model.workspace.waves[0].projects[0].tasks.map(\.id)
        model.select(.task(id: "issue-review"))
        await source.fail()
        await model.refresh()
        #expect(model.roadmap.errorMessage == "offline")
        #expect(model.sessions.errorMessage == "offline")
        #expect(model.workspace.waves[0].projects[0].tasks.map(\.id) == keys)
        #expect(model.workspace.sessions(for: model.selection).map(\.id) == ["human"])
        let view = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: Never.self) { try view.inspect().find(text: "Sessions unavailable: offline") }
    }

    @Test("Returning to a repository retains its last-good Sessions when refresh fails")
    func lastGoodSessionsSurviveRepositorySwitch() async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.setRepoPath("/src/context")
        #expect(model.sessions.value == nil)
        await source.fail()
        model.setRepoPath("/src/loopflow")
        #expect(model.sessions.value?.map(\.id) == ["human"])
        await model.refreshSessions()
        #expect(model.sessions.errorMessage == "offline")
        #expect(model.sessions.value?.map(\.id) == ["human"])
        model.setRepoPath("/src/context")
        model.sessionResolved("human", repo: "/src/loopflow")
        model.setRepoPath("/src/loopflow")
        #expect(model.sessions.value == [])
        #expect(model.sessions.errorMessage == "offline")
    }

    @Test("Incomplete planning preserves the repository's selected Task", arguments: [false, true])
    func unavailablePlanningPreservesRepositorySelection(truncated: Bool) async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(.task(id: "issue-review"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        waves[0]["projects"] = truncated
            ? ["state": "ok", "items": [], "truncated": true]
            : ["state": "unavailable", "reason": "planning offline"]
        waves[0]["unavailable_projects"] = []
        snapshot["waves"] = waves
        await source.replaceRoadmap(String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self))
        await model.refresh()
        #expect(model.selection == .task(id: "issue-review"))
        await model.refreshPortfolio(initialRepoPath: nil)
        #expect(model.selection == .task(id: "issue-review"))
        model.setRepoPath("/src/context")
        model.setRepoPath("/src/loopflow")
        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation.content == .details)
        #expect(model.workspace.unmatchedSessions.map(\.id) == ["human"])
    }

    @Test("Completing a Session removes its link without completing its Task")
    func resolutionKeepsTask() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        model.sessionResolved("human", repo: "/src/context")
        #expect(model.workspace.sessions(for: model.selection).count == 1)
        model.sessionResolved("human", repo: "/src/loopflow")
        #expect(model.workspace.sessions(for: model.selection).isEmpty)
        #expect(model.task(id: "issue-review")?.task.task.completed == false)
        #expect(model.selection == .task(id: "issue-review"))
    }

    @Test("Task details expose directive and Project proof from the same snapshot")
    func inspectorShowsPlanning() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let task = try #require(model.task(id: "issue-review"))
        let view = WorkSurfaceView(model: model)
        #expect(throws: Never.self) { try view.inspect().find(text: task.task.task.description) }
        #expect(throws: Never.self) { try view.inspect().find(text: task.project.project.definition) }
        #expect(throws: Never.self) { try view.inspect().find(text: task.project.project.krs[0].text) }
    }

    @Test("Slow planning does not gate Session reading or navigation")
    func sessionsArriveBeforePlanning() async throws {
        let snapshot = try roadmapJSON()
        let records = try sessionsJSON()
        let gate = PlanningGate()
        let model = PodiumModel(query: RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": await gate.wait(); return snapshot
            case "session": return records
            case "ls": return "[]"
            default: return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            }
        }, repoPath: "/src/loopflow")
        let refresh = Task { await model.refresh() }
        await gate.started()
        await model.refreshSessions()
        #expect(model.roadmap.isLoading)
        #expect(model.workspace.unmatchedSessions.map(\.id) == ["human"])
        await gate.release()
        await refresh.value
        #expect(model.workspace.sessions(for: .task(id: "issue-review")).map(\.id) == ["human"])
        #expect(model.workspace.unmatchedSessions.isEmpty)
    }

    @Test("Unresolved planning does not claim a selected Task has no Sessions")
    func unavailableAssociationIsNotNoSessions() async throws {
        let model = try model()
        await model.refresh()
        model.navigation.selection = .task(id: "absent-from-planning")
        model.navigation.content = .details
        let view = SessionsView(
            model: model, repoPath: "/src/loopflow",
            workspaces: SessionsWorkspaceRegistry()
        )
        #expect(throws: Never.self) {
            try view.inspect().find(text: "Session association unavailable — use the work list")
        }
    }

    @Test("New conversation follows visible scope while All work retains its previous selection")
    func conversationFollowsZoom() async throws {
        let model = try model()
        await model.refresh()
        #expect(model.conversationScope == .repo("/src/loopflow"))
        model.select(.wave(id: "wave-1"))
        guard case .wave = model.conversationScope else { Issue.record("Expected Wave context"); return }
        model.select(.task(id: "issue-review"))
        guard case .task(_, let issue) = model.conversationScope else { Issue.record("Expected Task context"); return }
        #expect(issue == model.task(id: "issue-review")?.task.task.identifier)
        model.navigation.content = .overview
        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.conversationScope == .repo("/src/loopflow"))
        #expect(model.conversationLabel == "loopflow")
        model.navigation.content = .terminals
        guard case .task = model.conversationScope else { Issue.record("Expected restored Task context"); return }
    }

    private func model() throws -> PodiumModel {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        return PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
    }
    private func roadmap() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
    }
    private func roadmapJSON() throws -> String {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
    }
    private func sessionsJSON() throws -> String {
        let records = [try session("human", work: .task(id: "ts_review00000000000000000000000000"))]
        return String(decoding: try JSONEncoder().encode(records), as: UTF8.self)
    }
    private func session(_ id: String, work: WorkReference?) throws -> SessionRecord {
        let workData = try JSONEncoder().encode(work)
        let workJSON = String(decoding: workData, as: UTF8.self)
        return try JSONDecoder().decode(SessionRecord.self, from: Data("""
        {"id":"\(id)","kind":"interactive","work":\(workJSON),"title":"\(id)",
         "detail":"codex","cwd":"/src/loopflow","state":"active",
         "ready_summary":null,"terminal_ids":[],"open_argv":["lf","session","open","\(id)"]}
        """.utf8))
    }
}

private actor PlanningGate {
    private var response: CheckedContinuation<Void, Never>?
    private var requested: CheckedContinuation<Void, Never>?
    func wait() async {
        await withCheckedContinuation { continuation in
            response = continuation
            requested?.resume()
            requested = nil
        }
    }
    func started() async {
        if response != nil { return }
        await withCheckedContinuation { requested = $0 }
    }
    func release() { response?.resume(); response = nil }
}

private actor ReadingSource {
    var roadmap: String
    let sessions: String
    var failed = false
    init(roadmap: String, sessions: String) { self.roadmap = roadmap; self.sessions = sessions }
    func fail() { failed = true }
    func replaceRoadmap(_ value: String) { roadmap = value }
    func read(_ args: [String]) throws -> String {
        if failed { throw RegistryQueryError("offline") }
        switch args.first {
        case "roadmap": return roadmap
        case "session": return sessions
        case "ls": return "[]"
        case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        default: throw RegistryQueryError("unexpected command")
        }
    }
}
#endif
