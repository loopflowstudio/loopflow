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

        model.select(.wave(waveId: "wave-1"))
        model.setRepoPath("/src/context")

        #expect(model.visibleRoadmaps.map(\.wave.name) == ["context"])
        #expect(model.waveSummary?.registeredWaves == 1)
        #expect(model.waveSummary?.activeRuns == 0)
        #expect(model.selection == nil)
    }

    @Test("Stable Work selection resolves against the latest snapshot")
    func stableSelectionSurvivesRefresh() async throws {
        let fixture = try PodiumTestFixture.load()
        let model = PodiumModel(query: fixture.query)

        await model.refresh()
        model.select(.task(waveId: "wave-1", taskId: "issue-now"))
        await model.refresh()

        #expect(model.selection == .task(waveId: "wave-1", taskId: "issue-now"))
        #expect(model.task(waveId: "wave-1", taskId: "issue-now")?.task.task.name
            == "Make lf roadmap the machine-wide view")

        model.select(.task(waveId: "wave-1", taskId: "missing"))
        #expect(model.selection == .wave(waveId: "wave-1"))
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
        #expect(model.waveSummary?.registeredWaves == 2)
    }

    @Test("Wave summary identifies running Waves without listeners")
    func waveSummaryIdentifiesUnservedRuns() async throws {
        let fixture = try PodiumTestFixture.load()
        let model = PodiumModel(query: fixture.query)

        await model.refresh()
        let summary = try #require(model.waveSummary)

        #expect(summary.registeredWaves == 2)
        #expect(summary.activeRuns == 1)
        #expect(summary.unservedRuns == 0)

        let unserved = fixture.waves.map { wave in
            Wave(
                id: wave.id,
                name: wave.name,
                repo: wave.repo,
                status: wave.status,
                live: false,
                paused: wave.paused,
                activeTasks: wave.activeTasks,
                activeProjects: wave.activeProjects,
                parentWaveId: wave.parentWaveId
            )
        }
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(unserved),
            processActivity: .available(fixture.processActivity),
            workActivity: .available(fixture.workActivity),
            repos: []
        )

        #expect(model.waveSummary?.activeRuns == 1)
        #expect(model.waveSummary?.unservedRuns == 1)
    }

    @Test("Work selection becomes one server-side Activity filter")
    func workActivityFollowsSelection() async throws {
        let fixture = try PodiumTestFixture.load()
        let model = PodiumModel(query: fixture.query)
        await model.refresh()

        model.select(.wave(waveId: "wave-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
            "--wave", "product", "--json",
        ])

        model.select(.project(waveId: "wave-1", projectId: "project-1"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
            "--wave", "product", "--project", "loopflow-api", "--json",
        ])

        model.select(.task(waveId: "wave-1", taskId: "issue-now"))
        await model.refreshWorkActivity()
        #expect(await fixture.activityArguments.last == [
            "activity", "--since", "7d", "--limit", "50",
            "--wave", "product", "--project", "loopflow-api",
            "--task", "W2-144", "--json",
        ])
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

        model.select(.wave(waveId: "wave-1"))
        let staleRefresh = Task { await model.refreshWorkActivity() }
        await deferred.waitUntilRequested()

        model.select(.task(waveId: "wave-1", taskId: "issue-now"))
        await model.refreshWorkActivity()
        #expect(model.workActivity.value?.items.first?.subject == "W2-144")

        await deferred.release(staleJSON)
        await staleRefresh.value

        #expect(model.selection == .task(waveId: "wave-1", taskId: "issue-now"))
        #expect(model.workActivity.value?.items.first?.subject == "W2-144")
    }
}

@Suite("Podium output signal")
struct PodiumOutputSignalTests {
    @Test("Black is off and blue is blocked without manufacturing output")
    func stateSeparatesRateFromAttention() throws {
        let empty = try JSONDecoder().decode(
            ActivitySnapshot.self,
            from: Data(#"{"schema_version":1,"observed_at":1784606400,"fast_window_seconds":300,"slow_window_seconds":1800,"aggregate":{"measured_output_tokens":0,"output_tokens_fast":0,"output_tokens_slow":0,"output_tokens_per_second_fast":0.0,"output_tokens_per_second_slow":0.0,"measured_turns":0,"unmeasured_turns":0},"nodes":[],"provider_processes":[]}"#.utf8)
        )
        let fixture = try PodiumTestFixture.load()

        #expect(PodiumSignalState.from(empty) == .off)
        #expect(PodiumSignalState.from(empty).lens == .black)
        #expect(PodiumSignalState.from(fixture.processActivity) == .blocked)
        #expect(PodiumSignalState.from(fixture.processActivity).lens == .blue)
    }

    @Test("The rail is logarithmic, monotonic, and capped")
    func rateScaleIsBounded() {
        #expect(TokenRateScale.level(0) == 0)
        #expect(TokenRateScale.level(1) < TokenRateScale.level(4))
        #expect(TokenRateScale.level(4) < TokenRateScale.level(10))
        #expect(TokenRateScale.level(10) == 1)
        #expect(TokenRateScale.level(100) == 1)
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
