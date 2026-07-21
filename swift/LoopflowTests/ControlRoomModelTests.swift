#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Control room model")
@MainActor
struct ControlRoomModelTests {
    @Test("Repository scope filters one shared snapshot and clears outside selection")
    func repositoryScopeFiltersSharedSnapshot() async throws {
        let fixture = try ControlRoomTestFixture.load()
        let model = ControlRoomModel(query: fixture.query)

        await model.refresh()
        #expect(model.visibleRoadmaps.map(\.wave.name) == ["product", "context"])

        model.select(.wave(waveId: "wave-1"))
        model.setRepoPath("/src/context")

        #expect(model.visibleRoadmaps.map(\.wave.name) == ["context"])
        #expect(model.fleetSummary?.registeredWaves == 1)
        #expect(model.fleetSummary?.activeRuns == 0)
        #expect(model.selection == nil)
    }

    @Test("Stable Work selection resolves against the latest snapshot")
    func stableSelectionSurvivesRefresh() async throws {
        let fixture = try ControlRoomTestFixture.load()
        let model = ControlRoomModel(query: fixture.query)

        await model.refresh()
        model.select(.task(waveId: "wave-1", taskId: "issue-now"))
        await model.refresh()

        #expect(model.selection == .task(waveId: "wave-1", taskId: "issue-now"))
        #expect(model.task(waveId: "wave-1", taskId: "issue-now")?.task.task.name
            == "Make lf roadmap the machine-wide view")

        model.select(.task(waveId: "wave-1", taskId: "missing"))
        #expect(model.selection == .wave(waveId: "wave-1"))
    }

    @Test("Refresh failure preserves last-good Work and exposes the reason")
    func refreshFailurePreservesLastGoodWork() async throws {
        let fixture = try ControlRoomTestFixture.load()
        let failing = RegistryQuery { _, _ in
            throw RegistryQueryError("registry unavailable")
        }
        let model = ControlRoomModel(query: failing)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            activity: .available(fixture.activity),
            repos: []
        )

        await model.refresh()

        #expect(model.roadmap.value == fixture.roadmap)
        #expect(model.roadmap.errorMessage == "registry unavailable")
        #expect(model.waves.value == fixture.waves)
        #expect(model.waves.errorMessage == "registry unavailable")
        #expect(model.activity.value == fixture.activity)
        #expect(model.activity.errorMessage == "registry unavailable")
        #expect(model.fleetSummary?.registeredWaves == 2)
    }

    @Test("Activity summary counts providers once and preserves orphan evidence")
    func activitySummaryPreservesOrthogonalEvidence() throws {
        let fixture = try ControlRoomTestFixture.load()

        let summary = fixture.activity.controlRoomSummary

        #expect(summary.activeAgents == 3)
        #expect(summary.working == 1)
        #expect(summary.waiting == 0)
        #expect(summary.stalled == 1)
        #expect(summary.orphaned == 1)
        #expect(summary.unclaimed == 0)
        #expect(summary.outputTokensPerSecond5m == 4.0)
        #expect(summary.measuredOutputTokens == 48_200)
    }

    @Test("Fleet summary keeps active Runs and answering listeners separate")
    func fleetSummaryPreservesOrthogonalEvidence() async throws {
        let fixture = try ControlRoomTestFixture.load()
        let model = ControlRoomModel(query: fixture.query)

        await model.refresh()
        let summary = try #require(model.fleetSummary)

        #expect(summary.registeredWaves == 2)
        #expect(summary.pausedWaves == 1)
        #expect(summary.activeRuns == 1)
        #expect(summary.liveListeners == 1)
        #expect(summary.unservedRuns == 0)
        #expect(summary.activeProjects == 1)
        #expect(summary.activeTasks == 2)

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
            activity: .available(fixture.activity),
            repos: []
        )

        #expect(model.fleetSummary?.activeRuns == 1)
        #expect(model.fleetSummary?.liveListeners == 0)
        #expect(model.fleetSummary?.unservedRuns == 1)
    }

    @Test("Selected Runs require explicit Wave, Project, and Task attribution")
    func selectedRunsFollowWorkAttribution() async throws {
        let fixture = try ControlRoomTestFixture.load()
        let model = ControlRoomModel(query: fixture.query)
        await model.refresh()

        model.select(.wave(waveId: "wave-1"))
        await model.refreshSelectedRuns()
        #expect(model.runs(for: .wave(waveId: "wave-1")).map(\.id)
            == ["invocation-product-run"])

        model.select(.project(waveId: "wave-1", projectId: "project-1"))
        await model.refreshSelectedRuns()
        #expect(model.runs(for: .project(waveId: "wave-1", projectId: "project-1")).map(\.id)
            == ["invocation-product-run"])

        model.select(.task(waveId: "wave-1", taskId: "issue-now"))
        await model.refreshSelectedRuns()
        #expect(model.runs(for: .task(waveId: "wave-1", taskId: "issue-now")).map(\.id)
            == ["invocation-product-run"])

        model.select(.task(waveId: "wave-1", taskId: "issue-available"))
        await model.refreshSelectedRuns()
        #expect(model.runs(for: .task(waveId: "wave-1", taskId: "issue-available")).isEmpty)
    }

    @Test("A late Run query cannot replace evidence for a newer selection")
    func staleRunQueryDoesNotReplaceNewSelection() async throws {
        let fixture = try ControlRoomTestFixture.load()
        var staleRuns = try #require(
            JSONSerialization.jsonObject(with: Data(fixture.runsJSON.utf8))
                as? [[String: Any]]
        )
        staleRuns[0]["id"] = "invocation-stale-wave"
        staleRuns[0]["project"] = NSNull()
        staleRuns[0]["task"] = NSNull()
        let staleData = try JSONSerialization.data(withJSONObject: staleRuns)
        let staleJSON = try #require(String(data: staleData, encoding: .utf8))
        let selectedJSON = fixture.runsJSON
        let deferred = DeferredRunResponse()
        let query = RegistryQuery { args, _ in
            guard args.first == "runs" else {
                throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
            }
            if args.contains("W2-144") { return selectedJSON }
            return await deferred.response()
        }
        let model = ControlRoomModel(query: query)
        model.applyFixture(
            roadmap: .available(fixture.roadmap),
            waves: .available(fixture.waves),
            activity: .available(fixture.activity),
            repos: []
        )

        model.select(.wave(waveId: "wave-1"))
        let staleRefresh = Task { await model.refreshSelectedRuns() }
        await deferred.waitUntilRequested()

        model.select(.task(waveId: "wave-1", taskId: "issue-now"))
        await model.refreshSelectedRuns()
        #expect(model.selectedRuns.value?.map(\.id) == ["invocation-product-run"])

        await deferred.release(staleJSON)
        await staleRefresh.value

        #expect(model.selection == .task(waveId: "wave-1", taskId: "issue-now"))
        #expect(model.selectedRuns.value?.map(\.id) == ["invocation-product-run"])
    }
}

private struct ControlRoomTestFixture {
    let roadmap: RoadmapSnapshot
    let waves: [Wave]
    let activity: ActivitySnapshot
    let runsJSON: String
    let query: RegistryQuery

    static func load(sourceFile: String = #filePath) throws -> ControlRoomTestFixture {
        let url = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmapData = try Data(contentsOf: url)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: roadmapData)
        let activityURL = url.deletingLastPathComponent().appendingPathComponent("activity_snapshot.json")
        let activityData = try Data(contentsOf: activityURL)
        let activity = try JSONDecoder().decode(ActivitySnapshot.self, from: activityData)
        let runsURL = url.deletingLastPathComponent().appendingPathComponent("recent_runs.json")
        let runsData = try Data(contentsOf: runsURL)
        let object = try #require(JSONSerialization.jsonObject(with: roadmapData) as? [String: Any])
        let roadmapWaves = try #require(object["waves"] as? [[String: Any]])
        let waveObjects = try roadmapWaves.map { try #require($0["wave"] as? [String: Any]) }
        let waveData = try JSONSerialization.data(withJSONObject: waveObjects)
        let snapshots = try JSONDecoder().decode([WaveSnapshot].self, from: waveData)
        let waves = snapshots.map { $0.toWave() }
        let roadmapJSON = try #require(String(data: roadmapData, encoding: .utf8))
        let wavesJSON = try #require(String(data: waveData, encoding: .utf8))
        let activityJSON = try #require(String(data: activityData, encoding: .utf8))
        let runsJSON = try #require(String(data: runsData, encoding: .utf8))
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": roadmapJSON
            case "ls": wavesJSON
            case "ps": activityJSON
            case "runs": runsJSON
            default: throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
            }
        }
        return ControlRoomTestFixture(
            roadmap: roadmap,
            waves: waves,
            activity: activity,
            runsJSON: runsJSON,
            query: query
        )
    }
}

private actor DeferredRunResponse {
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
