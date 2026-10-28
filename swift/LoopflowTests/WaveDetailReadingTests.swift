import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave detail reading")
struct WaveDetailReadingTests {
    @Test("a failed refresh discards volatile status and metric evidence")
    func failedRefreshDiscardsVolatileDetail() throws {
        let detail = try planningWave()
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
        let detail = try planningWave()
        var reading = WaveDetailReading()
        reading.recordFailure(RegistryQueryError("registry busy"))

        reading.update(detail)

        #expect(reading.errorMessage == nil)
        #expect(reading.snapshot?.wave.id == "wave-1")
    }

    @Test("a readable status with no chapter does not revive cached KRs")
    func missingCurrentChapterReplacesCachedPlan() throws {
        let cached = WavePlan(objective: "Earlier objective", projects: try planningWave().workMap.projects)
        #expect(cached.currentProject?.krs.isEmpty == false)

        let current = try planningWave { $0["projects"] = ["state": "unavailable", "reason": "Provider unavailable"] }
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
