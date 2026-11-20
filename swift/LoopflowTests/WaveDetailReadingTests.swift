import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave detail reading")
struct WaveDetailReadingTests {
    @Test("a failed refresh discards volatile status")
    func failedRefreshDiscardsVolatileDetail() throws {
        let detail = try planningWave()
        var reading = WaveDetailReading()

        reading.update(detail)
        #expect(reading.snapshot?.wave.id == "wave-1")
        reading.clear()

        #expect(reading.snapshot == nil)
    }

    @Test("an unavailable Project reading replaces cached Projects")
    func unavailableProjectsReplaceCachedPlan() throws {
        let cached = WavePlan(objective: "Earlier objective", projects: try planningWave().workMap.projects)
        #expect(!cached.projects.items.isEmpty)

        let current = try planningWave { $0["projects"] = ["state": "unavailable", "reason": "Provider unavailable"] }
        var reading = WaveDetailReading()
        reading.update(current)

        #expect(reading.plan(cached: cached).projects == .unavailable(reason: "Provider unavailable"))
        #expect(reading.plan(cached: cached).objective == current.workMap.objective)

        reading.clear()
        #expect(reading.plan(cached: cached) == cached)
    }

    /// `wave-1` of the planning fixture, after `edit` reshapes its wire entry.
    private func planningWave(_ edit: (inout [String: Any]) -> Void = { _ in }) throws -> WaveRoadmap {
        var wire = try #require(JSONSerialization.jsonObject(with: loadFixtureData("roadmap_snapshot.json")) as? [String: Any])
        var waves = try #require(wire["waves"] as? [[String: Any]])
        edit(&waves[0])
        wire["waves"] = waves
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        return try #require(roadmap.waves.first { $0.wave.id == "wave-1" })
    }

    private func loadFixtureData(_ name: String, sourceFile: String = #filePath) throws -> Data {
        let testFile = URL(fileURLWithPath: sourceFile)
        let fixtures = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
            .appendingPathComponent(name)
        return try Data(contentsOf: fixtures)
    }
}
