#if os(macOS)
import Foundation
import SwiftUI
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

    @Test("Every reference to a Wave's Project, ended or not, navigates to its Wave")
    func projectReferencesSelectWave() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)
        var wire = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
        var waves = try #require(wire["waves"] as? [[String: Any]])
        var project = try #require((waves[0]["project"] as? [String: Any])?["value"] as? [String: Any])
        project["status"] = "completed"
        waves[0]["project"] = ["state": "ok", "value": project]
        wire["waves"] = waves
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(roadmap), waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity), repos: [])
        let wave = try #require(roadmap.waves.first)
        let read = try #require(wave.project.present)
        for reference in [read.id, read.slug, read.trackerId].compactMap({ $0 }) {
            model.select(.project(id: reference))
            #expect(model.selection == .wave(id: wave.wave.id))
        }
    }

    @Test("Selected Task survives a planning update that renames it")
    func selectedTaskSurvivesPlanningUpdate() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)
        await model.refresh()
        model.select(.task(id: "issue-now"))
        var wire = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
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
        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.task(id: "issue-now")?.task.task.identifier == "W2-144")
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
        historical["project"] = ["state": "ok", "value": [
            "id": "old-plan", "tracker_id": NSNull(), "url": NSNull(), "unbound": NSNull(), "slug": "old", "name": "Previous",
            "status": "completed", "sync": ["actions": []]
        ]]
        let reply = try JSONSerialization.data(withJSONObject: historical)
        await deferred.release(try #require(String(data: reply, encoding: .utf8)))
        await lookup.value
        switch destination {
        case "history":
            #expect(model.selection == .wave(id: "wave-1"))
        case "task":
            #expect(model.selection == .task(id: "issue-now"))
        default:
            #expect(model.repoPath == "/src/context")
            #expect(model.selection == nil)
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
        #expect(model.roadmap.failure == "registry unavailable")
        #expect(model.waves.value == fixture.waves)
        #expect(model.waves.failure == "registry unavailable")
        #expect(model.workActivity.value == fixture.workActivity)
        #expect(model.workActivity.failure == "registry unavailable")
        #expect(model.visibleWaves.count == 2)
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
            "session", "history", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.project(id: "project-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "session", "history", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.task(id: "issue-now"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "session", "history", "--since", "7d", "--limit", "50",
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
            guard args.prefix(2) == ["session", "history"] else {
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
            #expect(args == ["session", "list", "--json", "--page", "--limit", "100"])
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
            #expect(args == ["session", "list", "--json", "--page", "--limit", "100"])
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
            #expect(args == ["session", "list", "--json", "--page", "--limit", "100"])
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

    @Test("The sidebar lists nothing before a started list is read, and launches from the saved one")
    func sidebarStartsFromTheSavedList() async throws {
        let fixture = try WorkTestFixture.load()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = WorkCache(directory: directory)
        cache.saveSelection(nil, repo: "/src/loopflow")
        cache.confirmMachine("machine-a")
        cache.flush()
        let feed = WorkFeed()
        let first = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow", cache: cache)
        first.confirmMachine("machine-a")
        let keeping = Task { await first.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        try await eventually { first.roadmap.value != nil }
        // Planning rows are not a second list: only the started list fills the sidebar.
        #expect(first.sidebarTaskIDs.isEmpty)
        var row = fixtureViewContent(work: .init(repository: "repo", kind: .task, id: "issue-now"), title: "Now")
        row.states = [.open, .working]
        let plan = fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [row])
        let body = String(decoding: try JSONEncoder().encode(plan), as: UTF8.self)
        await feed.send(try fixture.frame("task_list", sequence: 2, answers: scope.id, body: body))
        try await eventually { first.sidebarTaskIDs == ["issue-now"] }
        let saved = { (try? FileManager.default.contentsOfDirectory(atPath: directory.path))?
            .contains { $0.hasPrefix("readings-machine-a-task_list") } == true }
        try await eventually { saved() }
        // The next launch draws the list before anything is read.
        let next = WorkModel(query: fixture.streaming(WorkFeed()), repoPath: "/src/loopflow", cache: cache)
        #expect(next.sidebarTaskIDs == ["issue-now"])
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
        #expect(model.selection == key.work) // Selection restores before fresh execution evidence.
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
        try await eventually { model.roadmap.failure?.contains("t.paused") == true }
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
            try await eventually { model.roadmap.failure?.contains(truncated ? "incomplete" : "Task snapshot") == true }
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
        #expect(model.roadmap.failure == nil)
        #expect(!model.showsSavedPlanning)
    }

    @Test("Task Workflow survives a planning failure, which the Notice reports, and its reader's disconnect")
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
                              activity: nil, taskListWave: nil)
        await feed.send(try fixture.frame("task", sequence: 2, answers: scope.id, scope: shown,
            body: "{\"overview\":\(work),\"comments\":{\"identifier\":\"\(found.task.task.identifier)\",\"comments\":[],\"conflicts\":{},\"pending_sync\":[],\"refresh_error\":null},\"graph_processes\":[],\"execution\":null}"))
        try await eventually { model.taskWork[found.task.id].value?.graph != nil }
        let workflow = model.taskWork[found.task.id].value?.graph
        await feed.send(WorkFrame(sequence: 3, answers: scope.id,
            unavailable: "sqlite: no such column: t.paused", content: .planning(nil), wire: nil))
        try await eventually { model.roadmap.failure != nil }
        #expect(model.taskWork[found.task.id].value?.graph == workflow)
        let bar = TaskWorkflowBar(model: model, task: found.task, work: model.taskWork[found.task.id])
        #expect(throws: (any Error).self) { try bar.inspect().find(text: "No workflow yet") }
        // Changes still show at once: a failed read is no reason to refuse one.
        #expect(try !bar.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-move").isDisabled())
        #expect(model.navigation.notices.current?.text == "The plan couldn't be read: sqlite: no such column: t.paused")
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.taskWork[found.task.id].failure == "reader exited" }
        #expect(model.taskWork[found.task.id].value?.graph == workflow)
    }

    @Test("The plan's home updates independently, preserves failures and rejects old scopes")
    func planHomeFrames() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        let onHub = #"{"tracker":null,"plan":{"git":null,"hub":{"machine":"mini","unsent":1,"records":["3e1c9a07"],"error":null,"failing_since":null,"not_taken":null}},"actions":[]}"#
        let here = #"{"tracker":null,"plan":{"git":null,"hub":null},"actions":[]}"#
        func planFrame(_ sequence: Int, _ answers: Int, repo: String, body: String) throws -> WorkFrame {
            try fixture.frame("plan_home", sequence: sequence, answers: answers,
                              scope: WorkScope(repo: repo, headless: false, task: nil, activity: nil, taskListWave: nil),
                              body: body)
        }
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        await feed.send(try planFrame(2, scope.id, repo: "/src/loopflow", body: onHub))
        try await eventually { model.repositoryPlan.value?.plan?.hub?.line == "Hub mini, 1 unsent." }
        #expect(model.roadmap.value != nil)

        await feed.send(WorkFrame(sequence: 3, answers: scope.id,
                                 unavailable: "sync read failed", content: .planHome(nil), wire: nil))
        try await eventually { model.repositoryPlan.failure == "sync read failed" }
        #expect(model.repositoryPlan.value?.plan?.hub?.standing.unsent == 1)
        #expect(model.roadmap.failure == nil)
        await feed.send(try planFrame(4, scope.id, repo: "/src/loopflow", body: onHub))
        try await eventually { model.repositoryPlan.failure == nil }

        model.setRepoPath("/src/context")
        model.syncWorkScope()
        let changed = try await feed.request(2)
        // Even a later sequence cannot publish an answer from the old scope.
        await feed.send(try planFrame(5, scope.id, repo: "/src/loopflow", body: onHub))
        await feed.send(try planFrame(6, changed.id, repo: "/src/loopflow", body: onHub))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.repositoryPlan.value == nil)
        await feed.send(try planFrame(7, changed.id, repo: "/src/context", body: here))
        try await eventually { model.repositoryPlan.value?.line == "Plan: this Machine." }
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.repositoryPlan.failure == "reader exited" }
        #expect(model.repositoryPlan.value?.line == "Plan: this Machine.")
        try await feed.opened(2, within: .seconds(5))
        let reopened = try await feed.request(3)
        await feed.send(try planFrame(1, reopened.id, repo: "/src/context", body: onHub))
        try await eventually {
            model.repositoryPlan.value?.plan?.hub?.machine == "mini" && model.repositoryPlan.failure == nil
        }
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
        await feed.send(try fixture.unrelatedFrame(sequence: 4))
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

    @Test("A Task completed elsewhere leaves the working set and stays under Completed")
    func completedTaskLeavesWorkingSet() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        let listed = { (filter: TaskHistoryFilter) -> Bool in
            guard let row = model.task(id: "issue-now")?.task else { return false }
            return filter.includes(row, now: Date())
        }
        var completed = TaskHistoryFilter()
        completed.showCompleted = true

        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        model.select(.task(id: "issue-now"))
        #expect(listed(TaskHistoryFilter()))

        await feed.send(try fixture.planningFrame(sequence: 2, answers: nil, completing: "issue-now"))
        try await eventually { model.task(id: "issue-now")?.task.isCompleted == true }
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
        let query = RegistryQuery(subscribeWork: { await feed.open() }) { args, _ in
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
        try await eventually { model.graphCatalog.value?.isEmpty == false }
        let baseline = await catalog.reads
        await catalog.update("[]")
        await feed.send(try fixture.unrelatedFrame(sequence: 2))
        try await Task.sleep(for: .milliseconds(50))
        #expect(await catalog.reads == baseline)
        #expect(model.graphCatalog.value?.isEmpty == false)
        await feed.send(try fixture.planningFrame(sequence: 3, answers: nil))
        try await eventually { model.graphCatalog.value?.isEmpty == true }
        #expect(await catalog.reads == baseline + 1)
        // Both catalogues are one read: returning from an editor rereads it once.
        await model.rereadDefinitions()
        #expect(await catalog.reads == baseline + 2)
        #expect(model.graphCatalog.value?.isEmpty == true)
        await catalog.fail()
        await feed.send(try fixture.planningFrame(sequence: 4, answers: nil))
        try await eventually { model.graphCatalog.failure == "catalog unavailable" }
        #expect(model.graphCatalog.value?.isEmpty == true)
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
        #expect(model.graphCatalog.value?.isEmpty == true)
        #expect(model.graphCatalog.failure == nil)
    }

    @Test("A change shows in the click, then asks its parts for a reading once answered")
    func changeCheckpointsAfterItsAnswer() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let answered = AsyncStream<Void>.makeStream()
        let query = RegistryQuery(subscribeWork: { await feed.open() }) { args, _ in
            guard args.starts(with: ["task", "unassign"]) else { throw RegistryQueryError("unexpected read: \(args)") }
            for await _ in answered.stream { break }
            return ""
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let found = try #require(model.task(id: "issue-now"))
        model.assignTask(found.task, claimed: false)
        // Held: the change is shown and nothing asked for a reading yet.
        #expect(model.pendingChanges.changes.map(\.command) == [.unassign(task: found.task.task.identifier)])
        answered.continuation.finish()
        let request = try await feed.request(2)
        #expect(request == .checkpoint(id: request.id, replay: []))
        // The CLI answers without revisions: the preview holds through the reread it asked for.
        #expect(!model.pendingChanges.changes.isEmpty)
        await feed.send(try fixture.frame("checkpoint", sequence: 2, answers: request.id, body: "null"))
        try await eventually { model.pendingChanges.changes.isEmpty }
    }

    @Test("A held Claim shows the owner the repository reading names, before the reply")
    func claimShowsItsOwnerInTheClick() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let query = RegistryQuery(subscribeWork: { await feed.open() }) { args, _ in
            guard args.starts(with: ["task", "claim"]) else { throw RegistryQueryError("unexpected read: \(args)") }
            try await Task.sleep(for: .seconds(60))
            return ""
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id))
        var plan = try #require(JSONSerialization.jsonObject(with: loadFixtureData("view_plan.json")) as? [String: Any])
        plan["repo_path"] = "/src/loopflow"
        let body = String(decoding: try JSONSerialization.data(withJSONObject: plan), as: UTF8.self)
        await feed.send(try fixture.frame("task_list", sequence: 2, answers: scope.id, body: body))
        try await eventually { model.taskListReading.value != nil }
        let found = try #require(model.task(id: "issue-now"))
        #expect(found.task.runtime?.owner == nil)
        model.assignTask(found.task, claimed: true)
        let claimed = try #require(model.visibleTasks.first { $0.id == "issue-now" })
        #expect(claimed.runtime?.owner?.person == "Ada")
        #expect(claimed.runtime?.owner?.machine.label == "laptop")
    }

    @Test("A refused Complete returns the row; its Notice's Show selects the Task")
    func refusalShowsTheTask() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let query = RegistryQuery(subscribeWork: { await feed.open() }) { args, _ in
            guard args.starts(with: ["task", "complete"]) else { throw RegistryQueryError("unexpected read: \(args)") }
            throw RegistryQueryError("its PR #12 is open")
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        let found = try #require(model.task(id: "issue-now"))
        model.completeTask(found.task, children: nil)
        try await eventually { model.pendingChanges.changes.isEmpty }
        let notice = try #require(model.navigation.notices.current)
        #expect(notice.text.hasPrefix("\(found.task.task.identifier) couldn't be completed"))
        #expect(notice.action?.title == "Show")
        model.select(nil)
        notice.action?.perform()
        #expect(model.selection == .task(id: "issue-now"))
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
        await feed.send(try fixture.unrelatedFrame(sequence: 3))
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

    @Test("Successful frames before repeated failures retain backoff, and cancellation stops retries")
    func observationRetryBackoff() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1, within: .seconds(5))
        for attempt in 1...3 {
            await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
            try await eventually { model.roadmap.value != nil && model.roadmap.failure == nil }
            let failed = ContinuousClock.now
            await feed.fail(RegistryQueryError("part ended"))
            try await eventually { model.roadmap.failure == "part ended" }
            #expect(model.task(id: "issue-now") != nil)
            if attempt < 3 {
                try await feed.opened(attempt + 1, within: .seconds(5))
                #expect(ContinuousClock.now - failed >= .seconds(attempt))
            }
        }
        let cancelled = ContinuousClock.now
        keeping.cancel()
        await keeping.value
        #expect(ContinuousClock.now - cancelled < .seconds(1))
        #expect(await feed.openCount == 3)
    }

    @Test("New turns replace the Session excerpt and an ended reader recovers without reviving old output")
    func readerEndRecovers() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
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
            if model.sessions.failure != nil { return "failed" }
            return model.sessions.value?.first?.preview?.text ?? "none"
        }

        try await feed.opened(1)
        answers = try await feed.request(1).id
        await feed.send(try fixture.planningFrame(sequence: 1, answers: answers))
        await feed.send(try sessionFrame(2, turn: "Previous result", available: true))
        try await eventually { preview() == "Previous result" }
        await feed.send(try sessionFrame(3, turn: nil))
        try await eventually { preview() == "none" }
        await feed.send(try sessionFrame(4, turn: "New turn output"))
        try await eventually { preview() == "New turn output" }
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.sessions.failure == "reader exited" }
        #expect(model.sessions.value?.first?.preview?.text == "New turn output")
        #expect(preview() == "failed")
        #expect(model.roadmap.value != nil)
        #expect(!model.roadmap.isLoading)

        try await feed.opened(2, within: .seconds(5))
        answers = try await feed.request(2).id
        // Planning recovery alone cannot revive the old Session reading.
        await feed.send(try fixture.planningFrame(sequence: 1, answers: answers, renaming: "After restart"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "After restart" }
        #expect(preview() == "failed")
        await feed.send(try sessionFrame(2, turn: nil))
        try await eventually { model.sessions.failure == nil }
        #expect(preview() == "none")
        await feed.send(try sessionFrame(3, turn: "Recovered result", available: true))
        try await eventually { preview() == "Recovered result" }
    }

}

@MainActor
func eventually(_ condition: @MainActor () -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(3)
    while !condition() {
        guard ContinuousClock.now < deadline else {
            Issue.record("condition did not hold in time")
            throw CancellationError()
        }
        try await Task.sleep(for: .milliseconds(5))
    }
}

private actor Flag {
    private(set) var value = false
    func set() { value = true }
}

/// A scripted reader: the test decides which frames arrive and when.
actor WorkFeed {
    private var continuations: [AsyncThrowingStream<WorkFrame, any Error>.Continuation] = []
    private var requests: [WorkRequest] = []
    var openCount: Int { continuations.count }

    nonisolated func open() async -> WorkObservation {
        let (stream, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        await add(continuation)
        return WorkObservation(frames: stream, request: { request in
            Task { await self.record(request) }
        }, cancel: { continuation.finish() })
    }

    private func add(_ continuation: AsyncThrowingStream<WorkFrame, any Error>.Continuation) {
        continuations.append(continuation)
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
            case "plan" where args.dropFirst().first == "list": return roadmapJSON
            case "repo" where args.dropFirst().first == "tracker": return #"{"tracker":null,"plan":{"git":null,"hub":null},"actions":[]}"#
            case "desktop" where args.dropFirst().prefix(2) == ["view", "show"]:
                return String(decoding: try JSONEncoder().encode(fixtureViewPlan(
                    owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [])), as: UTF8.self)
            case "wave" where args.dropFirst().first == "list": return wavesJSON
            case "session" where args.dropFirst().first == "list": return #"{"entries":[],"next":null}"#
            case "session" where args.dropFirst().first == "history":
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
        RegistryQuery(subscribeWork: { await feed.open() }) { args, _ in
            throw RegistryQueryError("a streamed window ran lf \(args.joined(separator: " "))")
        }
    }

    static let scope = WorkScope(repo: "/src/loopflow", headless: false, task: nil, activity: nil, taskListWave: nil)

    /// One line of `part`'s subscription, as the transport hands it to the window.
    func frame(
        _ part: String, sequence: Int, answers: Int?, scope: WorkScope = Self.scope, body: String
    ) throws -> WorkFrame {
        if part == "checkpoint" {
            return WorkFrame(sequence: sequence, answers: answers, unavailable: nil, content: .checkpoint, wire: nil)
        }
        let answers = answers.map(String.init) ?? "null"
        let line = #"{"answers":\#(answers),"unavailable":null,"body":\#(body),"read_at":null}"#
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
                rows[row]["lifecycle"] = "done"
                rows[row]["section"] = "done"
                rows[row]["selection_states"] = ["done"]
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
                  scope: WorkScope(repo: repo, headless: false, task: nil, activity: nil, taskListWave: nil), body: entries)
    }

    /// A reading of where the plan travels: a frame that confirms and changes no Work.
    func unrelatedFrame(sequence: Int) throws -> WorkFrame {
        try frame("plan_home", sequence: sequence, answers: nil,
                  body: #"{"actions":[],"plan":{"git":null,"hub":null},"tracker":null}"#)
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
@Suite("Task planning readability", .serialized)
@MainActor
struct TaskPlanningReadabilityTests {
    private let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task-opaque")

    private func plan(title: String = "Readable local Task") -> WorkViewPlan {
        let content = WorkViewPlan.Content(work: key, title: title,
            states: [.working], identifier: "LOO-905", workingGroup: .active, graphStep: nil, primarySession: nil, location: nil, argv: nil, unavailable: [], openChildren: nil)
        return fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [content])
    }

    private func savedRoadmap(_ fixture: WorkTestFixture) throws -> String {
        var root = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
        var tasks = try #require(root["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        items[0]["section"] = "working"
        var other = items[0]
        var task = try #require(other["task"] as? [String: Any])
        task["id"] = "context-task"
        task["identifier"] = "CTX-1"
        task["name"] = "Readable context Task"
        task["repo"] = "/src/context"
        task["repo_id"] = "context-repo"
        task["wave_id"] = NSNull()
        task["project_id"] = NSNull()
        other["task"] = task
        other["runtime"] = NSNull()
        tasks["items"] = items + [other]
        root["tasks"] = tasks
        return String(decoding: try JSONSerialization.data(withJSONObject: root), as: UTF8.self)
    }

    @Test("Name freshness follows its source while other reads fail", arguments: [false, true])
    func independentNameFreshness(roadmapFails: Bool) throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query, repoPath: "/src/loopflow")
        let record = try #require(fixture.roadmap.tasks.items.first { $0.id == "issue-now" })
        let work = WorkWorkspaceKey(repository: "repo", kind: .task, id: record.runtime?.workId ?? record.id)
        let member = WorkViewPlan.Content(work: work, title: "Fresh list name",
            states: [.working], identifier: record.task.identifier, workingGroup: .active, graphStep: nil, primarySession: nil, location: nil, argv: nil, unavailable: [], openChildren: nil)
        let list = fixtureViewPlan(owner: key, members: [member])
        func planning(_ failed: Bool) {
            model.applyFixture(roadmap: failed ? .unavailable(lastGood: fixture.roadmap, reason: "Planning offline") : .available(fixture.roadmap),
                waves: .available(fixture.waves), workActivity: .available(fixture.workActivity), repos: [], fixed: true)
        }
        planning(roadmapFails)
        model.applyTaskList(list, wave: nil, reason: "")
        if !roadmapFails { model.applyTaskList(nil, wave: nil, reason: "List offline") }
        let expected = roadmapFails ? "Fresh list name" : record.task.name
        #expect(model.taskPresentation(id: record.id).title == expected)
        #expect(model.taskPresentation(id: record.id).context == nil)
        let row = WorkTaskIdentity(presentation: model.taskPresentation(id: record.id))
        #expect(try row.inspect().find(text: expected).string() == expected)
        #expect(throws: (any Error).self) { try row.inspect().find(text: "Last known") }
        #expect(model.workStatus.message?.contains(roadmapFails ? "Planning offline" : "List offline") == true)
        // Replacing the healthy source cannot clear the other source's failure.
        if roadmapFails { model.applyTaskList(list, wave: nil, reason: "") } else { planning(false) }
        #expect(model.workStatus.message?.contains(roadmapFails ? "Planning offline" : "List offline") == true)
        if roadmapFails { planning(false) } else { model.applyTaskList(list, wave: nil, reason: "") }
        #expect(model.workStatus.message == nil)
    }

    @Test("A fresh Task subscription outranks missing or retained roadmap identity", arguments: [false, true], [false, true])
    func currentSubscriptionName(retainedRoadmap: Bool, roadmapFailsFirst: Bool) async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let record = try #require(fixture.roadmap.tasks.items.first { $0.id == "issue-now" })
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
        let work = try JSONDecoder().decode(TaskOverview.self, from: Data(contentsOf: fixtures.appendingPathComponent("task_work.json")))
        let comments = try JSONDecoder().decode(TaskComments.self, from: Data(#"{"identifier":"W2-144","comments":[],"conflicts":{},"pending_sync":[],"refresh_error":null}"#.utf8))
        let body = WorkFrame.TaskPart(task: record.task.identifier, row: record, work: work,
                                      graphProcesses: [], comments: comments, execution: nil)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        let ids = [record.id, record.runtime?.workId].compactMap { $0 }
        func shows(_ title: String, _ context: String?) -> Bool {
            ids.allSatisfy { model.taskPresentation(id: $0).title == title && model.taskPresentation(id: $0).context == context }
        }
        if retainedRoadmap {
            await feed.send(try fixture.planningFrame(sequence: 1, answers: scope.id, renaming: "Older roadmap name"))
            try await eventually { shows("Older roadmap name", nil) }
        }
        func failRoadmap() async throws {
            await feed.send(WorkFrame(sequence: 2, answers: scope.id, unavailable: "Planning offline",
                                      content: .planning(nil), wire: nil))
            try await eventually { model.roadmap.failure == "Planning offline" }
        }
        func publishTask(_ sequence: Int) async {
            await feed.send(WorkFrame(sequence: sequence, answers: scope.id, unavailable: nil,
                                      content: .task(body), wire: nil))
        }
        if roadmapFailsFirst { try await failRoadmap() }
        await publishTask(1)
        if !roadmapFailsFirst { try await failRoadmap() }
        try await eventually { shows(record.task.name, nil) }
        let row = WorkTaskIdentity(presentation: model.taskPresentation(id: record.id))
        #expect(try row.inspect().find(text: record.task.name).string() == record.task.name)
        #expect(throws: (any Error).self) { try row.inspect().find(text: "Last known") }
        #expect(model.workStatus.message?.contains("Planning offline") == true)
        // Both readings failed: identity stays readable and says so.
        await feed.send(WorkFrame(sequence: 2, answers: scope.id, unavailable: "Task offline",
                                  content: .task(nil), wire: nil))
        try await eventually { model.taskPresentation(id: record.id).context == "Last known" }
        #expect(shows(retainedRoadmap ? "Older roadmap name" : record.task.name, "Last known"))
        // An identical row restores current identity; a healthy roadmap then supplies the name.
        await publishTask(3)
        try await eventually { shows(record.task.name, nil) }
        await feed.send(try fixture.planningFrame(sequence: 3, answers: scope.id, renaming: "Recovered roadmap name"))
        try await eventually { shows("Recovered roadmap name", nil) }
        #expect(model.workStatus.message?.contains("Planning offline") != true)
        keeping.cancel()
        await keeping.value
    }

    @Test("A current selection name precedes failed retained list content")
    func currentSelectionName() throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query, repoPath: "/src/loopflow")
        model.applyTaskList(plan(title: "Old list name"), wave: nil, reason: "")
        model.applyTaskList(nil, wave: nil, reason: "List offline")
        model.applySelectionPlans([plan(title: "Current selection name")], reason: "")
        #expect(model.taskPresentation(id: key.id).title == "Current selection name")
        #expect(model.taskPresentation(id: key.id).context == nil)
        model.applySelectionPlans(nil, reason: "Selection offline")
        #expect(model.taskPresentation(id: key.id).title == "Old list name")
        #expect(model.taskPresentation(id: key.id).context == "Last known")
        model.applySelectionPlans([plan(title: "Recovered selection name")], reason: "")
        #expect(model.taskPresentation(id: key.id).title == "Recovered selection name")
        #expect(model.taskPresentation(id: key.id).context == nil)
        #expect(model.taskListReading.failure == "List offline")
        #expect(model.selectionPlans.failure == nil)
    }

    @Test("Loaded empty descriptions omit the preview, also while updating", arguments: [false, true])
    func emptyDescription(saved: Bool) throws {
        let fixture = try WorkTestFixture.load()
        var root = try #require(JSONSerialization.jsonObject(with: Data(fixture.roadmapJSON.utf8)) as? [String: Any])
        var tasks = try #require(root["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        var task = try #require(items[0]["task"] as? [String: Any])
        task["description"] = ""
        items[0]["task"] = task
        items[0]["section"] = "working"
        tasks["items"] = items
        root["tasks"] = tasks
        let wire = try JSONSerialization.data(withJSONObject: root)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = WorkCache(directory: directory)
        cache.saveRoadmap(String(decoding: wire, as: UTF8.self))
        cache.flush()
        let model = WorkModel(query: fixture.query, repoPath: "/src/loopflow", cache: saved ? cache : nil)
        if !saved {
            model.applyFixture(roadmap: .available(try JSONDecoder().decode(RoadmapSnapshot.self, from: wire)),
                waves: .available(fixture.waves), workActivity: .available(fixture.workActivity), repos: [], fixed: true)
        }
        let record = try #require(model.task(id: "issue-now")?.task)
        let member = WorkViewPlan.Content(
            work: .init(repository: "repo", kind: .task, id: record.runtime?.workId ?? record.id),
            title: record.task.name, states: [.working], identifier: record.task.identifier,
            workingGroup: .active, graphStep: nil, primarySession: nil, location: nil, argv: nil, unavailable: [], openChildren: nil)
        model.applyTaskList(fixtureViewPlan(owner: key, members: [member]), wave: nil, reason: "")
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        #expect(try navigator.inspect().find(text: record.task.name).string() == record.task.name)
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: "Loading planning…") }
        if saved {
            #expect(try navigator.inspect().find(text: "Updating…").string() == "Updating…")
        }
    }

    @Test("A named list arrives before planning; failure and replacement retain readable identity")
    func listBeforePlanning() throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No side effects") }, repoPath: "/src/loopflow")
        #expect(model.taskPresentation(id: key.id).title == "Loading Task…")
        model.applyTaskList(plan(), wave: nil, reason: "")
        #expect(model.sidebarTaskIDs == [key.id])
        #expect(model.workspaceTitle(key) == "Readable local Task")
        // `lf desktop ls` names rows as the sidebar does, not by Work ID.
        let listed = model.desktopList(repository: "repo")
        #expect(listed.map(\.title) == ["Readable local Task"])
        #expect(listed.map(\.identifier) == ["LOO-905"])
        #expect(listed.map(\.work) == [key])
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        #expect(try navigator.inspect().find(text: "Readable local Task").string() == "Readable local Task")
        #expect(try navigator.inspect().find(text: "Loading planning…").string() == "Loading planning…")
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: "Planning unavailable") }
        model.select(.task(id: key.id))
        #expect(model.selection == key.reference)
        #expect(try TitleBar(model: model).inspect().find(text: "Readable local Task").string() == "Readable local Task")
        let row = WorkTaskIdentity(presentation: model.taskPresentation(id: key.id))
        #expect(try row.inspect().find(text: "Readable local Task").string() == "Readable local Task")
        #expect(try row.inspect().find(text: "LOO-905").string() == "LOO-905")
        model.applyTaskList(nil, wave: nil, reason: "List offline")
        #expect(model.taskPresentation(id: key.id).context == "Last known")
        #expect(model.workStatus.message == "Couldn't update: List offline")
        #expect(model.sidebarTaskIDs == [key.id])
        model.applyTaskList(plan(title: "Renamed locally"), wave: nil, reason: "")
        #expect(model.workspaceTitle(key) == "Renamed locally")
        #expect(model.taskPresentation(id: key.id).context == nil)
        #expect(model.workStatus == .loading, "planning and Sessions have not answered")
        model.applyTaskList(fixtureViewPlan(owner: key, members: []), wave: nil, reason: "")
        #expect(model.sidebarTaskIDs.isEmpty, "a complete removal cannot revive old membership")
    }

    @Test("Missing-facts titles stay private; Wave, repository and Machine readings do not leak")
    func unknownAndScopeChanges() {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No side effects") }, repoPath: "/src/loopflow")
        model.confirmMachine("one")
        model.applyTaskList(plan(title: key.id), wave: nil, reason: "")
        #expect(model.workspaceTitle(key) == "Loading Task…")
        model.applyTaskList(nil, wave: nil, reason: "Unavailable")
        #expect(model.workspaceTitle(key) == "Task unavailable")
        model.applyTaskList(plan(), wave: nil, reason: "")
        model.navigation.sidebarWaveID = "other-wave"
        #expect(model.taskListReading.isLoading)
        #expect(model.taskPresentation(id: key.id).title == "Loading Task…")
        model.navigation.sidebarWaveID = nil
        model.setRepoPath("/src/other")
        #expect(model.taskPresentation(id: key.id).title == "Loading Task…")
        model.setRepoPath("/src/loopflow")
        #expect(model.workspaceTitle(key) == "Readable local Task")
        model.confirmMachine("two")
        #expect(model.taskPresentation(id: key.id).title == "Loading Task…")
    }

    @Test("Session status and keyboard hints do not require a full Task record")
    func partialSessionMetadata() throws {
        let fixture = try WorkTestFixture.load()
        let entries = try fixture.sessionEntries()
        let session = try #require(RegistryQuery.decode([SessionRecord].self, from: entries.json).first)
        let presentation = WorkModel.TaskPresentation(title: "Loading Task…", identifier: nil, context: nil)
        let metadata = TaskRow(id: key.id, presentation: presentation, task: nil, status: .available(session))
        #expect(try metadata.inspect().find(text: "Available · inferred").string() == "Available · inferred")
        #expect(throws: (any Error).self) { try metadata.inspect().find(text: "Unowned") }
        let shortcut = TaskRow(id: key.id, presentation: presentation, task: nil, status: .unknown, slotHint: "⌘1")
        #expect(try shortcut.inspect().find(text: "⌘1").string() == "⌘1")
    }

    @Test("The sidebar claims no empty list until one is read; a failed read is the Notice's")
    func emptyListStates() throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No side effects") }, repoPath: "/src/loopflow")
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        // A pending list makes no empty claim; the one work status says it is loading.
        #expect(try navigator.inspect().find(text: "Loading work…").string() == "Loading work…")
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: "No working Tasks in this list.") }
        model.applyTaskList(nil, wave: nil, reason: "Offline")
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: "No working Tasks in this list.") }
        #expect(model.navigation.notices.current?.text == "Tasks couldn't be read: Offline")
        model.applyTaskList(fixtureViewPlan(owner: key, members: []), wave: nil, reason: "")
        #expect(try navigator.inspect().find(text: "No working Tasks in this list.").string() == "No working Tasks in this list.")
    }

    @Test("Saved planning rows never fill the sidebar; only the started list does")
    func savedIdentity() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = WorkCache(directory: directory)
        cache.saveRoadmap(try savedRoadmap(fixture))
        cache.flush()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow", cache: cache)
        #expect(model.workStatus == .updating)
        #expect(model.visibleTasks.contains { $0.section == .working })
        #expect(model.sidebarTaskIDs.isEmpty)
        let record = try #require(model.task(id: "issue-now")?.task)
        #expect(model.taskPresentation(id: record.id).title == record.task.name)
        #expect(model.taskPresentation(id: record.id).context == "Updating…")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(try fixture.frame("planning", sequence: 1, answers: scope.id, body: savedRoadmap(fixture)))
        try await eventually { !model.showsSavedPlanning }
        #expect(model.taskListReading.isLoading)
        #expect(model.sidebarTaskIDs.isEmpty, "planning is not a second list")
        let content = WorkViewPlan.Content(
            work: .init(repository: "repo", kind: .task, id: record.runtime?.workId ?? record.id),
            title: "Fresh local name", states: [.working], identifier: record.task.identifier,
            workingGroup: .active, graphStep: nil, primarySession: nil, location: nil, argv: nil, unavailable: [], openChildren: nil)
        model.applyTaskList(fixtureViewPlan(owner: key, members: [content]), wave: nil, reason: "")
        #expect(model.sidebarTaskIDs == [record.id])
        #expect(model.taskPresentation(id: record.id).title == record.task.name, "current roadmap identity keeps priority")
        model.applyTaskList(fixtureViewPlan(owner: key, members: []), wave: nil, reason: "")
        #expect(model.sidebarTaskIDs.isEmpty, "a fresh empty list supersedes saved membership")
        keeping.cancel()
        await keeping.value
    }

    @Test("Two repository windows restore their own names from one Machine cache")
    func cachedRepositoryWindows() throws {
        let fixture = try WorkTestFixture.load()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = WorkCache(directory: directory)
        cache.saveRoadmap(try savedRoadmap(fixture))
        cache.flush()
        let windows = ["/src/loopflow", "/src/context"].map {
            WorkModel(query: fixture.query, repoPath: $0, cache: cache)
        }
        for model in windows {
            let own = try #require(model.visibleTasks.first)
            model.select(.task(id: own.id))
            let crumbs = TitleBar(model: model)
            #expect(try crumbs.inspect().find(text: own.task.name).string() == own.task.name)
            #expect(try crumbs.inspect().find(text: "Updating…").string() == "Updating…")
            #expect(model.workStatus == .updating)
            // ⌘1–9 and `lf desktop ls` follow the sidebar's list: none is saved here.
            #expect(model.desktopList(repository: "repo").map(\.title) == model.sidebarTaskIDs.map { model.taskPresentation(id: $0).title })
            for other in windows where other !== model {
                for task in other.visibleTasks {
                    #expect(model.task(id: task.id) == nil)
                }
            }
        }
    }

    @Test("Independent list and Session arrival, disconnect and recovery keep the selected Task", arguments: [false, true])
    func arrivalAndReconnect(sessionsFirst: Bool) async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        func sessions(_ sequence: Int, _ answers: Int) throws -> WorkFrame {
            try fixture.sessionsFrame(sequence: sequence, answers: answers, repo: "/src/loopflow", entries: "[]")
        }
        if sessionsFirst { await feed.send(try sessions(1, scope.id)) }
        await feed.send(WorkFrame(sequence: 1, answers: scope.id, unavailable: nil, content: .taskList(plan()), wire: nil))
        try await eventually { model.sidebarTaskIDs == [key.id] }
        model.select(.task(id: key.id))
        let selection = model.selection
        if !sessionsFirst { await feed.send(try sessions(1, scope.id)) }
        #expect(model.workspaceTitle(key) == "Readable local Task")
        await feed.fail(RegistryQueryError("Disconnected"))
        try await eventually { model.taskListReading.failure == "Disconnected" }
        #expect(model.sidebarTaskIDs == [key.id])
        #expect(model.taskPresentation(id: key.id).context == "Last known")
        #expect(model.workStatus.message == "Couldn't update: Disconnected")
        #expect(model.selection == selection)
        try await feed.opened(2, within: .seconds(5))
        // No Task row has arrived to change the selected Task's scope.
        let reopened = try await feed.request(scope.id + 1)
        await feed.send(WorkFrame(sequence: 1, answers: reopened.id, unavailable: nil,
                                  content: .taskList(plan(title: "Recovered name")), wire: nil))
        await feed.send(try fixture.planningFrame(sequence: 1, answers: reopened.id))
        await feed.send(WorkFrame(sequence: 1, answers: reopened.id, unavailable: nil, content: .selections([]), wire: nil))
        await feed.send(try sessions(1, reopened.id))
        try await eventually { model.workStatus == .current }
        #expect(model.workspaceTitle(key) == "Recovered name")
        #expect(model.selection == selection)
        keeping.cancel()
        await keeping.value
    }

    @Test("A delayed one-shot Retry cannot overwrite a newer list or survive leaving its repository", arguments: [false, true])
    func lateRetry(replaced: Bool) async throws {
        let delayed = DeferredActivityResponse()
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["desktop", "view", "show"]) { return await delayed.response() }
            throw RegistryQueryError("Other reads unavailable")
        }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        let refreshing = Task { await model.refresh() }
        await delayed.waitUntilRequested()
        if replaced {
            model.applyTaskList(plan(title: "Newer list"), wave: nil, reason: "")
        } else {
            model.setRepoPath("/src/other")
            model.setRepoPath("/src/loopflow")
        }
        await delayed.release(String(decoding: try JSONEncoder().encode(plan(title: "Late list")), as: UTF8.self))
        await refreshing.value
        #expect(model.taskListReading.value?.contents.first?.title == (replaced ? "Newer list" : nil))
    }

    @Test("A failed list read posts one Notice keyed by its part, and a good read clears it")
    func listFailurePostsOneNotice() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await feed.opened(1)
        let scope = try await feed.request(1)
        await feed.send(WorkFrame(sequence: 1, answers: scope.id, unavailable: "List offline", content: .taskList(nil), wire: nil))
        try await eventually { model.taskListReading.failure != nil }
        let notices = model.navigation.notices
        #expect(notices.current?.text == "Tasks couldn't be read: List offline")
        #expect(model.workStatus.progress == nil)
        let posted = notices.posted
        await feed.send(WorkFrame(sequence: 2, answers: scope.id, unavailable: "List offline", content: .taskList(nil), wire: nil))
        await feed.send(WorkFrame(sequence: 3, answers: scope.id, unavailable: nil, content: .taskList(plan()), wire: nil))
        try await eventually { model.taskListReading.failure == nil }
        // Failing again posted nothing new; the good read put the Notice away.
        #expect(notices.posted == posted)
        #expect(notices.current == nil)
        keeping.cancel()
        await keeping.value
    }
}
#endif
