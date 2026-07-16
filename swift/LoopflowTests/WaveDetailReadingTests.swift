import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave detail reading")
struct WaveDetailReadingTests {
    @Test("a failed refresh preserves the last successful Wave detail")
    func failedRefreshPreservesLastGoodDetail() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        var reading = WaveDetailReading()

        reading.update(detail)
        reading.recordFailure(RegistryQueryError("registry busy"))

        #expect(reading.snapshot?.wave.id == "wave-1")
        #expect(reading.snapshot?.projects[0].project.slug == "release-feedback")
        #expect(reading.errorMessage == "Wave status unavailable: registry busy")
    }

    @Test("a successful refresh clears the stale warning")
    func successfulRefreshClearsWarning() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        var reading = WaveDetailReading()
        reading.recordFailure(RegistryQueryError("registry busy"))

        reading.update(detail)

        #expect(reading.errorMessage == nil)
        #expect(reading.snapshot?.wave.id == "wave-1")
    }

    // The populated detail-pane hierarchy can't be driven live in every
    // environment (a Wave whose registry carries W2-123 lens data needs a
    // schema-current `lf` + populated store). This walks the real populated
    // `lf status --json` fixture through the exact projections the detail-pane
    // Project and Task rows render (`WaveLens.forProject` / `.forTask`,
    // open-task count, KR list) — the mockup hierarchy proven at the data layer.
    @Test("the populated detail hierarchy renders objective, projects, KRs, and shared lenses")
    func populatedDetailHierarchyProjectsThroughLensGrammar() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        let workMap = detail.workMap

        // Objective leads the pane, and Projects stay persistently visible.
        #expect(!workMap.objective.trimmingCharacters(in: .whitespaces).isEmpty)
        #expect(workMap.projects.count == 1)

        let project = try #require(workMap.projects.first)

        // KR list is a Project's strongest quality — it must be present.
        #expect(project.project.krs.count == 1)
        #expect(project.project.krs.allSatisfy { !$0.text.isEmpty })

        // Open-task count is the other headline quality (both fixture tasks open).
        let openTasks = project.tasks.filter { !$0.task.completed }.count
        #expect(openTasks == 2)

        // Project row lens: derived from shared runtime + Task attention only.
        // The runtime body is not running, so the fold wins — a red Task
        // (INF-123) outranks the black one (INF-124): red > green > unknown > black.
        let projectLens = WaveLens.forProject(runtime: project.runtime, tasks: project.tasks)
        #expect(projectLens.color == .red)
        #expect(projectLens.reason == "waiting for review")

        // Task rows: the shared attention level and reason, verbatim — Swift
        // never reconstructs the level from status or process flags.
        let byId = Dictionary(uniqueKeysWithValues: project.tasks.map { ($0.task.identifier, $0) })
        let inf123 = try #require(byId["INF-123"])
        let inf124 = try #require(byId["INF-124"])

        let lens123 = WaveLens.forTask(inf123.attention)
        #expect(lens123.color == .red)
        #expect(lens123.color == WaveLensColor(inf123.attention.level))
        #expect(lens123.reason == inf123.attention.reason)

        let lens124 = WaveLens.forTask(inf124.attention)
        #expect(lens124.color == .black)
        #expect(lens124.color == WaveLensColor(inf124.attention.level))
        #expect(lens124.reason == inf124.attention.reason)

        // Every rendered lens carries a reason — VoiceOver names the state.
        #expect(!projectLens.reason.isEmpty)
        #expect(!lens123.reason.isEmpty)
        #expect(!lens124.reason.isEmpty)
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
