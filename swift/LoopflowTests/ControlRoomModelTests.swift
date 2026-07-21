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
            repos: []
        )

        await model.refresh()

        #expect(model.roadmap.value == fixture.roadmap)
        #expect(model.roadmap.errorMessage == "registry unavailable")
        #expect(model.waves.value == fixture.waves)
        #expect(model.waves.errorMessage == "registry unavailable")
    }
}

private struct ControlRoomTestFixture {
    let roadmap: RoadmapSnapshot
    let waves: [Wave]
    let query: RegistryQuery

    static func load(sourceFile: String = #filePath) throws -> ControlRoomTestFixture {
        let url = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmapData = try Data(contentsOf: url)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: roadmapData)
        let object = try #require(JSONSerialization.jsonObject(with: roadmapData) as? [String: Any])
        let roadmapWaves = try #require(object["waves"] as? [[String: Any]])
        let waveObjects = try roadmapWaves.map { try #require($0["wave"] as? [String: Any]) }
        let waveData = try JSONSerialization.data(withJSONObject: waveObjects)
        let snapshots = try JSONDecoder().decode([WaveSnapshot].self, from: waveData)
        let waves = snapshots.map { snapshot in
            Wave(
                id: snapshot.id,
                name: snapshot.name,
                repo: snapshot.repo,
                status: snapshot.status,
                live: snapshot.live,
                activeTasks: snapshot.activeTasks,
                activeProjects: snapshot.activeProjects,
                parentWaveId: snapshot.parentWaveId
            )
        }
        let roadmapJSON = try #require(String(data: roadmapData, encoding: .utf8))
        let wavesJSON = try #require(String(data: waveData, encoding: .utf8))
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": roadmapJSON
            case "ls": wavesJSON
            default: throw RegistryQueryError("unexpected command \(args.joined(separator: " "))")
            }
        }
        return ControlRoomTestFixture(roadmap: roadmap, waves: waves, query: query)
    }
}
#endif
