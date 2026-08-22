import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave detail reading")
struct WaveDetailReadingTests {
    @Test("a failed refresh discards volatile status and metric evidence")
    func failedRefreshDiscardsVolatileDetail() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        var reading = WaveDetailReading()

        reading.update(detail)
        #expect(reading.snapshot?.metricPortfolio.metrics.first?.evidence == .met(
            value: 1,
            sourceWindowStart: "2026-08-13T18:00:00Z",
            sourceWindowEnd: "2026-08-20T18:00:00Z"
        ))
        reading.recordFailure(RegistryQueryError("registry busy"))

        #expect(reading.snapshot == nil)
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

    @Test("Mac metric rows render the shared DTO fields without recomputing evidence")
    func metricRowPresentationUsesSharedEvidence() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        let met = try #require(detail.metricPortfolio.metrics.first)
        let metRow = WaveMetricRowPresentation(metric: met, owner: "Loopflow API")

        #expect(metRow.name == "Task loops earn trust")
        #expect(metRow.description == "Fraction of Task epochs settled during the trailing seven days that either completed with every PR landed through Loopflow auto-merge or stopped with a non-resumable failure receipt. Open epochs are excluded. A user-landed PR or manual Git repair inside the Task epoch fails the metric.")
        #expect(metRow.state == "Met")
        #expect(metRow.owner == "Loopflow API")
        #expect(metRow.instrumentState == "Instrumented")
        #expect(metRow.value == "100%")
        #expect(metRow.target == "≥ 100%")
        #expect(metRow.window == "7d")
        #expect(metRow.freshness == "Fresh until 2026-08-22T00:00:00Z")
        #expect(metRow.reason == nil)

        let portfolio = try JSONDecoder().decode(
            MetricPortfolio.self,
            from: loadFixtureData("metric_portfolio.json")
        )
        let unavailable = try #require(portfolio.metrics.first {
            if case .unavailable = $0.evidence { return true }
            return false
        })
        let unavailableRow = WaveMetricRowPresentation(
            metric: unavailable,
            owner: "Loopflow API"
        )
        #expect(unavailableRow.state == "Unavailable")
        #expect(unavailableRow.instrumentState == "Instrumented")
        #expect(unavailableRow.value == "—")
        #expect(unavailableRow.reason == "source timeout · source time 2026-08-20T18:00:00Z")
    }

    @Test("Mac metric summary distinguishes official health, candidates, and contract issues")
    func metricPortfolioSummaryKeepsBoundariesVisible() throws {
        let portfolio = try JSONDecoder().decode(
            MetricPortfolio.self,
            from: loadFixtureData("metric_portfolio.json")
        )

        let presentation = WaveMetricPortfolioPresentation(portfolio: portfolio)

        #expect(presentation.officialCount == 3)
        #expect(presentation.candidateCount == 6)
        #expect(presentation.holdingCount == 1)
        #expect(presentation.needsAttentionCount == 2)
        #expect(presentation.contractIssueCount == 4)
        #expect(presentation.headline == "1 of 3 official measures currently hold.")
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
        #expect(projectLens.reason == "merge pull request head 333333333333 on GitHub")

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
