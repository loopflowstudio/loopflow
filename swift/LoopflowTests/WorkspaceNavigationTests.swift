#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Unified Work and Session navigation")
@MainActor
struct WorkspaceNavigationTests {
    @Test("Repository path spellings share one outline root")
    func repositoryAliasesShareRoot() async throws {
        let source = try ReadingSource(
            roadmap: roadmapJSON().replacingOccurrences(of: "\"/src/loopflow\"", with: "\"/src/loopflow/\""),
            sessions: sessionsJSON())
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.navigation.presentation = .full
        let view = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        let roots = try view.inspect().findAll(ViewType.Button.self) {
            (try? $0.accessibilityIdentifier().hasPrefix("workspace-repository-")) == true
        }
        #expect(roots.count == 1)
        #expect(try roots.first?.accessibilityIdentifier() == "workspace-repository-/src/loopflow")
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-session-count-issue-review")
    }

    @Test("Compression promotes leaves without changing identity or hiding upcoming Tasks")
    func compressedOutlinePreservesIdentity() throws {
        var json = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var waves = try #require(json["waves"] as? [[String: Any]])
        waves = [waves[0]]
        waves[0]["unavailable_tasks"] = []
        json["waves"] = waves
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: json))
        let records = try [session("human", work: .task(id: "ts_review00000000000000000000000000")),
                           session("project", work: .project(id: "ps_11111111111111111111111111111111")),
                           session("unknown", work: .task(id: "absent"))]
        let projection = WorkspaceProjection(roadmaps: snapshot.waves, sessions: records)
        func rows(_ mode: WorkspacePresentation, collapsed: Set<WorkspaceNodeKey> = [], readable: Bool = true) -> [WorkspaceOutlineRow] {
            projection.outline(presentation: mode, collapsed: collapsed, search: "", planningReadable: readable)
        }
        let full = rows(.full)
        let compact = rows(.compact)
        let flat = rows(.sessions)
        // Task Sessions ride their Task row; Wave and unmatched Sessions stay leaves.
        func sessionIds(_ rows: [WorkspaceOutlineRow]) -> Set<String> {
            Set(rows.flatMap { ($0.session.map { [$0.id] } ?? []) + $0.inlineSessions.map(\.id) })
        }
        #expect(sessionIds(full) == sessionIds(compact))
        #expect(sessionIds(full) == Set(flat.compactMap { $0.session?.id }))
        #expect(compact.compactMap(\.workKey).allSatisfy { $0.work.kind == .task })
        let review = try #require(compact.first { $0.workKey?.work == .task(id: "issue-review") })
        #expect(review.inlineSessions.map(\.id) == ["human"])
        #expect(!compact.contains { $0.session?.id == "human" })
        // Upcoming work lives in the Wave plan, not the started working set.
        #expect(!compact.contains { $0.workKey?.work == .task(id: "issue-available") })
        #expect(flat.allSatisfy { $0.session != nil && $0.depth == 0 })
        let human = try #require(flat.first { $0.session?.id == "human" })
        #expect(human.ancestors.map(\.key.work.kind) == [.wave, .task])
        let project = projection.waves[0].id
        let folded = rows(.compact, collapsed: [project])
        #expect(folded.contains { $0.workKey == project })
        #expect(!folded.contains { $0.inlineSessions.contains { $0.id == "human" } })
        #expect(rows(.sessions, collapsed: [project]).map(\.id) == flat.map(\.id))
        #expect(rows(.compact, readable: false).map(\.id) == full.map(\.id))
        let searched = projection.outline(presentation: .compact, collapsed: [project], search: "human", planningReadable: true)
        #expect(searched.contains { $0.inlineSessions.contains { $0.id == "human" } })
        let identifierSearch = projection.outline(presentation: .sessions, collapsed: [], search: "W2-131", planningReadable: true)
        #expect(identifierSearch.compactMap { $0.session?.id } == ["human"])
    }

    @Test("Compact keeps an empty Wave available for inspection and conversation")
    func compactRetainsEmptySubjects() throws {
        var json = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var wave = try #require((json["waves"] as? [[String: Any]])?.first)
        wave["tasks"] = ["state": "ok", "items": [], "truncated": false]
        wave["unavailable_tasks"] = []
        json["waves"] = [wave]
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: json))
        let projection = WorkspaceProjection(roadmaps: snapshot.waves, sessions: [])
        let rows = projection.outline(presentation: .compact, collapsed: [], search: "", planningReadable: true)
        #expect(rows.count == 1)
        #expect(rows.first?.workKey?.work.kind == .wave)
        #expect(projection.outline(presentation: .sessions, collapsed: [], search: "", planningReadable: true).isEmpty)
    }

    @Test("Compact preserves Wave context when planning is multiple, partial or unavailable",
          arguments: ["multiple", "partial", "unavailable"])
    func compressionRequiresPlanningEvidence(evidence: String) throws {
        var json = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var wave = try #require((json["waves"] as? [[String: Any]])?.first)
        wave["unavailable_tasks"] = []
        var tasks = try #require(wave["tasks"] as? [String: Any])
        if evidence == "partial" { tasks["truncated"] = true }
        else if evidence == "unavailable" { tasks = ["state": "unavailable", "reason": "offline"] }
        wave["tasks"] = tasks
        var waves = [wave]
        if evidence == "multiple" {
            var second = wave
            var identity = try #require(wave["wave"] as? [String: Any])
            identity["id"] = "another-wave"
            second["wave"] = identity
            second["tasks"] = ["state": "ok", "items": [], "truncated": false]
            waves.append(second)
        }
        json["waves"] = waves
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: json))
        let projection = WorkspaceProjection(roadmaps: snapshot.waves, sessions: [])
        let rows = projection.outline(presentation: .compact, collapsed: [], search: "", planningReadable: true)
        let kinds = rows.compactMap { $0.workKey?.work.kind }
        #expect(!kinds.contains(.project))
        #expect(kinds.filter { $0 == .wave }.count == (evidence == "multiple" ? 2 : 1))
    }

    @Test("Session list disambiguates same-named conversations and retains unknown ancestry")
    func flatSessionsDisambiguate() throws {
        let records = try [session("first", work: .task(id: "ts_review00000000000000000000000000")),
                           session("second", work: .task(id: "ts_review00000000000000000000000000")),
                           session("third", work: nil)].map { record in
            var json = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
            json["title"] = "Conversation"
            return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: json))
        }
        let projection = WorkspaceProjection(roadmaps: try roadmap().waves, sessions: records)
        let rows = projection.outline(presentation: .sessions, collapsed: [], search: "", planningReadable: true)
        #expect(rows.count == 3)
        #expect(Set(rows.map(\.detail)).count == 3)
        #expect(rows.first { $0.session?.id == "first" }?.detail?.contains("first") == true)
        #expect(rows.first { $0.session?.id == "third" }?.detail == "Repository or unavailable ancestry")
    }

    @Test("Hierarchy rows open their Task; the Session list opens the exact Session")
    func sessionLeavesOpenAcrossPresentations() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let selected = model.selection
        var opened: [String] = []
        var tasks: [WorkReference] = []
        for presentation in WorkspacePresentation.allCases {
            model.navigation.presentation = presentation
            let view = WorkspaceNavigator(model: model, onOpenSession: { opened.append($0.id) },
                                          onOpenTask: { tasks.append($0) })
            if presentation == .sessions {
                try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-human").button().tap()
            } else {
                try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-task-issue-review").button().tap()
                #expect(throws: (any Error).self) {
                    try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-human")
                }
            }
            #expect(model.selection == selected)
        }
        #expect(opened == ["human"])
        #expect(tasks == [.task(id: "issue-review"), .task(id: "issue-review")])
    }

    @Test("Autonomous, upcoming and human work share stable ranked planning rows")
    func workDoesNotRequireSessions() throws {
        let roadmap = try roadmap()
        let empty = WorkspaceProjection(roadmaps: roadmap.waves, sessions: [])
        let interactive = try session("human", work: .task(id: "ts_review00000000000000000000000000"))
        let joined = WorkspaceProjection(roadmaps: roadmap.waves, sessions: [interactive])
        let before = empty.waves[0].tasks
        let after = joined.waves[0].tasks

        #expect(before.map(\.id) == after.map(\.id))
        #expect(after.map(\.task.id) == ["issue-later", "issue-review", "issue-now", "issue-available"])
        #expect(after[2].task.condition.state == .clear)
        #expect(after[3].task.runtime == nil)
        #expect(after[1].sessions.map(\.id) == ["human"])
        #expect(after[3].sessions.isEmpty)
        #expect(joined.subject(for: "human") == .task(id: "issue-review"))
        #expect(WorkspaceNavigation().presentation == .compact)
    }

    @Test("The sidebar holds started work; inspection, a prepared checkout or a Session gap never changes it")
    func startedWorkingSet() async throws {
        let model = try model()
        await model.refresh()
        func sidebar() -> [String] {
            model.workspace.outline(presentation: .full, collapsed: [], search: "", planningReadable: true)
                .compactMap { $0.workKey?.work.kind == .task ? $0.workKey?.work.id : nil }
        }
        // Started (review), prepared-but-unstarted (now), upcoming (available),
        // completed (later): only started, incomplete work is in the working set.
        #expect(sidebar() == ["issue-review"])
        model.select(.task(id: "issue-available"))
        #expect(sidebar() == ["issue-review"])
        // Started work survives its Session resolving and any process exit.
        model.sessionResolved("human", repo: "/src/loopflow")
        #expect(sidebar() == ["issue-review"])
        // All planned Tasks remain reachable in the Wave plan.
        #expect(model.workspace.waves[0].tasks.count == 4)
    }

    @Test("Many started Tasks across Waves populate the sidebar while every planned Task stays reachable")
    func denseWorkingSet() throws {
        var json = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        let base = try #require((json["waves"] as? [[String: Any]])?.first)
        let template = try #require(((base["tasks"] as? [String: Any])?["items"] as? [[String: Any]])?.first {
            (($0["task"] as? [String: Any])?["id"] as? String) == "issue-review"
        })
        var waves: [[String: Any]] = []
        for waveIndex in 0..<3 {
            var wave = base
            var identity = try #require(base["wave"] as? [String: Any])
            identity["id"] = "wave-\(waveIndex)"
            identity["name"] = "Wave \(waveIndex)"
            wave["wave"] = identity
            wave["unavailable_tasks"] = []
            let items = (0..<50).map { index -> [String: Any] in
                var item = template
                var task = item["task"] as! [String: Any]
                task["id"] = "task-\(waveIndex)-\(index)"
                task["identifier"] = "T-\(waveIndex)-\(index)"
                task["rank"] = index
                item["task"] = task
                var runtime = item["runtime"] as! [String: Any]
                runtime["work_id"] = "work-\(waveIndex)-\(index)"
                // 20 started Tasks spread across the three Waves.
                let global = waveIndex * 50 + index
                runtime["started"] = global < 140 && global.isMultiple(of: 7)
                item["runtime"] = runtime
                return item
            }
            wave["tasks"] = ["state": "ok", "items": items, "truncated": false]
            waves.append(wave)
        }
        json["waves"] = waves
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: json))
        let projection = WorkspaceProjection(roadmaps: snapshot.waves, sessions: [])
        let rows = projection.outline(presentation: .compact, collapsed: [], search: "", planningReadable: true)
        #expect(rows.filter { $0.workKey?.work.kind == .task }.count == 20)
        #expect(rows.filter { $0.workKey?.work.kind == .wave }.count == 3)
        #expect(projection.waves.allSatisfy { $0.tasks.count == 50 })
        // A poll that re-reads identical evidence yields identical rows.
        let again = WorkspaceProjection(roadmaps: snapshot.waves, sessions: [])
            .outline(presentation: .compact, collapsed: [], search: "", planningReadable: true)
        #expect(again.map(\.id) == rows.map(\.id))
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
        #expect(projection.waves[0].sessions.map(\.id) == ["wave", "project"])
        let completed = try #require(projection.waves[0].tasks.first)
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
        navigation.selectedSessionId = "human"
        #expect(navigation.selection == .task(id: "issue-review"))
        let group = model.workspace.waves[0].id
        navigation.toggle(group)
        navigation.search = "upcoming"
        #expect(navigation.isExpanded(group))
        navigation.toggle(group)
        navigation.search = ""
        #expect(!navigation.isExpanded(group))
        navigation.content = .terminals
        navigation.presentation = .sessions
        model.setRepoPath("/src/context")
        model.select(.wave(id: "wave-2"))
        model.setRepoPath("/src/loopflow")

        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation === navigation)
        #expect(model.navigation.content == .terminals)
        #expect(model.navigation.selectedSessionId == "human")
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
        let keys = model.workspace.waves[0].tasks.map(\.id)
        model.select(.task(id: "issue-review"))
        await source.fail()
        await model.refresh()
        #expect(model.roadmap.errorMessage == "offline")
        #expect(model.sessions.errorMessage == "offline")
        #expect(model.workspace.waves[0].tasks.map(\.id) == keys)
        #expect(model.workspace.subject(for: "human") == model.selection)
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
        waves[0]["tasks"] = truncated
            ? ["state": "ok", "items": [], "truncated": true]
            : ["state": "unavailable", "reason": "planning offline"]
        waves[0]["unavailable_tasks"] = []
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
        model.navigation.selectedSessionId = "human"
        model.sessionResolved("human", repo: "/src/context")
        #expect(model.workspace.subject(for: "human") == model.selection)
        model.sessionResolved("human", repo: "/src/loopflow")
        #expect(model.workspace.subject(for: "human") == nil)
        #expect(model.task(id: "issue-review")?.task.task.completed == false)
        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation.selectedSessionId == nil)
    }

    @Test("Task details expose directive and current chapter proof from the same snapshot")
    func inspectorShowsPlanning() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let task = try #require(model.task(id: "issue-review"))
        let view = WorkSurfaceView(model: model, onNewSession: { _ in })
        #expect(throws: Never.self) { try view.inspect().find(text: task.task.task.name) }
        #expect(throws: Never.self) { try view.inspect().find(text: task.task.task.description) }
        #expect(throws: Never.self) { try view.inspect().find(button: "New session") }
        model.select(.wave(id: task.wave.wave.id))
        #expect(throws: Never.self) { try view.inspect().find(text: task.wave.wave.goal) }
        #expect(throws: Never.self) { try view.inspect().find(text: "Current KRs") }
        #expect(throws: Never.self) { try view.inspect().find(text: task.wave.chapter!.krs[0].text) }
        #expect(throws: Never.self) { try view.inspect().find(viewWithAccessibilityIdentifier: "podium-task-issue-available") }
    }

    @Test("Inspection distinguishes no Sessions from an unavailable reading", arguments: [true, false])
    func inspectorShowsSessionEvidence(hasSession: Bool) async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(.task(id: hasSession ? "issue-review" : "issue-available"))
        let view = WorkSurfaceView(model: model)
        if hasSession {
            #expect(throws: (any Error).self) { try view.inspect().find(text: "No open Sessions.") }
            #expect(throws: Never.self) { try view.inspect().find(viewWithAccessibilityIdentifier: "task-session-human") }
        } else {
            #expect(throws: Never.self) { try view.inspect().find(text: "No open Sessions.") }
        }
        await source.fail()
        await model.refresh()
        #expect(throws: (any Error).self) {
            try WorkSurfaceView(model: model).inspect().find(text: "No open Sessions.")
        }
    }

    @Test("Work outside the plan remains explained in Wave details")
    func unavailableTaskLivesInWaveDetails() async throws {
        let model = try model()
        await model.refresh()
        let wave = try #require(model.workspace.waves.first)
        let task = try #require(wave.roadmap.unavailableTasks.first)
        let text = "\(task.taskIdentifier): \(task.reason) · \(task.recovery)"
        let navigator = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: text) }
        model.select(wave.id.work)
        #expect(throws: Never.self) { try WorkSurfaceView(model: model).inspect().find(text: text) }
    }

    @Test("Incomplete planning keeps out-of-plan Work reachable", arguments: [false, true])
    func incompletePlanningRetainsWork(hasUnplannedWork: Bool) async throws {
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var wave = try #require((snapshot["waves"] as? [[String: Any]])?.first)
        wave["tasks"] = ["state": "ok", "items": [], "truncated": false]
        if !hasUnplannedWork { wave["unavailable_tasks"] = [] }
        snapshot["waves"] = [wave]
        let planning = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let model = PodiumModel(query: RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return planning
            case "session", "ls": return "[]"
            case "ps": return #"{"schema_version":1,"observed_at":1,"nodes":[],"provider_processes":[]}"#
            default: return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            }
        }, repoPath: "/src/loopflow")
        await model.refresh()
        await model.refreshProcessActivity()
        #expect(model.processActivity.errorMessage == nil)
        let navigator = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: "No active Tasks in this repository.") }
        if hasUnplannedWork {
            let projected = try #require(model.workspace.waves.first)
            try navigator.inspect().find(button: projected.roadmap.wave.name).tap()
            #expect(model.selection == projected.id.work)
            let task = try #require(projected.roadmap.unavailableTasks.first)
            #expect(throws: Never.self) {
                try WorkSurfaceView(model: model).inspect().find(text: "\(task.taskIdentifier): \(task.reason) · \(task.recovery)")
            }
        }
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
        #expect(model.workspace.subject(for: "human") == .task(id: "issue-review"))
        #expect(model.workspace.unmatchedSessions.isEmpty)
    }

    @Test("New conversation follows the visible subject and repository scope")
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
    private func session(_ id: String, work: WorkReference?, state: SessionState = .active) throws -> SessionRecord {
        let workData = try JSONEncoder().encode(work)
        let workJSON = String(decoding: workData, as: UTF8.self)
        return try JSONDecoder().decode(SessionRecord.self, from: Data("""
        {"id":"\(id)", "run_id": "\(id)","kind":"interactive","work":\(workJSON),"title":"\(id)",
         "detail":"codex","cwd":"/src/loopflow","state":"\(state.rawValue)",
         "wave_id":\(id == "project" ? "\"wave-1\"" : "null"),"work_path":null,"actions":\(sessionActionFixtureJSON(kind: "interactive", state: state.rawValue)),
         "ready_summary":null,"title_source":"generated","flow_membership":{"kind":"independent"},"terminal_ids":[],"open_argv":["lf","session","open","\(id)"]}
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
