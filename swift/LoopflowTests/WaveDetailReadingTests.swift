import Foundation
import SwiftUI
import Testing
import ViewInspector

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

    @Test("an unavailable Project reading replaces the cached Project")
    func unavailableProjectReplacesCachedPlan() throws {
        let cached = WavePlan(objective: "Earlier objective", project: try planningWave().workMap.project)
        #expect(cached.project.present != nil)

        let current = try planningWave { $0["project"] = ["state": "unavailable", "reason": "Provider unavailable"] }
        var reading = WaveDetailReading()
        reading.update(current)

        #expect(reading.plan(cached: cached).project == .unavailable(reason: "Provider unavailable"))
        #expect(reading.plan(cached: cached).objective == current.workMap.objective)

        reading.clear()
        #expect(reading.plan(cached: cached) == cached)
    }

    @Test("the Wave header names its Project and links it once the tracker has a URL")
    @MainActor
    func headerLinksTheProject() throws {
        let linked = try #require(try planningWave().workMap.project.present)
        let link = try WaveHeaderProject(project: linked).inspect().find(ViewType.Link.self)
        #expect(try link.url() == URL(string: "https://linear.app/loopflow/project/current-1a2b3c4d5e6f"))
        #expect(try link.labelView().text().string() == "current")

        let local = try #require(try planningWave { wave in
            var reading = wave["project"] as! [String: Any]
            var project = reading["value"] as! [String: Any]
            project["url"] = NSNull()
            reading["value"] = project
            wave["project"] = reading
        }.workMap.project.present)
        let header = try WaveHeaderProject(project: local).inspect()
        #expect(throws: (any Error).self) { try header.find(ViewType.Link.self) }
        #expect(try header.find(text: "current").string() == "current")
    }

    @Test("an unbound Project says under its name which command makes it in the tracker")
    @MainActor
    func unboundProjectNamesItsMaker() throws {
        let sentence = "not in Linear yet; `lf wave plan connect current` makes it"
        let unbound = try #require(try planningWave { wave in
            var reading = wave["project"] as! [String: Any]
            var project = reading["value"] as! [String: Any]
            project["tracker_id"] = NSNull()
            project["url"] = NSNull()
            project["unbound"] = sentence
            reading["value"] = project
            wave["project"] = reading
        }.workMap.project.present)
        let view = try WaveProjectView(project: unbound, repo: "/repo").inspect()
        #expect(try view.find(text: sentence).string() == sentence)

        let linked = try #require(try planningWave().workMap.project.present)
        #expect(throws: (any Error).self) {
            try WaveProjectView(project: linked, repo: "/repo").inspect().find(text: sentence)
        }
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
