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

    @Test("a readable status with no chapter does not revive cached KRs")
    func missingCurrentChapterReplacesCachedPlan() throws {
        let data = try loadFixtureData("wave_detail.json")
        let cachedDetail = try JSONDecoder().decode(WaveDetailSnapshot.self, from: data)
        let cached = WavePlan(objective: "Earlier objective", projects: cachedDetail.workMap.projects)
        #expect(cached.currentProject?.krs.isEmpty == false)

        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire["projects"] = ["state": "unavailable", "reason": "Provider unavailable"]
        let current = try JSONDecoder().decode(
            WaveDetailSnapshot.self, from: JSONSerialization.data(withJSONObject: wire)
        )
        var reading = WaveDetailReading()
        reading.update(current)

        #expect(reading.plan(cached: cached).currentProject == nil)
        #expect(reading.plan(cached: cached).objective == current.workMap.objective)

        reading.recordFailure(RegistryQueryError("registry unavailable"))
        #expect(reading.plan(cached: cached) == cached)
        #expect(reading.errorMessage != nil)
    }

    @Test("Mac metric rows render the shared DTO fields without recomputing evidence")
    func metricRowPresentationUsesSharedEvidence() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        let met = try #require(detail.metricPortfolio.metrics.first)
        let metRow = WaveMetricRowPresentation(metric: met, owner: "Loopflow API", targetUnavailable: false)

        #expect(metRow.name == "Task loops earn trust")
        #expect(metRow.description == "Fraction of Tasks settled during the trailing seven days that either completed with every PR landed through Loopflow auto-merge or stopped with a non-resumable failure receipt. Open Tasks are excluded. A user-landed PR or manual Git repair inside the Task fails the metric.")
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
            owner: "Loopflow API", targetUnavailable: false
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

        #expect(presentation.officialCount == 4)
        #expect(presentation.candidateCount == 7)
        #expect(presentation.holdingCount == 1)
        #expect(presentation.requiresWorkCount == 2)
        #expect(presentation.contractIssueCount == 5)
        #expect(presentation.headline == "Project targets unavailable.")
        #expect(!presentation.targetUnavailable(for: try #require(portfolio.metrics.first)))
        let known = WaveMetricPortfolioPresentation(portfolio: MetricPortfolio(
            metrics: portfolio.metrics, contractIssues: []
        ))
        #expect(known.headline == "1 of 3 Project targets currently hold.")
    }

    @Test("an unset chapter target preserves the reading without failing the measure")
    func untargetedMetricIsNeutral() throws {
        let portfolio = try JSONDecoder().decode(MetricPortfolio.self, from: loadFixtureData("metric_portfolio.json"))
        let metric = try #require(portfolio.metrics.first { $0.target == nil })
        let row = WaveMetricRowPresentation(metric: metric, owner: "Wave", targetUnavailable: false)
        #expect(row.state == "No target")
        #expect(row.target == "unset for this Project")
        #expect(row.value != "—")
        let presentation = WaveMetricPortfolioPresentation(portfolio: MetricPortfolio(metrics: [metric], contractIssues: []))
        #expect(presentation.requiresWorkCount == 0)
        #expect(presentation.holdingCount == 0)
        #expect(presentation.headline == "No targets set for this Project.")
    }

    @Test("unavailable chapter planning retains the reading and never claims no target")
    func unavailableChapterIsExplicit() throws {
        let fixture = try JSONDecoder().decode(MetricPortfolio.self, from: loadFixtureData("metric_portfolio.json"))
        let metric = try #require(fixture.metrics.first { $0.identity.metricId == "target-unavailable" })
        for metrics in [[], [metric]] {
            let presentation = WaveMetricPortfolioPresentation(portfolio: MetricPortfolio(
                metrics: metrics, contractIssues: [.chapterUnavailable(waveId: "wave-unavailable", reason: "PM snapshot unavailable")]
            ))
            #expect(presentation.headline == "Project targets unavailable.")
            #expect(presentation.requiresWorkCount == 0)
            let row = WaveMetricRowPresentation(metric: metric, owner: "Wave", targetUnavailable: presentation.targetUnavailable(for: metric))
            #expect(row.target == "unavailable for this Project")
            #expect(row.state == "Unknown")
            #expect(row.value == "100%")
            #expect(row.reason == "Project target planning is unavailable.")
        }
    }

    // The populated detail-pane hierarchy can't be driven live in every
    // environment (a Wave whose registry carries W2-123 lens data needs a
    // schema-current `lf` + populated store). This walks the real populated
    // `lf wave status --json` fixture through the exact projections the detail-pane
    // Project and Task rows render (`WaveLens.forTasks` / `.forTask`,
    // open-task count, KR list) — the mockup hierarchy proven at the data layer.
    @Test("the populated detail hierarchy renders objective, chapter KRs, Tasks, and shared lenses")
    func populatedDetailHierarchyProjectsThroughLensGrammar() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        let workMap = detail.workMap

        // The Wave objective leads the pane; chapter KRs and Tasks share its scope.
        #expect(!workMap.objective.trimmingCharacters(in: .whitespaces).isEmpty)
        #expect(workMap.currentProject != nil)

        let chapter = try #require(workMap.currentProject)
        let tasks = workMap.tasks.items

        // KR list is a Project's strongest quality — it must be present.
        #expect(chapter.krs.count == 1)
        #expect(chapter.krs.allSatisfy { !$0.text.isEmpty })

        // Open-task count is the other headline quality (both fixture tasks open).
        let openTasks = tasks.filter { !$0.task.completed }.count
        #expect(openTasks == 2)

        // Project row lens: derived from Task condition only. A waiting Task
        // (INF-123) outranks the clear one (INF-124).
        let projectLens = WaveLens.forTasks(tasks: tasks)
        #expect(projectLens.color == .blue)
        #expect(projectLens.reason == "merge pull request head 333333333333 on GitHub")

        // Task rows: the shared condition and reason, verbatim — Swift
        // never reconstructs the level from status or process flags.
        let byId = Dictionary(uniqueKeysWithValues: tasks.map { ($0.task.identifier, $0) })
        let inf123 = try #require(byId["INF-123"])
        let inf124 = try #require(byId["INF-124"])

        let lens123 = WaveLens.forTask(inf123.condition)
        #expect(lens123.color == .blue)
        #expect(lens123.color == WaveLensColor(inf123.condition.state))
        #expect(lens123.reason == inf123.condition.reason)

        let lens124 = WaveLens.forTask(inf124.condition)
        #expect(lens124.color == .black)
        #expect(lens124.color == WaveLensColor(inf124.condition.state))
        #expect(lens124.reason == inf124.condition.reason)

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
