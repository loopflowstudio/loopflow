import Foundation
import Testing

@testable import Loopflow

/// The Active Sessions census is a pure projection: same inputs, same rows.
/// One mixed fixture drives every rule — red propagation, evidence
/// classification, view-only vs. openable controls, and honest empty states —
/// so the view never has to reason about any of it.
@Suite("Active Sessions census")
struct ActiveSessionsCensusTests {
    private struct CensusInput: Decodable {
        let roadmap: RoadmapSnapshot
        let runs: [SkillRunEntry]
        let handoffs: [InteractiveHandoffListRow]
    }

    private func census() throws -> ActiveSessionsCensus {
        let data = try loadFixtureData("active_sessions_census.json")
        let input = try JSONDecoder().decode(CensusInput.self, from: data)
        return ActiveSessionsCensus(
            roadmap: input.roadmap,
            runs: input.runs,
            handoffs: input.handoffs
        )
    }

    @Test("every Wave becomes a group, and orphaned bodies are never dropped")
    func groupsCoverEveryWaveAndOrphans() throws {
        let census = try census()
        let ids = census.groups.map(\.id)
        #expect(ids == ["wave-product", "wave-remote", "wave-context", "wave-quiet", "unattributed"])
        // The ghost handoff references a Wave absent from the roadmap; it still
        // surfaces rather than vanishing.
        let orphan = try #require(census.groups.first { $0.id == "unattributed" })
        #expect(orphan.rows.count == 1)
        #expect(orphan.rows[0].handoffSessionId == "ih_ghost")
    }

    @Test("a completed session and an unstarted task are not live bodies")
    func livenessFilterDropsTerminalAndUnstarted() throws {
        let product = try group("wave-product")
        let taskRows = product.rows.filter { $0.kind == .task }
        #expect(taskRows.map(\.id) == ["ts_alive", "ts_stale", "ts_dead", "ts_unavail"])
        // The completed handoff never enters the census.
        #expect(!product.rows.contains { $0.handoffSessionId == "ih_done" })
    }

    @Test("a waiting handoff paints its Task, Project, and Wave red while the body lives")
    func waitingHandoffPropagatesRed() throws {
        let product = try group("wave-product")
        #expect(product.tint == .red)
        let task = try row(product, "ts_alive")
        #expect(task.tint == .red)
        // The body is alive and fresh — red comes from the handoff, not the body.
        #expect(task.evidence == .observed)
        let project = try row(product, "ps_prod")
        #expect(project.tint == .red)
    }

    @Test("evidence stays distinguishable: observed, stale, stopped, unavailable, unreachable")
    func evidenceStatesAreDistinct() throws {
        let product = try group("wave-product")
        #expect(try row(product, "ts_alive").evidence == .observed)
        #expect(try row(product, "ts_stale").evidence == .stale)
        #expect(try row(product, "ts_dead").evidence == .stopped)
        #expect(try row(product, "ts_unavail").evidence == .unavailable)

        // Remote Wave that is not live: its live bodies read unreachable, not dead.
        let remote = try group("wave-remote")
        #expect(remote.evidence == .observed)
        #expect(try row(remote, "ts_remote").evidence == .unreachable)
        #expect(try row(remote, "ts_remote").tint == .green)
    }

    @Test("unavailable and missing are different facts, not a healthy empty state")
    func unavailableAndMissingStayDistinct() throws {
        let context = try group("wave-context")
        #expect(context.evidence == .unavailable)
        #expect(context.unavailableReason != nil)
        // Its wave-level waiting handoff still shows and reddens the Wave.
        #expect(context.tint == .red)
        #expect(context.rows.count == 1)
        #expect(context.rows[0].handoffSessionId == "ih_wait_wave_ctx")

        let quiet = try group("wave-quiet")
        #expect(quiet.evidence == .missing)
        #expect(quiet.rows.isEmpty)
        #expect(quiet.tint == .neutral)
    }

    @Test("only interactive handoffs are actionable; every other body is view-only")
    func onlyHandoffsAreOpenable() throws {
        let census = try census()
        for group in census.groups {
            for row in group.rows {
                if row.kind == .handoff {
                    #expect(row.isOpenable)
                    #expect(row.handoffSessionId != nil)
                } else {
                    #expect(row.actions.isEmpty)
                    #expect(row.handoffSessionId == nil)
                }
            }
        }
        let openable = census.groups.flatMap(\.rows).filter(\.isOpenable)
        #expect(openable.count == 4)
    }

    @Test("a direct execution body is visible but carries no controls")
    func directExecutionIsViewOnly() throws {
        let product = try group("wave-product")
        let run = try #require(product.rows.first { $0.kind == .directExecution })
        #expect(run.id == "run:exec-active")
        #expect(run.model == "claude-opus-4-8")
        #expect(run.actions.isEmpty)
        // The finished run is not a live body.
        #expect(!product.rows.contains { $0.id == "run:exec-ended" })
    }

    @Test("VoiceOver labels speak ownership, reason, freshness, and action")
    func accessibilityLabelsAreComplete() throws {
        let product = try group("wave-product")
        let dead = try row(product, "ts_dead")
        let label = dead.accessibilityLabel
        #expect(label.contains("Task session W2-149"))
        #expect(label.contains("red lens"))
        #expect(label.contains("waiting on wave"))
        #expect(label.contains("body exited before hand-back"))
        #expect(label.contains("body stopped"))
        #expect(label.contains("View only"))

        let handoff = try #require(product.rows.first { $0.handoffSessionId == "ih_wait_task" })
        #expect(handoff.accessibilityLabel.contains("Interactive handoff"))
        #expect(handoff.accessibilityLabel.contains("waiting on human"))
        #expect(handoff.accessibilityLabel.contains("Open available"))
    }

    // MARK: - Helpers

    private func group(_ id: String) throws -> ActiveSessionWaveGroup {
        try #require(census().groups.first { $0.id == id })
    }

    private func row(_ group: ActiveSessionWaveGroup, _ id: String) throws -> ActiveSessionRow {
        try #require(group.rows.first { $0.id == id })
    }

    private func loadFixtureData(_ name: String, sourceFile: String = #filePath) throws -> Data {
        let fixtures = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
            .appendingPathComponent(name)
        return try Data(contentsOf: fixtures)
    }
}
