#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Work model")
@MainActor
struct WorkModelTests {
    @Test("Back visits retained Work in repository order without recording its own return")
    func backUsesRepositoryHistory() throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Back must not prepare or launch a conversation")
        }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(fixture.roadmap), waves: .available(fixture.waves),
                           workActivity: .available(fixture.workActivity), repos: [])
        model.select(.wave(id: "wave-1"))
        model.select(.task(id: "issue-now"))
        model.select(.task(id: "issue-now"))
        model.select(nil)
        model.setRepoPath("/src/context")
        #expect(model.navigation.backStack.isEmpty)
        model.goBack()
        #expect(model.selection == nil)
        model.setRepoPath("/src/loopflow")
        model.goBack()
        #expect(model.selection == .task(id: "issue-now"))
        model.goBack()
        #expect(model.selection == .wave(id: "wave-1"))
        model.goBack()
        #expect(model.selection == nil)
        #expect(model.navigation.backStack.isEmpty)
    }

    @Test("Repository scope filters one shared snapshot and clears outside selection")
    func repositoryScopeFiltersSharedSnapshot() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)

        await model.refresh()
        #expect(model.visibleRoadmaps.map(\.wave.name) == ["product", "context"])

        model.select(.wave(id: "wave-1"))
        model.setRepoPath("/src/context")

        #expect(model.visibleRoadmaps.map(\.wave.name) == ["context"])
        #expect(model.visibleWaves.count == 1)
        #expect(model.selection == nil)
    }

    @Test("Registry Wave paths cannot create repository choices")
    func registryWavesDoNotCreateRepos() throws {
        let fixture = try WorkTestFixture.load()
        let staleWorktree = Wave(
            id: "stale",
            name: "stale",
            repo: "/tmp/loopflow.old-task",
            status: .ready
        )
        let model = WorkModel(query: fixture.query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available([staleWorktree]),
            workActivity: .available(fixture.workActivity),
            repos: [PortfolioRepo(path: "/src/loopflow", error: nil)]
        )

        #expect(model.allRepos.map(\.path) == ["/src/loopflow"])
    }

    @Test("Stable Work selection resolves against the latest snapshot")
    func stableSelectionSurvivesRefresh() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)

        await model.refresh()
        model.select(.task(id: "issue-now"))
        await model.refresh()

        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.task(id: "issue-now")?.task.task.name
            == "Make lf roadmap the machine-wide view")

        model.select(.task(id: "missing"))
        #expect(model.selection == nil)
    }

    @Test("Current chapter source references navigate directly to the Wave")
    func chapterReferencesSelectWave() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)
        await model.refresh()
        let wave = try #require(fixture.roadmap.waves.first)
        let chapter = try #require(wave.currentProject)
        for reference in [chapter.id, chapter.slug, chapter.linearId].compactMap({ $0 }) {
            model.select(.project(id: reference))
            #expect(model.selection == .wave(id: wave.wave.id))
        }
    }

    @Test("Selected Task survives a chapter transfer with temporarily absent membership")
    func selectedTaskSurvivesChapterTransfer() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)
        await model.refresh()
        model.select(.task(id: "issue-now"))
        var wire = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
        var waves = try #require(wire["waves"] as? [[String: Any]])
        var tasks = try #require(wire["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        let index = try #require(items.firstIndex { ($0["task"] as? [String: Any])?["id"] as? String == "issue-now" })
        var planning = try #require(items[index]["task"] as? [String: Any])
        planning["name"] = "Updated while selected"
        items[index]["task"] = planning
        tasks["items"] = items
        wire["tasks"] = tasks
        let updated = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(updated), waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity), repos: [])
        #expect(model.task(id: "issue-now")?.task.task.name == "Updated while selected")
        var projects = try #require(waves[0]["projects"] as? [String: Any])
        var plans = try #require(projects["items"] as? [[String: Any]])
        var successor = plans[0]
        successor["id"] = "next"
        successor["name"] = "Next chapter"
        plans[0]["current"] = false
        plans.append(successor)
        projects["items"] = plans
        waves[0]["projects"] = projects
        wire["waves"] = waves
        let transferring = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(transferring), waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity), repos: [])
        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.task(id: "issue-now")?.task.task.identifier == "W2-144")
        #expect(model.task(id: "issue-now")?.task.task.name == "Updated while selected")
        #expect(model.task(id: "issue-now")?.wave!.projects.items.last?.id == "next")
    }

    @Test("Chapter history remains reachable when the current plan is unavailable")
    func historyOpensWithoutCurrentChapter() throws {
        let fixture = try WorkTestFixture.load()
        var wire = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
        var waves = try #require(wire["waves"] as? [[String: Any]])
        waves[0]["projects"] = ["state": "unavailable", "reason": "Provider unavailable"]
        wire["waves"] = waves
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        let model = WorkModel(query: fixture.query)
        model.applyFixture(roadmap: .available(roadmap), waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity), repos: [])
        model.select(.wave(id: "wave-1"))

        let view = WorkSurfaceView(model: model)
        try view.inspect().find(button: "Project history").tap()

        #expect(model.historyWave?.id == "wave-1")
        #expect(model.selection == .wave(id: "wave-1"))
    }

    @Test("A delayed historical reference cannot replace newer navigation", arguments: ["history", "task", "repo"])
    func historicalReferenceRespectsNavigation(destination: String) async throws {
        let fixture = try WorkTestFixture.load()
        let deferred = DeferredActivityResponse()
        let model = WorkModel(query: RegistryQuery { _, _ in await deferred.response() }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(fixture.roadmap), waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity), repos: [])
        model.select(.project(id: "old-plan"))
        let lookup = try #require(model.historyLookup)
        await deferred.waitUntilRequested()
        if destination == "task" { model.select(.task(id: "issue-now")) }
        if destination == "repo" { model.setRepoPath("/src/context") }
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
        var historical = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixtures.appendingPathComponent("wave_detail.json"))) as? [String: Any])
        historical["projects"] = ["state": "ok", "truncated": false, "items": [[
            "id": "old-plan", "linear_id": NSNull(), "slug": "old", "name": "Previous",
            "workflow": "feature", "status": "completed", "current": false, "metric_targets": [], "krs": [],
            "sync": ["connected": false, "changes": []]
        ]]]
        let reply = try JSONSerialization.data(withJSONObject: historical)
        await deferred.release(try #require(String(data: reply, encoding: .utf8)))
        await lookup.value
        switch destination {
        case "history":
            #expect(model.selection == .wave(id: "wave-1"))
            #expect(model.historyWave?.id == "wave-1")
            #expect(model.historyReference == "old-plan")
        case "task":
            #expect(model.selection == .task(id: "issue-now"))
            #expect(model.historyWave == nil)
        default:
            #expect(model.repoPath == "/src/context")
            #expect(model.selection == nil)
            #expect(model.historyWave == nil)
        }
    }

    @Test("Refresh failure preserves last-good evidence and exposes the reason")
    func refreshFailurePreservesLastGoodEvidence() async throws {
        let fixture = try WorkTestFixture.load()
        let failing = RegistryQuery { _, _ in
            throw RegistryQueryError("registry unavailable")
        }
        let model = WorkModel(query: failing)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        await model.refresh()

        #expect(model.roadmap.value == fixture.roadmap)
        #expect(model.roadmap.errorMessage == "registry unavailable")
        #expect(model.waves.value == fixture.waves)
        #expect(model.waves.errorMessage == "registry unavailable")
        #expect(model.workActivity.value == fixture.workActivity)
        #expect(model.workActivity.errorMessage == "registry unavailable")
        #expect(model.visibleWaves.count == 2)
    }

    @Test("Authored Waves remain visible without active Runs")
    func authoredWavesRemainVisible() async throws {
        let fixture = try WorkTestFixture.load()
        let repo = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-authored-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: repo) }
        for name in ["infrastructure", "intelligence", "product"] {
            let wave = repo
                .appendingPathComponent(".lf/wave", isDirectory: true)
                .appendingPathComponent(name, isDirectory: true)
            try FileManager.default.createDirectory(at: wave, withIntermediateDirectories: true)
            try Data("# \(name)\n".utf8).write(to: wave.appendingPathComponent("GOAL.md"))
        }
        try git(["init", "-q"], at: repo)

        let model = WorkModel(query: fixture.query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available([]),
            workActivity: .available(fixture.workActivity),
            repos: []
        )
        await model.applyPortfolio(
            [PortfolioRepo(path: repo.resolvingSymlinksInPath().path, error: nil)], initialRepoPath: nil
        )
        #expect(model.repos.count == 1)
        model.setRepoPath(repo.path)

        #expect(model.visibleWaves.map(\.displayName) == [
            "Infrastructure", "Intelligence", "Product",
        ])
    }

    @Test("A checkout's repository identity follows its origin once that is resolved")
    func repoIdentityFollowsResolvedOrigin() throws {
        let fixture = try WorkTestFixture.load()
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-identity-\(UUID().uuidString)", isDirectory: true)
        let origin = root.appendingPathComponent("repo", isDirectory: true)
        let worktree = root.appendingPathComponent("repo.wt", isDirectory: true)
        try FileManager.default.createDirectory(at: origin, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try git(["init", "-q"], at: origin)
        try git(
            ["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"],
            at: origin
        )
        try git(["worktree", "add", "-q", worktree.path], at: origin)

        let model = WorkModel(query: fixture.query, repoPath: origin.path)
        #expect(model.repoIdentity(worktree.path) != model.repoIdentity(origin.path))
        _ = WaveOrigin.resolve(worktree.path)
        #expect(model.repoIdentity(worktree.path) == model.repoIdentity(origin.path))
    }

    @Test("Work selection becomes one server-side Activity filter")
    func workActivityFollowsSelection() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)
        await model.refresh()

        model.select(.wave(id: "wave-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "history", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.project(id: "project-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "history", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.task(id: "issue-now"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "history", "--since", "7d", "--limit", "50",
            "--wave", "product",
            "--task", "W2-144", "--json",
        ])
    }

    @Test("A late Activity query cannot replace evidence for a newer selection")
    func staleActivityQueryDoesNotReplaceNewSelection() async throws {
        let fixture = try WorkTestFixture.load()
        let staleJSON = try fixture.workActivityJSON(replacingFirstSubjectWith: "stale-wave")
        let selectedJSON = try fixture.workActivityJSON(replacingFirstSubjectWith: "W2-144")
        let deferred = DeferredActivityResponse()
        let query = RegistryQuery { args, _ in
            guard args.first == "history" else {
                throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
            }
            if args.contains("W2-144") { return selectedJSON }
            return await deferred.response()
        }
        let model = WorkModel(query: query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        model.select(.wave(id: "wave-1"))
        let staleRefresh = Task { await model.refreshWorkActivity() }
        await deferred.waitUntilRequested()

        model.select(.task(id: "issue-now"))
        await model.refreshWorkActivity()
        #expect(model.workActivity.value?.items.first?.subject == "W2-144")

        await deferred.release(staleJSON)
        await staleRefresh.value

        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.workActivity.value?.items.first?.subject == "W2-144")
    }

    @Test("Session refresh reads Task GraphSteps")
    func sessionRefreshReadsTaskGraphSteps() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let json = try String(
            contentsOf: fixtures.appendingPathComponent("sessions.json"),
            encoding: .utf8
        )
        let query = RegistryQuery { args, cwd in
            #expect(args == ["desktop", "sessions", "--json", "--page", "--limit", "100"])
            #expect(cwd == "/src/loopflow")
            return #"{"entries":\#(json),"next":null}"#
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")

        await model.refreshSessions()

        #expect(model.sessions.value?.map(\.id) == [
            "task_00000000000000000000000000000001:task-design:review_kickoff:0",
        ])
    }

    @Test("Changing repository clears cached Sessions")
    func changingRepositoryClearsCachedSessions() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let json = try String(
            contentsOf: fixtures.appendingPathComponent("sessions.json"),
            encoding: .utf8
        )
        let query = RegistryQuery { args, _ in
            #expect(args == ["desktop", "sessions", "--json", "--page", "--limit", "100"])
            return #"{"entries":\#(json),"next":null}"#
        }
        let model = WorkModel(query: query, repoPath: "/src/first")
        await model.refreshSessions()
        #expect(model.sessions.value?.count == 1)

        model.setRepoPath("/src/second")

        #expect(model.sessions.value == nil)
    }

    @Test("A late Session refresh cannot cross repository scope")
    func staleSessionRefreshDoesNotReplaceNewRepository() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let json = try String(
            contentsOf: fixtures.appendingPathComponent("sessions.json"),
            encoding: .utf8
        )
        let deferred = DeferredActivityResponse()
        let query = RegistryQuery { args, _ in
            #expect(args == ["desktop", "sessions", "--json", "--page", "--limit", "100"])
            return #"{"entries":\#(await deferred.response()),"next":null}"#
        }
        let model = WorkModel(query: query, repoPath: "/src/first")
        let refresh = Task { await model.refreshSessions() }
        await deferred.waitUntilRequested()

        model.setRepoPath("/src/second")
        await deferred.release(json)
        await refresh.value

        #expect(model.sessions.value == nil)
    }

    private func git(_ args: [String], at directory: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", directory.path] + args
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try process.run()
        process.waitUntilExit()
        try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
    }
}

@Suite("Work observation stream", .serialized)
@MainActor
struct WorkModelStreamTests {
    @Test("Data-directory frames and confirmed Machine identity preserve the same workspace")
    func streamDirectoryIsNotMachineIdentity() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        model.confirmMachine("machine-local")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        try await eventually { model.task(id: "issue-now") != nil }
        model.select(.task(id: "issue-now"))
        model.confirmMachine("machine-local")
        #expect(model.task(id: "issue-now") != nil)
        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.observingMachineId == "machine-local")
        await feed.send(try fixture.planningFrame(sequence: 2, answers: scope.id, renaming: "Updated"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "Updated" }
        model.confirmMachine("machine-local")
        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.observingMachineId == "machine-local")
    }

    @Test("Failed or truncated planning retains saved rows, selection, panes and cache")
    func incompletePlanningRetainsSavedWork() async throws {
        let fixture = try WorkTestFixture.load()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = WorkCache(directory: directory)
        cache.saveRoadmap(fixture.roadmapJSON)
        cache.saveSelection(.task(id: "issue-review"), repo: "/src/loopflow")
        cache.flush()
        let cacheURL = directory.appendingPathComponent("workspace.json")
        let feed = WorkFeed()
        let registry = SessionsWorkspaceRegistry()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow", cache: cache)
        model.bindWorkspaces(registry, repository: "/src/loopflow")
        let key = WorkNodeKey(repo: "/src/loopflow", work: .task(id: "issue-review"))
        #expect(model.selection == nil) // Saved data does not restore last launch's Task.
        model.select(key.work)
        let workspaceKey = model.workspaceKey(for: key.work, repository: "/src/loopflow")
        let workspace = try #require(registry.retained(workspaceKey))
        workspace.multiplexer.load(sessionId: "retained")
        let layout = workspace.multiplexer.layout
        model.navigation.renaming = SessionRenameDraft(sessionId: "retained", text: "Unsent name")
        let initial = model.visibleWork.waves.flatMap(\.tasks)
        #expect(initial.contains { $0.id == key })
        // Explicit opening now persists the working slot before any failed read.
        cache.flush()
        let saved = try Data(contentsOf: cacheURL)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(WorkFrame(sequence: 1, answers: scope.id,
            unavailable: "sqlite: no such column: t.paused", content: .planning(nil), wire: nil))
        try await eventually { model.roadmap.errorMessage?.contains("t.paused") == true }
        #expect(model.visibleWork.waves.flatMap(\.tasks).map(\.id) == initial.map(\.id))
        #expect(model.selection == key.work)
        #expect(workspace.multiplexer.layout == layout)
        for (index, truncated) in [false, true].enumerated() {
            var wire = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
            let failure: [String: Any] = truncated
                ? ["state": "ok", "items": [], "truncated": true]
                : ["state": "unavailable", "reason": "Task snapshot: no such column: t.paused"]
            wire["tasks"] = failure
            let roadmap = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
            await feed.send(try fixture.frame("planning", sequence: index + 2, answers: scope.id, body: roadmap))
            try await eventually { model.roadmap.errorMessage?.contains(truncated ? "incomplete" : "Task snapshot") == true }
            #expect(model.visibleWork.waves.flatMap(\.tasks).map(\.id) == initial.map(\.id))
            #expect(model.selection == key.work)
            #expect(model.navigation.renaming?.text == "Unsent name")
            #expect(registry.retained(workspaceKey) === workspace)
            #expect(workspace.multiplexer.layout == layout)
            #expect(model.showsSavedPlanning)
            cache.flush()
            #expect(try Data(contentsOf: cacheURL) == saved)
        }
        await feed.send(try fixture.planningFrame(sequence: 4, answers: scope.id, renaming: "Recovered"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "Recovered" }
        #expect(model.roadmap.errorMessage == nil)
        #expect(!model.showsSavedPlanning)
    }

    @Test("Task Workflow survives planning failure and becomes stale when its reader disconnects")
    func workflowSurvivesPlanningFailure() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let initial = try await feed.request(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: initial.id))
        try await eventually { model.task(id: "issue-now") != nil }
        model.select(.task(id: "issue-now"))
        model.syncWorkScope()
        let scope = try await feed.request(2)
        let found = try #require(model.task(id: "issue-now"))
        let workURL = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/task_work.json")
        let work = try String(contentsOf: workURL, encoding: .utf8)
        let shown = WorkScope(repo: "/src/loopflow", headless: false, task: found.task.task.identifier,
                              activity: nil)
        await feed.send(try fixture.frame("task", sequence: 2, answers: scope.id, scope: shown,
            body: "{\"overview\":\(work),\"comments\":{\"identifier\":\"\(found.task.task.identifier)\",\"comments\":[],\"conflicts\":{},\"pending_sync\":[],\"refresh_error\":null},\"graph_processes\":[],\"execution\":null}"))
        try await eventually { model.taskWork[found.task.id].value?.graph != nil }
        let workflow = model.taskWork[found.task.id].value?.graph
        await feed.send(WorkFrame(sequence: 3, answers: scope.id,
            unavailable: "sqlite: no such column: t.paused", content: .planning(nil), wire: nil))
        try await eventually { model.roadmap.errorMessage != nil }
        #expect(model.taskWork[found.task.id].value?.graph == workflow)
        let header = TaskWorkflowHeader(model: model, task: found.task, work: model.taskWork[found.task.id])
        #expect(throws: (any Error).self) { try header.inspect().find(text: "No workflow yet") }
        #expect(try header.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-move").isDisabled())
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.taskWork[found.task.id].errorMessage == "reader exited" }
        #expect(model.taskWork[found.task.id].value?.graph == workflow)
    }

    @Test("Peer status updates independently, preserves failures and rejects old scopes")
    func peerPlanningFrames() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/peer_planning_status.json")
        let statuses = String(decoding: try Data(contentsOf: url), as: UTF8.self)
        func peers(_ sequence: Int, _ answers: Int, repo: String, statuses: String) throws -> WorkFrame {
            let body = #"{"transport":"git","destinations":\#(statuses)}"#
            return try fixture.frame("peer_planning", sequence: sequence, answers: answers,
                                     scope: WorkScope(repo: repo, headless: false, task: nil, activity: nil),
                                     body: body)
        }
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        await feed.send(try peers(2, scope.id, repo: "/src/loopflow", statuses: statuses))
        try await eventually { model.peerPlanning.value?.count == 2 }
        #expect(model.peerPlanning.value?[0].pendingLocal == true)
        #expect(model.peerPlanning.value?[1].active == true)
        #expect(model.roadmap.value != nil)

        await feed.send(WorkFrame(sequence: 3, answers: scope.id,
                                 unavailable: "sync read failed", content: .peerPlanning(nil), wire: nil))
        try await eventually { model.peerPlanning.errorMessage == "sync read failed" }
        #expect(model.peerPlanning.value?.count == 2)
        #expect(model.roadmap.errorMessage == nil)
        await feed.send(try peers(4, scope.id, repo: "/src/loopflow", statuses: statuses))
        try await eventually { model.peerPlanning.errorMessage == nil }

        model.setRepoPath("/src/context")
        model.syncWorkScope()
        let changed = try await feed.request(2)
        // Even a later sequence cannot publish an answer from the old scope.
        await feed.send(try peers(5, scope.id, repo: "/src/loopflow", statuses: statuses))
        await feed.send(try peers(6, changed.id, repo: "/src/loopflow", statuses: statuses))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.peerPlanning.value == nil)
        await feed.send(try peers(7, changed.id, repo: "/src/context", statuses: "[]"))
        try await eventually { model.peerPlanning.value == [] }
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.peerPlanning.errorMessage == "reader exited" }
        #expect(model.peerPlanning.value == [])
        try await feed.opened(2, within: .seconds(5))
        let reopened = try await feed.request(3)
        await feed.send(try peers(1, reopened.id, repo: "/src/context", statuses: statuses))
        try await eventually { model.peerPlanning.value?.count == 2 && model.peerPlanning.errorMessage == nil }
    }

    @Test("Frames update planning in place and an older frame cannot replace a newer one")
    func framesReplaceReadings() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }

        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        model.select(.task(id: "issue-now"))
        let renamed = try fixture.planningFrame(sequence: 3, answers: nil, renaming: "Renamed elsewhere")
        await feed.send(renamed)
        try await eventually { model.task(id: "issue-now")?.task.task.name == "Renamed elsewhere" }
        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.workStatus == .current)

        // Sequence 2 was read before sequence 3; another Machine's frame names nothing here.
        await feed.send(try fixture.planningFrame(sequence: 2, answers: nil))
        await feed.send(try fixture.activityFrame(sequence: 4))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.task(id: "issue-now")?.task.task.name == "Renamed elsewhere")
        #expect(!model.roadmap.isLoading)
    }

    @Test("Another Machine replaces what the previous Machine showed")
    func anotherMachineDropsPreviousContent() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        model.confirmMachine("machine-a")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }

        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        let session = try fixture.sessionEntries()
        await feed.send(try fixture.sessionsFrame(sequence: 2, answers: scope.id, repo: "/src/loopflow", entries: session.json))
        try await eventually { model.sessions.value?.count == session.ids.count }
        model.select(.task(id: "issue-now"))

        // The other Machine has the same repository path and none of these Sessions.
        model.confirmMachine("machine-b")
        await feed.send(try fixture.planningFrame(sequence: 4, answers: scope.id, renaming: "Other Machine"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "Other Machine" }
        #expect(model.selection == nil)
        #expect(model.sessions.value == nil)
    }

    @Test("A preview copy is marked in the breadcrumbs and is its own data identity")
    func previewCopyIsMarked() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        let crumbs = WorkBreadcrumbs(model: model)

        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        try await eventually { model.task(id: "issue-now") != nil }
        #expect(throws: (any Error).self) { try crumbs.inspect().find(viewWithAccessibilityIdentifier: "preview-copy") }

        model.select(.task(id: "issue-now"))
        // A draft-carrying build replaces the reader; its observation names the copy.
        await feed.serve(copy: NSHomeDirectory() + "/.lf/copies/3f2a9c1b")
        await feed.fail(RegistryQueryError("lf replaced"))
        try await feed.opened(2, within: .seconds(5))
        let reopened = try await feed.request(2)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: reopened.id, renaming: "On the copy"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "On the copy" }
        // The copy holds different data than the shared store it came from.
        #expect(model.selection == nil)
        let marker = try crumbs.inspect().find(viewWithAccessibilityIdentifier: "preview-copy")
        #expect(try marker.help().string() == "Preview copy: ~/.lf/copies/3f2a9c1b — work here is not in the shared store")
    }

    @Test("A Task completed elsewhere leaves the working set and stays under Completed")
    func completedTaskLeavesWorkingSet() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        let listed = { (filter: TaskHistoryFilter) -> Bool in
            guard let row = model.task(id: "issue-now")?.task else { return false }
            return filter.includes(row.task, condition: row.condition, now: Date())
        }
        var completed = TaskHistoryFilter()
        completed.showCompleted = true

        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        model.select(.task(id: "issue-now"))
        #expect(listed(TaskHistoryFilter()))

        await feed.send(try fixture.planningFrame(sequence: 2, answers: nil, completing: "issue-now"))
        try await eventually { model.task(id: "issue-now")?.task.task.isTerminal == true }
        #expect(!listed(TaskHistoryFilter()))
        #expect(listed(completed))
        // Still in the reading, so the open Task is not deselected.
        #expect(model.selection == .task(id: "issue-now"))
        #expect(!model.roadmap.isLoading)
    }

    @Test("Refresh returns when a planning frame answers it")
    func refreshWaitsForItsAnswer() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }

        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let finished = Flag()
        let refreshing = Task { await model.refresh(); await finished.set() }
        let scope = try await feed.request(1)
        let refresh = try await feed.request(2)
        #expect(refresh == .refresh(id: refresh.id))
        // Read before the request: not an answer.
        await feed.send(try fixture.planningFrame(sequence: 2, answers: scope.id, renaming: "Stale"))
        try await Task.sleep(for: .milliseconds(50))
        #expect(await finished.value == false)
        #expect(model.task(id: "issue-now")?.task.task.name != "Stale")
        await feed.send(try fixture.planningFrame(sequence: 3, answers: refresh.id, renaming: "Answered"))
        await refreshing.value
        #expect(model.task(id: "issue-now")?.task.task.name == "Answered")
    }

    @Test("Plan changes invalidate an open catalogue without unrelated reloads")
    func storedDefinitionsFollowPlanning() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/graph_catalog.json")
        let original = try String(contentsOf: path, encoding: .utf8)
        actor Catalog {
            var content: String
            var reads = 0
            var fails = false
            init(_ content: String) { self.content = content }
            func update(_ content: String) { self.content = content }
            func fail() { fails = true }
            func read() throws -> String {
                reads += 1
                if fails { throw RegistryQueryError("catalog unavailable") }
                return content
            }
        }
        let catalog = Catalog(original)
        let query = RegistryQuery(watchWork: { await feed.open() }) { args, _ in
            guard args.starts(with: ["graph", "list"]) else {
                throw RegistryQueryError("unexpected catalogue operation")
            }
            return try await catalog.read()
        }
        let model = WorkModel(query: query)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        await model.loadGraphCatalog()
        try await eventually { model.workflowCatalog.value?.isEmpty == false }
        let baseline = await catalog.reads
        await catalog.update("[]")
        await feed.send(try fixture.activityFrame(sequence: 2))
        try await Task.sleep(for: .milliseconds(50))
        #expect(await catalog.reads == baseline)
        #expect(model.workflowCatalog.value?.isEmpty == false)
        await feed.send(try fixture.planningFrame(sequence: 3, answers: nil))
        try await eventually { model.workflowCatalog.value?.isEmpty == true }
        #expect(await catalog.reads == baseline + 1)
        // Both catalogues are one read: returning from an editor rereads it once.
        await model.rereadDefinitions()
        #expect(await catalog.reads == baseline + 2)
        #expect(model.graphCatalog.value?.isEmpty == true)
        await catalog.fail()
        await feed.send(try fixture.planningFrame(sequence: 4, answers: nil))
        try await eventually { model.workflowCatalog.errorMessage == "catalog unavailable" }
        #expect(model.workflowCatalog.value?.isEmpty == true)
    }

    @Test("A late catalogue failure cannot replace a newer successful read")
    func catalogueKeepsNewerReading() async throws {
        let deferred = DeferredActivityResponse()
        actor Source {
            var first = true
            func read(_ deferred: DeferredActivityResponse) async -> String {
                if first {
                    first = false
                    return await deferred.response()
                }
                return "[]"
            }
        }
        let source = Source()
        let model = WorkModel(query: RegistryQuery { _, _ in await source.read(deferred) })
        let old = Task { await model.loadGraphCatalog() }
        await deferred.waitUntilRequested()
        await model.loadGraphCatalog(force: true)
        await deferred.release("invalid old response")
        await old.value
        #expect(model.workflowCatalog.value?.isEmpty == true)
        #expect(model.workflowCatalog.errorMessage == nil)
    }

    @Test("A mutation waits for its checkpoint without asking for broad refresh")
    func mutationConfirmation() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let query = RegistryQuery(watchWork: { await feed.open() }) { args, _ in
            guard args.starts(with: ["task", "hold"]) else {
                throw RegistryQueryError("unexpected read: \(args)")
            }
            return ""
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let found = try #require(model.task(id: "issue-now"))
        let owner = model.navigation
        let action = Task { await model.deferTask(found.task, deferred: true) }
        let request = try await feed.request(2)
        #expect(request == .checkpoint(id: request.id, replay: []))
        await feed.send(try fixture.planningFrame(sequence: 2, answers: request.id, renaming: "Committed"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "Committed" }
        #expect(owner.taskRunDrafts[found.task.id]?.phase == .awaitingObservation)
        model.setRepoPath("/src/context")
        model.navigation.taskRunDrafts[found.task.id] = TaskRunDraft(error: "New draft")
        await feed.send(try fixture.frame("checkpoint", sequence: 3, answers: request.id, body: "null"))
        await action.value
        #expect(owner.taskRunDrafts[found.task.id] == nil)
        #expect(model.navigation.taskRunDrafts[found.task.id]?.error == "New draft")
    }

    @Test("A successful Task update stops waiting despite unrelated frames, and a late checkpoint is harmless")
    func taskConfirmationDeadline() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let query = RegistryQuery(watchWork: { await feed.open() }) { args, _ in
            guard args.starts(with: ["task", "complete"]) else { throw RegistryQueryError("unexpected command") }
            try await Task.sleep(for: .milliseconds(100))
            return ""
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let found = try #require(model.task(id: "issue-now"))
        let action = Task { await model.completeTask(found.task) }
        try await eventually { model.navigation.taskRunDrafts[found.task.id]?.phase == .runningCommand }
        let request = try await feed.request(2)
        #expect(model.navigation.taskRunDrafts[found.task.id]?.phase == .awaitingObservation)
        let heartbeats = Task {
            for sequence in 2...14 {
                await feed.send(try fixture.activityFrame(sequence: sequence))
                try await Task.sleep(for: .seconds(1))
            }
        }
        defer { heartbeats.cancel() }
        await action.value
        let draft = try #require(model.navigation.taskRunDrafts[found.task.id])
        #expect(!draft.acting)
        #expect(draft.error?.hasPrefix("Task update accepted; refresh not confirmed.") == true)
        #expect(model.task(id: "issue-now") != nil)
        await feed.send(try fixture.frame("checkpoint", sequence: 16, answers: request.id, body: "null"))
        let confirming = Task { try await model.confirmWorkUpdate() }
        let next = try await feed.request(3)
        await feed.send(try fixture.frame("checkpoint", sequence: 17, answers: next.id, body: "null"))
        try await confirming.value
    }

    @Test("Only the requested checkpoint confirms a write, including an unchanged result")
    func unchangedConfirmation() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let finished = Flag()
        let confirming = Task { try await model.confirmWorkUpdate(); await finished.set() }
        let request = try await feed.request(2)
        await feed.send(try fixture.frame("checkpoint", sequence: 2, answers: request.id - 1, body: "null"))
        await feed.send(try fixture.activityFrame(sequence: 3))
        await feed.send(try fixture.planningFrame(sequence: 4, answers: request.id))
        try await eventually { model.planningSequence == 4 }
        #expect(await finished.value == false)
        await feed.send(try fixture.frame("checkpoint", sequence: 5, answers: request.id, body: "null"))
        try await confirming.value
        #expect(await finished.value)
    }

    @Test("A failed confirmation retains content and reports failure", arguments: [false, true])
    func failedConfirmation(readerEnds: Bool) async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let confirming = Task { try await model.confirmWorkUpdate() }
        let request = try await feed.request(2)
        if readerEnds {
            await feed.fail(RegistryQueryError("reader lost"))
        } else {
            await feed.send(WorkFrame(sequence: 2, answers: request.id,
                                      unavailable: "read failed", content: .checkpoint, wire: nil))
        }
        do {
            try await confirming.value
            Issue.record("unconfirmed write succeeded")
        } catch {
            #expect(error.localizedDescription.contains(readerEnds ? "ended" : "read failed"))
        }
        #expect(model.task(id: "issue-now") != nil)
    }

    @Test("New turns clear previews and an ended reader recovers without reviving old output")
    func readerEndRecovers() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        let now = Date(timeIntervalSince1970: 1000)
        let entries = try fixture.sessionEntries()
        let original = try #require((JSONSerialization.jsonObject(with: Data(entries.json.utf8)) as? [[String: Any]])?.first)
        var answers = 0
        func sessionFrame(_ sequence: Int, turn: String?, available: Bool = false) throws -> WorkFrame {
            var entry = original
            entry["state"] = "active"
            entry["task_ids"] = ["preview-task"]
            entry["program_status"] = available ? NSNull() : ["seen": true, "records": [[
                "state": "working", "id": NSNull(), "kind": NSNull(), "progress": NSNull(),
                "app": NSNull(), "title": NSNull(), "msg": NSNull()
            ]]] as Any
            entry["availability"] = available ? "available" : NSNull()
            entry["agent_process_id"] = "retained-agent"
            entry["preview"] = turn.map {
                ["provider_thread": "thread", "provider_turn": $0, "source": "codex",
                 "observed_at": 995, "text": $0] as [String: Any]
            } ?? NSNull() as Any
            let json = String(decoding: try JSONSerialization.data(withJSONObject: [entry]), as: UTF8.self)
            return try fixture.sessionsFrame(sequence: sequence, answers: answers, repo: "/src/loopflow", entries: json)
        }
        func preview() -> String {
            let records = model.sessions.value ?? []
            return SessionProgramStatus(sessions: records, unavailable: model.sessions.errorMessage)
                .preview(now: now, fallback: "brief", sessions: records).text
        }

        try await feed.opened(1)
        answers = try await feed.request(1).id
        await feed.send(try fixture.planningFrame(sequence: 1, answers: answers))
        await feed.send(try sessionFrame(2, turn: "Previous result", available: true))
        try await eventually { preview() == "Previous result" }
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [
            fixtureViewContent(work: .init(repository: "repo", kind: .task, id: "preview-task"), title: "Preview")
        ])], reason: "")
        model.navigation.sidebarFilter = .available
        #expect(model.sidebarTaskIDs == ["preview-task"])
        await feed.send(try sessionFrame(3, turn: nil))
        try await eventually { preview() == "Summary · brief" }
        await feed.send(try sessionFrame(4, turn: "New turn output"))
        try await eventually { preview() == "New turn output" }
        model.navigation.sidebarFilter = .busy
        #expect(model.sidebarTaskIDs == ["preview-task"])
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.sessions.errorMessage == "reader exited" }
        #expect(model.sessions.value?.first?.preview?.text == "New turn output")
        #expect(preview() == "Summary · brief")
        #expect(model.sidebarTaskIDs.isEmpty)
        model.navigation.sidebarFilter = .available
        #expect(model.sidebarTaskIDs.isEmpty)
        model.navigation.sidebarFilter = .working
        #expect(model.sidebarTaskIDs == ["preview-task"])
        #expect(model.roadmap.value != nil)
        #expect(!model.roadmap.isLoading)

        try await feed.opened(2, within: .seconds(5))
        answers = try await feed.request(2).id
        // Planning recovery alone cannot revive the old Session reading.
        await feed.send(try fixture.planningFrame(sequence: 1, answers: answers, renaming: "After restart"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "After restart" }
        #expect(preview() == "Summary · brief")
        await feed.send(try sessionFrame(2, turn: nil))
        try await eventually { model.sessions.errorMessage == nil }
        #expect(preview() == "Summary · brief")
        await feed.send(try sessionFrame(3, turn: "Recovered result", available: true))
        try await eventually { preview() == "Recovered result" }
    }

    private func eventually(_ condition: @MainActor () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(3)
        while !condition() {
            guard ContinuousClock.now < deadline else {
                Issue.record("condition did not hold in time")
                throw CancellationError()
            }
            try await Task.sleep(for: .milliseconds(5))
        }
    }
}

private actor Flag {
    private(set) var value = false
    func set() { value = true }
}

/// A scripted reader: the test decides which frames arrive and when.
private actor WorkFeed {
    private var continuations: [AsyncThrowingStream<WorkFrame, any Error>.Continuation] = []
    private var requests: [WorkRequest] = []
    /// The preview copy the next reader's build works on.
    private var copy: String?

    nonisolated func open() async -> WorkObservation {
        let (stream, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        let copy = await add(continuation)
        return WorkObservation(frames: stream, request: { request in
            Task { await self.record(request) }
        }, cancel: { continuation.finish() }, copy: copy)
    }

    func serve(copy: String?) { self.copy = copy }

    private func add(_ continuation: AsyncThrowingStream<WorkFrame, any Error>.Continuation) -> String? {
        continuations.append(continuation)
        return copy
    }

    private func record(_ request: WorkRequest) { requests.append(request) }
    func send(_ frame: WorkFrame) { continuations.last?.yield(frame) }
    func fail(_ error: any Error) { continuations.last?.finish(throwing: error) }

    func opened(_ count: Int, within: Duration = .seconds(3)) async throws {
        let deadline = ContinuousClock.now + within
        while continuations.count < count {
            guard ContinuousClock.now < deadline else { throw RegistryQueryError("reader \(count) never opened") }
            try await Task.sleep(for: .milliseconds(5))
        }
    }

    /// The request with this id, once it has been made.
    func request(_ id: Int) async throws -> WorkRequest {
        let deadline = ContinuousClock.now + .seconds(3)
        while true {
            if let request = requests.first(where: { $0.id == id }) { return request }
            guard ContinuousClock.now < deadline else { throw RegistryQueryError("request \(id) was never made") }
            try await Task.sleep(for: .milliseconds(5))
        }
    }
}

private struct WorkTestFixture {
    let roadmap: RoadmapSnapshot
    let waves: [Wave]
    let workActivity: WorkActivitySnapshot
    let workActivityData: Data
    let roadmapJSON: String
    let wavesJSON: String
    let workActivityJSON: String
    let activityArguments: ActivityArguments
    let query: RegistryQuery

    static func load(sourceFile: String = #filePath) throws -> WorkTestFixture {
        let fixtures = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let roadmapData = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: roadmapData)
        let workActivityData = try Data(
            contentsOf: fixtures.appendingPathComponent("work_activity_snapshot.json")
        )
        let workActivity = try JSONDecoder().decode(
            WorkActivitySnapshot.self,
            from: workActivityData
        )
        let object = try #require(JSONSerialization.jsonObject(with: roadmapData) as? [String: Any])
        let roadmapWaves = try #require(object["waves"] as? [[String: Any]])
        let waveObjects = try roadmapWaves.map { try #require($0["wave"] as? [String: Any]) }
        let waveData = try JSONSerialization.data(withJSONObject: waveObjects)
        let snapshots = try JSONDecoder().decode([WaveSnapshot].self, from: waveData)
        let waves = snapshots.map { $0.toWave() }
        let roadmapJSON = try #require(String(data: roadmapData, encoding: .utf8))
        let wavesJSON = try #require(String(data: waveData, encoding: .utf8))
        let workActivityJSON = try #require(String(data: workActivityData, encoding: .utf8))
        let activityArguments = ActivityArguments()
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "desktop" where args.dropFirst().first == "plan": return roadmapJSON
            case "planning": return #"{"destinations":[]}"#
            case "wave" where args.dropFirst().first == "list": return wavesJSON
            case "desktop" where args.dropFirst().first == "sessions": return #"{"entries":[],"next":null}"#
            case "history":
                await activityArguments.record(args)
                return workActivityJSON
            default: throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
            }
        }
        return WorkTestFixture(
            roadmap: roadmap,
            waves: waves,
            workActivity: workActivity,
            workActivityData: workActivityData,
            roadmapJSON: roadmapJSON,
            wavesJSON: wavesJSON,
            workActivityJSON: workActivityJSON,
            activityArguments: activityArguments,
            query: query
        )
    }

    func streaming(_ feed: WorkFeed) -> RegistryQuery {
        RegistryQuery(watchWork: { await feed.open() }) { args, _ in
            throw RegistryQueryError("a streamed window ran lf \(args.joined(separator: " "))")
        }
    }

    static let scope = WorkScope(repo: "/src/loopflow", headless: false, task: nil, activity: nil)

    /// One line of `part`'s watch, as the transport hands it to the window.
    func frame(
        _ part: String, sequence: Int, answers: Int?, scope: WorkScope = Self.scope, body: String
    ) throws -> WorkFrame {
        if part == "checkpoint" {
            return WorkFrame(sequence: sequence, answers: answers, unavailable: nil, content: .checkpoint, wire: nil)
        }
        let answers = answers.map(String.init) ?? "null"
        let line = #"{"answers":\#(answers),"unavailable":null,"body":\#(body)}"#
        return try WorkFrame.decode(part: try #require(WorkPart(rawValue: part)), scope: scope,
                                    sequence: sequence, line: Data(line.utf8))
    }

    /// `completing` settles that Task successfully a minute ago, with no runtime left.
    func planningFrame(
        sequence: Int, answers: Int?, renaming name: String? = nil, completing task: String? = nil
    ) throws -> WorkFrame {
        var roadmap = roadmapJSON
        if let name {
            roadmap = roadmap.replacingOccurrences(of: "Make lf roadmap the machine-wide view", with: name)
        }
        if let task {
            var root = try #require(JSONSerialization.jsonObject(with: Data(roadmap.utf8)) as? [String: Any])
            var evidence = try #require(root["tasks"] as? [String: Any])
            var rows = try #require(evidence["items"] as? [[String: Any]])
            for row in rows.indices {
                guard var plan = rows[row]["task"] as? [String: Any], plan["id"] as? String == task else { continue }
                plan["state"] = "completed"
                plan["completed"] = true
                plan["completed_at"] = Date().addingTimeInterval(-60).formatted(.iso8601)
                rows[row]["task"] = plan
                rows[row]["runtime"] = NSNull()
                if var condition = rows[row]["condition"] as? [String: Any] {
                    condition["unresolved_execution"] = false
                    rows[row]["condition"] = condition
                }
            }
            evidence["items"] = rows
            root["tasks"] = evidence
            roadmap = try #require(String(data: JSONSerialization.data(withJSONObject: root), encoding: .utf8))
        }
        return try frame("planning", sequence: sequence, answers: answers, body: roadmap)
    }

    func sessionsFrame(sequence: Int, answers: Int?, repo: String, entries: String) throws -> WorkFrame {
        try frame("sessions", sequence: sequence, answers: answers,
                  scope: WorkScope(repo: repo, headless: false, task: nil, activity: nil), body: entries)
    }

    /// A live-process reading: a frame that confirms and changes no Work.
    func activityFrame(sequence: Int) throws -> WorkFrame {
        try frame("activity", sequence: sequence, answers: nil,
                  body: #"{"nodes":[]}"#)
    }

    func sessionEntries(sourceFile: String = #filePath) throws -> (json: String, ids: [String]) {
        let page = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session_page.json")
        let object = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: page)) as? [String: Any])
        let entries = try #require(object["entries"] as? [[String: Any]])
        let json = try #require(String(data: JSONSerialization.data(withJSONObject: entries), encoding: .utf8))
        return (json, try entries.map { try #require($0["id"] as? String) })
    }

    func workActivityJSON(replacingFirstSubjectWith subject: String) throws -> String {
        var object = try #require(
            JSONSerialization.jsonObject(with: workActivityData) as? [String: Any]
        )
        var items = try #require(object["items"] as? [[String: Any]])
        items[0]["subject"] = subject
        object["items"] = items
        let data = try JSONSerialization.data(withJSONObject: object)
        return try #require(String(data: data, encoding: .utf8))
    }
}

private actor ActivityArguments {
    private(set) var last: [String] = []

    func record(_ args: [String]) {
        last = args
    }
}

private actor DeferredActivityResponse {
    private var responseContinuation: CheckedContinuation<String, Never>?
    private var requestContinuation: CheckedContinuation<Void, Never>?

    func response() async -> String {
        await withCheckedContinuation { continuation in
            responseContinuation = continuation
            requestContinuation?.resume()
            requestContinuation = nil
        }
    }

    func waitUntilRequested() async {
        if responseContinuation != nil { return }
        await withCheckedContinuation { continuation in
            requestContinuation = continuation
        }
    }

    func release(_ response: String) {
        responseContinuation?.resume(returning: response)
        responseContinuation = nil
    }
}
#endif
