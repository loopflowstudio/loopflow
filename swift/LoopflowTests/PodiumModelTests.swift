#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Podium model")
@MainActor
struct PodiumModelTests {
    @Test("Repository scope filters one shared snapshot and clears outside selection")
    func repositoryScopeFiltersSharedSnapshot() async throws {
        let fixture = try PodiumTestFixture.load()
        let model = PodiumModel(query: fixture.query)

        await model.refresh()
        #expect(model.visibleRoadmaps.map(\.wave.name) == ["product", "context"])

        model.select(.wave(id: "wave-1"))
        model.setRepoPath("/src/context")

        #expect(model.visibleRoadmaps.map(\.wave.name) == ["context"])
        #expect(model.waveSummary?.waves == 1)
        #expect(model.selection == nil)
    }

    @Test("Stable Work selection resolves against the latest snapshot")
    func stableSelectionSurvivesRefresh() async throws {
        let fixture = try PodiumTestFixture.load()
        let model = PodiumModel(query: fixture.query)

        await model.refresh()
        model.select(.task(id: "issue-now"))
        await model.refresh()

        #expect(model.selection == .task(id: "issue-now"))
        #expect(model.task(id: "issue-now")?.task.task.name
            == "Make lf roadmap the machine-wide view")

        model.select(.task(id: "missing"))
        #expect(model.selection == nil)
    }

    @Test("Refresh failure preserves last-good evidence and exposes the reason")
    func refreshFailurePreservesLastGoodEvidence() async throws {
        let fixture = try PodiumTestFixture.load()
        let failing = RegistryQuery { _, _ in
            throw RegistryQueryError("registry unavailable")
        }
        let model = PodiumModel(query: failing)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            processActivity: .available(fixture.processActivity),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        await model.refresh()

        #expect(model.roadmap.value == fixture.roadmap)
        #expect(model.roadmap.errorMessage == "registry unavailable")
        #expect(model.waves.value == fixture.waves)
        #expect(model.waves.errorMessage == "registry unavailable")
        #expect(model.processActivity.value == fixture.processActivity)
        #expect(model.processActivity.errorMessage == "registry unavailable")
        #expect(model.workActivity.value == fixture.workActivity)
        #expect(model.workActivity.errorMessage == "registry unavailable")
        #expect(model.waveSummary?.waves == 2)
    }

    @Test("Live refresh changes only process evidence and preserves its last good frame")
    func liveRefreshChangesOnlyProcessEvidence() async throws {
        let fixture = try PodiumTestFixture.load()
        let frames = LiveProcessFrames(frames: [
            try fixture.processActivityJSON(providersLive: true, observedAt: 1),
            try fixture.processActivityJSON(providersLive: true, observedAt: 2),
        ])
        let query = RegistryQuery { args, _ in
            try await frames.next(args: args)
        }
        let model = PodiumModel(query: query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            processActivity: .available(fixture.processActivity),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        await model.refreshProcessActivity()
        #expect(model.processActivity.value?.observedAt == 1)
        #expect(model.processActivity.value?.nodes.count == 3)

        await model.refreshProcessActivity()
        #expect(model.processActivity.value?.observedAt == 2)
        #expect(model.processActivity.value?.nodes.count == 3)

        await model.refreshProcessActivity()
        #expect(model.processActivity.value?.observedAt == 2)
        #expect(model.processActivity.errorMessage == "no process frame available")
        #expect(await frames.commands == [
            ["ps", "--json"],
            ["ps", "--json"],
            ["ps", "--json"],
        ])
    }

    @Test("Live process refreshes never overlap")
    func liveProcessRefreshesNeverOverlap() async throws {
        let fixture = try PodiumTestFixture.load()
        let deferred = DeferredProcessResponse()
        let query = RegistryQuery { args, _ in
            try await deferred.response(args: args)
        }
        let model = PodiumModel(query: query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            processActivity: .available(fixture.processActivity),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        let first = Task { await model.refreshProcessActivity() }
        await deferred.waitUntilRequested()
        await model.refreshProcessActivity()
        #expect(await deferred.requestCount == 1)

        await deferred.release(try fixture.processActivityJSON(providersLive: true, observedAt: 3))
        await first.value
        #expect(model.processActivity.value?.observedAt == 3)
    }

    @Test("Wave summary counts authored Waves without active Runs")
    func waveSummaryCountsAuthoredWaves() async throws {
        let fixture = try PodiumTestFixture.load()
        let repo = FileManager.default.temporaryDirectory
            .appendingPathComponent("podium-authored-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: repo) }
        for name in ["infrastructure", "intelligence", "product"] {
            let wave = repo
                .appendingPathComponent("wave", isDirectory: true)
                .appendingPathComponent(name, isDirectory: true)
            try FileManager.default.createDirectory(at: wave, withIntermediateDirectories: true)
            try Data("# \(name)\n".utf8).write(to: wave.appendingPathComponent("GOAL.md"))
        }

        let model = PodiumModel(query: fixture.query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available([]),
            processActivity: .available(fixture.processActivity),
            workActivity: .available(fixture.workActivity),
            repos: []
        )
        await model.refreshPortfolio(
            initialRepoPath: nil,
            persistedRepos: [PortfolioRepo(path: repo.path, lastOpened: .distantPast)]
        )
        model.setRepoPath(repo.path)

        #expect(model.visibleWaves.map(\.displayName) == [
            "infrastructure", "intelligence", "product",
        ])
        #expect(model.waveSummary?.waves == 3)
    }

    @Test("A development worktree merges authored and registered Wave identity")
    func developmentWorktreeMergesAuthoredAndRegisteredWave() async throws {
        let fixture = try PodiumTestFixture.load()
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("podium-worktree-\(UUID().uuidString)", isDirectory: true)
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

        func git(_ args: [String], at directory: URL) throws {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["git", "-C", directory.path] + args
            process.standardOutput = Pipe()
            process.standardError = Pipe()
            try process.run()
            process.waitUntilExit()
            try #require(
                process.terminationStatus == 0,
                "git \(args.joined(separator: " "))"
            )
        }

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
            status: .ready,
            live: true
        )
        let model = PodiumModel(query: fixture.query, repoPath: worktree.path)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available([registered]),
            processActivity: .available(fixture.processActivity),
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
        #expect(model.visibleRepos[0].path.normalizedFilePath == worktree.path.normalizedFilePath)
        #expect(model.repoIdentity(model.visibleRepos[0].path) == model.repoIdentity(worktree.path))
        #expect(model.visibleWaves.map(\.displayName) == ["product"])
        #expect(model.visibleWaves.map(\.isRegistered) == [true])
        #expect(model.waveSummary == WaveSummary(waves: 1))
    }

    @Test("Work selection becomes one server-side Activity filter")
    func workActivityFollowsSelection() async throws {
        let fixture = try PodiumTestFixture.load()
        let model = PodiumModel(query: fixture.query)
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
            "--wave", "product", "--project", "loopflow-api", "--json",
        ])

        model.select(.task(id: "issue-now"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
            "--wave", "product", "--project", "loopflow-api",
            "--task", "W2-144", "--json",
        ])
    }

    @Test("Unavailable Activity stays visible while the same scope retries")
    func unavailableActivityStaysVisibleDuringRetry() async throws {
        let fixture = try PodiumTestFixture.load()
        let deferred = DeferredActivityResponse()
        let query = RegistryQuery { args, _ in
            #expect(args.first == "activity")
            return await deferred.response()
        }
        let model = PodiumModel(query: query)

        let failedRefresh = Task { await model.refreshWorkActivity() }
        await deferred.waitUntilRequested()
        await deferred.release("{}")
        await failedRefresh.value
        let failure = try #require(model.workActivity.errorMessage)

        let retry = Task { await model.refreshWorkActivity() }
        await deferred.waitUntilRequested()

        #expect(model.workActivity.errorMessage == failure)
        #expect(!model.workActivity.isLoading)

        let success = try #require(String(data: fixture.workActivityData, encoding: .utf8))
        await deferred.release(success)
        await retry.value
        #expect(model.workActivity.value?.items.count == fixture.workActivity.items.count)
    }

    @Test("A late Activity query cannot replace evidence for a newer selection")
    func staleActivityQueryDoesNotReplaceNewSelection() async throws {
        let fixture = try PodiumTestFixture.load()
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
        let model = PodiumModel(query: query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            processActivity: .available(fixture.processActivity),
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

    @Test("User attention refresh reads the shared Ask queue")
    func userAttentionRefreshReadsSharedQueue() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let json = try String(
            contentsOf: fixtures.appendingPathComponent("ask_attention.json"),
            encoding: .utf8
        )
        let query = RegistryQuery { args, cwd in
            #expect(args == ["ask", "list", "--user", "--json"])
            #expect(cwd == "/src/loopflow")
            return json
        }
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")

        await model.refreshUserAskAttention()

        #expect(model.userAskAttention.value?.map(\.id) == [
            "ask_00000000000000000000000000000001",
        ])
    }

    @Test("Changing repository clears cached Ask attention")
    func changingRepositoryClearsCachedAskAttention() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let json = try String(
            contentsOf: fixtures.appendingPathComponent("ask_attention.json"),
            encoding: .utf8
        )
        let query = RegistryQuery { args, _ in
            #expect(args == ["ask", "list", "--user", "--json"])
            return json
        }
        let model = PodiumModel(query: query, repoPath: "/src/first")
        await model.refreshUserAskAttention()
        #expect(model.userAskAttention.value?.count == 1)

        model.setRepoPath("/src/second")

        #expect(model.userAskAttention.value == nil)
    }

    @Test("A late Ask refresh cannot cross repository scope")
    func staleAskRefreshDoesNotReplaceNewRepository() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let json = try String(
            contentsOf: fixtures.appendingPathComponent("ask_attention.json"),
            encoding: .utf8
        )
        let deferred = DeferredActivityResponse()
        let query = RegistryQuery { args, _ in
            #expect(args == ["ask", "list", "--user", "--json"])
            return await deferred.response()
        }
        let model = PodiumModel(query: query, repoPath: "/src/first")
        let refresh = Task { await model.refreshUserAskAttention() }
        await deferred.waitUntilRequested()

        model.setRepoPath("/src/second")
        await deferred.release(json)
        await refresh.value

        #expect(model.userAskAttention.value == nil)
    }
}

@Suite("Podium process signal")
struct PodiumOutputSignalTests {
    @Test("Exact provider process state drives the signal")
    func processStateDrivesSignal() throws {
        let fixture = try PodiumTestFixture.load()
        let empty = try JSONDecoder().decode(
            ActivitySnapshot.self,
            from: Data(fixture.processActivityJSON(providersLive: false, observedAt: 1).utf8)
        )

        #expect(PodiumSignalState.from(empty) == .off)
        #expect(PodiumSignalState.from(empty).lens == .black)
        #expect(PodiumSignalState.from(fixture.processActivity) == .blocked)
        #expect(PodiumSignalState.from(fixture.processActivity).lens == .blue)

        let silentWorker = ActivityNode(
            id: "provider:1",
            parentId: nil,
            kind: .providerProcess,
            label: "codex",
            repo: "/src/loopflow",
            wave: "product",
            pid: 1,
            startedAt: 1,
            state: .working
        )
        #expect(PodiumSignalState.from(nodes: [silentWorker]) == .producing)
    }
}

private struct PodiumTestFixture {
    let roadmap: RoadmapSnapshot
    let waves: [Wave]
    let processActivity: ActivitySnapshot
    let workActivity: WorkActivitySnapshot
    let workActivityData: Data
    let activityArguments: ActivityArguments
    let query: RegistryQuery

    static func load(sourceFile: String = #filePath) throws -> PodiumTestFixture {
        let fixtures = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let roadmapData = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: roadmapData)
        let processActivityData = try Data(
            contentsOf: fixtures.appendingPathComponent("activity_snapshot.json")
        )
        let processActivity = try JSONDecoder().decode(
            ActivitySnapshot.self,
            from: processActivityData
        )
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
        let processActivityJSON = try #require(String(data: processActivityData, encoding: .utf8))
        let workActivityJSON = try #require(String(data: workActivityData, encoding: .utf8))
        let activityArguments = ActivityArguments()
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return roadmapJSON
            case "ls": return wavesJSON
            case "ps": return processActivityJSON
            case "ask": return "[]"
            case "activity":
                await activityArguments.record(args)
                return workActivityJSON
            default: throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
            }
        }
        return PodiumTestFixture(
            roadmap: roadmap,
            waves: waves,
            processActivity: processActivity,
            workActivity: workActivity,
            workActivityData: workActivityData,
            activityArguments: activityArguments,
            query: query
        )
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

    func processActivityJSON(providersLive: Bool, observedAt: Int64) throws -> String {
        let encoded = try JSONEncoder().encode(processActivity)
        var object = try #require(JSONSerialization.jsonObject(with: encoded) as? [String: Any])
        if !providersLive {
            let nodes = try #require(object["nodes"] as? [[String: Any]])
            object["nodes"] = nodes.filter { $0["kind"] as? String == "exec" }
        }
        object["observed_at"] = observedAt
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

private actor LiveProcessFrames {
    private var frames: [String]
    private(set) var commands: [[String]] = []

    init(frames: [String]) {
        self.frames = frames
    }

    func next(args: [String]) throws -> String {
        commands.append(args)
        guard !frames.isEmpty else {
            throw RegistryQueryError("no process frame available")
        }
        return frames.removeFirst()
    }
}

private actor DeferredProcessResponse {
    private var responseContinuation: CheckedContinuation<String, Error>?
    private var requestContinuation: CheckedContinuation<Void, Never>?
    private(set) var requestCount = 0

    func response(args: [String]) async throws -> String {
        guard args == ["ps", "--json"] else {
            throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
        }
        requestCount += 1
        return try await withCheckedThrowingContinuation { continuation in
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
