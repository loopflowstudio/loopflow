#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Work model")
@MainActor
struct WorkModelTests {
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
            repos: [PortfolioRepo(path: "/src/loopflow", lastOpened: .distantPast)]
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
        for reference in [chapter.id, chapter.slug, chapter.workId].compactMap({ $0 }) {
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
        var tasks = try #require(waves[0]["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        let index = try #require(items.firstIndex { ($0["task"] as? [String: Any])?["id"] as? String == "issue-now" })
        var planning = try #require(items[index]["task"] as? [String: Any])
        planning["name"] = "Updated while selected"
        items[index]["task"] = planning
        tasks["items"] = items
        waves[0]["tasks"] = tasks
        wire["waves"] = waves
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
        waves[0]["tasks"] = ["state": "ok", "items": [], "truncated": false]
        wire["waves"] = waves
        let transferring = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(transferring), waves: .available(fixture.waves),
            workActivity: .available(fixture.workActivity), repos: [])
        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.task(id: "issue-now")?.task.task.identifier == "W2-144")
        #expect(model.task(id: "issue-now")?.task.task.name == "Updated while selected")
        #expect(model.task(id: "issue-now")?.wave.projects.items.last?.id == "next")
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
            "id": "old-plan", "work_id": NSNull(), "slug": "old", "name": "Previous",
            "workflow": "feature", "status": "completed", "current": false, "metric_targets": [], "krs": []
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
                .appendingPathComponent("wave", isDirectory: true)
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
        await model.refreshPortfolio(
            initialRepoPath: nil,
            persistedRepos: [PortfolioRepo(path: repo.path, lastOpened: .distantPast)]
        )
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

    @Test("A development worktree becomes one main-repository choice")
    func developmentWorktreeBecomesMainRepositoryChoice() async throws {
        let fixture = try WorkTestFixture.load()
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-worktree-\(UUID().uuidString)", isDirectory: true)
        let origin = root.appendingPathComponent("repo", isDirectory: true)
        let worktree = root.appendingPathComponent("repo.wt", isDirectory: true)
        try FileManager.default.createDirectory(
            at: origin.appendingPathComponent("wave/product", isDirectory: true),
            withIntermediateDirectories: true
        )
        try Data("# product\n".utf8).write(
            to: origin.appendingPathComponent("wave/product/GOAL.md")
        )
        defer { try? FileManager.default.removeItem(at: root) }

        try git(["init", "-q"], at: origin)
        try git(["add", "wave/product/GOAL.md"], at: origin)
        try git(
            ["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"],
            at: origin
        )
        try git(["worktree", "add", "-q", worktree.path], at: origin)

        let registered = Wave(
            id: "product",
            name: "product",
            repo: origin.path,
            status: .ready
        )
        let model = WorkModel(query: fixture.query, repoPath: worktree.path)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available([registered]),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        await model.refreshPortfolio(
            initialRepoPath: worktree.path,
            persistedRepos: [
                PortfolioRepo(path: origin.path, lastOpened: .distantPast),
                PortfolioRepo(path: worktree.path, lastOpened: .distantPast),
            ]
        )

        #expect(model.visibleRepos.count == 1)
        #expect(model.repoPath?.normalizedFilePath == origin.path.normalizedFilePath)
        #expect(model.visibleRepos[0].path.normalizedFilePath == origin.path.normalizedFilePath)
        #expect(model.visibleRepos[0].displayName == "repo")
        #expect(model.repoIdentity(model.visibleRepos[0].path) == model.repoIdentity(worktree.path))
        #expect(model.visibleWaves.map(\.displayName) == ["Product"])
        #expect(model.visibleWaves.map(\.isRegistered) == [true])

        let restored = WorkModel(query: fixture.query, repoPath: worktree.path)
        await restored.refreshPortfolio(initialRepoPath: nil)
        #expect(restored.repoPath?.normalizedFilePath == origin.path.normalizedFilePath)
        #expect(restored.allRepos.contains {
            $0.path.normalizedFilePath == origin.path.normalizedFilePath
        })
    }

    @Test("Work selection becomes one server-side Activity filter")
    func workActivityFollowsSelection() async throws {
        let fixture = try WorkTestFixture.load()
        let model = WorkModel(query: fixture.query)
        await model.refresh()

        model.select(.wave(id: "wave-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.project(id: "project-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.task(id: "issue-now"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
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
            guard args.first == "activity" else {
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

    @Test("Session refresh reads Task FlowSteps")
    func sessionRefreshReadsTaskFlowSteps() async throws {
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
        let process = Foundation.Process()
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
        await feed.send(try fixture.heartbeat(sequence: 4))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.task(id: "issue-now")?.task.task.name == "Renamed elsewhere")
        #expect(!model.roadmap.isLoading)
    }

    @Test("Another Machine's frame replaces what the previous Machine showed")
    func anotherMachineDropsPreviousContent() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed), repoPath: "/src/loopflow")
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
        await feed.send(try fixture.heartbeat(sequence: 3, home: "/elsewhere"))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.selection == .task(id: "issue-now"))
        await feed.send(try fixture.planningFrame(sequence: 4, answers: scope.id, renaming: "Other Machine", home: "/elsewhere"))
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
            return filter.includes(row.task, condition: row.condition, now: model.taskHistoryNow)
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

    @Test("A reader that ends leaves the last reading marked unavailable, then recovers")
    func readerEndRecovers() async throws {
        let fixture = try WorkTestFixture.load()
        let feed = WorkFeed()
        let model = WorkModel(query: fixture.streaming(feed))
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }

        try await feed.opened(1)
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil))
        try await eventually { model.roadmap.value != nil }
        await feed.fail(RegistryQueryError("reader exited"))
        try await eventually { model.roadmap.errorMessage == "reader exited" }
        #expect(model.roadmap.value != nil)
        #expect(!model.roadmap.isLoading)

        try await feed.opened(2, within: .seconds(5))
        // The new reader numbers its frames from one.
        await feed.send(try fixture.planningFrame(sequence: 1, answers: nil, renaming: "After restart"))
        try await eventually { model.task(id: "issue-now")?.task.task.name == "After restart" }
        #expect(model.roadmap.errorMessage == nil)
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
            case "roadmap": return roadmapJSON
            case "wave" where args.dropFirst().first == "list": return wavesJSON
            case "session": return #"{"entries":[],"next":null}"#
            case "activity":
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

    private func frame(
        _ part: String, sequence: Int, answers: Int?, home: String = "/home", body: String
    ) throws -> WorkFrame {
        let answers = answers.map(String.init) ?? "null"
        let line = #"{"part":"\#(part)","sequence":\#(sequence),"answers":\#(answers),"home":"\#(home)","revisions":null,"unavailable":null,"body":\#(body)}"#
        return try WorkFrame.decode(line: Data(line.utf8))
    }

    /// `completing` settles that Task successfully a minute ago, with no runtime left.
    func planningFrame(
        sequence: Int, answers: Int?, renaming name: String? = nil, completing task: String? = nil,
        home: String = "/home"
    ) throws -> WorkFrame {
        var roadmap = roadmapJSON
        if let name {
            roadmap = roadmap.replacingOccurrences(of: "Make lf roadmap the machine-wide view", with: name)
        }
        if let task {
            var root = try #require(JSONSerialization.jsonObject(with: Data(roadmap.utf8)) as? [String: Any])
            var waves = try #require(root["waves"] as? [[String: Any]])
            for wave in waves.indices {
                guard var evidence = waves[wave]["tasks"] as? [String: Any],
                      var rows = evidence["items"] as? [[String: Any]] else { continue }
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
                waves[wave]["tasks"] = evidence
            }
            root["waves"] = waves
            roadmap = try #require(String(data: JSONSerialization.data(withJSONObject: root), encoding: .utf8))
        }
        return try frame("planning", sequence: sequence, answers: answers, home: home,
                         body: #"{"roadmap":\#(roadmap),"waves":\#(wavesJSON)}"#)
    }

    func sessionsFrame(sequence: Int, answers: Int?, repo: String, entries: String) throws -> WorkFrame {
        try frame("sessions", sequence: sequence, answers: answers,
                  body: #"{"repo":"\#(repo)","includes_headless":false,"entries":\#(entries)}"#)
    }

    func heartbeat(sequence: Int, home: String = "/home") throws -> WorkFrame {
        try frame("heartbeat", sequence: sequence, answers: nil, home: home, body: #"{"projections":{}}"#)
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
