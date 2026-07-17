import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("Turn spend fixture preserves additive identity and absent measurements")
    func turnSpendFixtureRoundTrips() throws {
        let data = try loadFixtureData("turn_spend.json")
        let turns = try JSONDecoder().decode([TurnSpend].self, from: data)

        #expect(turns.map(\.id) == ["turn-1", "turn-2"])
        #expect(turns[0].launchId == "launch-1")
        #expect(turns[0].traceId == "trace-1")
        #expect(turns[0].execId == "exec-1")
        #expect(turns[0].totalTokens == 1200)
        #expect(turns[0].agent == "claude:opus")
        #expect(turns[1].inputTokens == nil)
        #expect(turns[1].outputTokens == 0)
        #expect(turns[1].cacheReadTokens == 150)
        #expect(turns[1].costUsd == nil)

        let encoded = try JSONEncoder().encode(turns)
        let decoded = try JSONDecoder().decode([TurnSpend].self, from: encoded)
        #expect(decoded == turns)
    }

    @Test("Context Lab fixture preserves missing coverage and trace identity")
    func contextLabFixturePreservesResearchTruth() throws {
        let data = try loadFixtureData("context_lab_snapshot.json")
        let snapshot = try JSONDecoder().decode(ContextLabSnapshot.self, from: data)

        #expect(snapshot.totals.runs == 1)
        #expect(snapshot.totals.agentSessions == 1)
        #expect(snapshot.totals.initialPromptTokens == 1_000)
        #expect(snapshot.totals.lifetimeInputTokens == 2_400)
        #expect(snapshot.totals.medianPeakContextPercent == 45)
        #expect(snapshot.coverage.unknownTurns == 1)
        #expect(snapshot.coverage.sourceObservableAgentSessions == 1)
        #expect(snapshot.aggregateRoot.children[0].children[0].children.count == 1)
        #expect(snapshot.query.projects == ["context"])
        #expect(snapshot.query.steeredOnly)
        #expect(snapshot.query.currentRevisionOnly)
        #expect(snapshot.sessions[0].task == "W2-71")
        #expect(snapshot.sessions[0].turns[1].suppliedContextTokens == nil)
        #expect(snapshot.sources[0].impressions == 1)
        #expect(snapshot.sources[0].currentRevisionNodeId == "context-revision")
        #expect(snapshot.sources[1].impressions == nil)
        #expect(snapshot.evidence[0].isEditable)
        #expect(snapshot.evidence[0].currentSourceSha256 == "fedcba9876543210")
        #expect(snapshot.evidence[0].measurements.lastSeen == 120)
        #expect(snapshot.evidence[0].measurements.providerModels[0].model == "gpt-5")
        #expect(snapshot.evidence[0].representatives[0].address.turnId == "turn-1")
    }

    @Test("wave detail fixture preserves Project and Task identity")
    func waveDetailFixturePreservesHierarchy() throws {
        let data = try loadFixtureData("wave_detail.json")
        let detail = try JSONDecoder().decode(WaveDetailSnapshot.self, from: data)

        #expect(detail.wave.home.address == "ssh://jack@mini-heart")
        #expect(detail.wave.home.owner == "jack")
        #expect(detail.wave.home.location == .ssh(host: "mini-heart", port: nil))
        // The Home runtime evidence carries the state and the one contextual action.
        #expect(detail.homeRuntime.state == .running)
        #expect(detail.homeRuntime.action == .attach(endpoint: "127.0.0.1:7777"))
        #expect(detail.projects[0].project.slug == "release-feedback")
        #expect(detail.projects[0].tasks.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.projects[0].tasks[0].prs.compactMap(\.publication?.github?.number) == [912])
        #expect(detail.projects[0].tasks[0].activePr == "pr_33333333333333333333333333333333")
        #expect(detail.projects[0].tasks[0].prs[0].publication?.afterMerge == .completeTask)
        #expect(detail.projects[0].directive?.version == 1)
        #expect(detail.projects[0].tasks[0].directive?.version == 2)
        #expect(detail.projects[0].tasks[0].directive?.incorporatedAt != nil)
        #expect(detail.projects[0].tasks[0].reference.workspace?.slug == "infrastructure-task")
        #expect(detail.projects[0].tasks[0].reference.workspace?.worktree == "/src/loopflow.infrastructure.task")
        #expect(detail.projects[0].tasks[0].reference.issueUrl?.host == "linear.app")
        // The leased body observation rides the runtime snapshot on the wire. A
        // Task waiting on review reads NeedsInput, owned by a human.
        #expect(detail.projects[0].tasks[0].runtime?.observation.category == .needsInput)
        #expect(detail.projects[0].tasks[0].runtime?.observation.owner == .human)
        #expect(detail.projects[0].runtime?.observation.category == .needsInput)
        #expect(detail.projects[0].tasks[1].runtime == nil)
        #expect(detail.projects[0].tasks[1].reference.issueUrl == nil)
        #expect(detail.projects[0].tasks[1].reference.workspace == nil)
        #expect(detail.runs.items[0].traceId == "run-1")
        #expect(detail.runs.items[0].skill == "task_pursue")
        #expect(detail.runs.items[0].suppliedContextTokens == 3000)
        #expect(detail.runs.items[0].status == "ok")
        #expect(detail.attention.items[0].subject == "INF-123")
        #expect(detail.attention.items[0].owner == .review)
        #expect(detail.attention.items[0].reason == "waiting for review")
        #expect(detail.attention.items[0].ageSeconds == 7200)
        #expect(detail.projects[0].tasks[0].attention.level == .red)
        #expect(detail.projects[0].tasks[0].attention.reason == "waiting for review")
    }

    @Test("roadmap fixture preserves sections and durable Task references")
    func roadmapFixturePreservesTaskReferences() throws {
        let data = try loadFixtureData("roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)

        #expect(roadmap.generatedAt == "2026-07-15T00:00:00Z")
        #expect(roadmap.waves.count == 2)
        let product = try #require(roadmap.waves.first)
        #expect(product.wave.name == "product")
        let project = try #require(product.projects.items.first)
        #expect(project.tasks.map(\.section) == [.now, .needsAttention, .available, .later])
        #expect(project.tasks.map(\.attention.level) == [.green, .red, .black, .black])
        #expect(project.tasks[0].reference.workspace?.slug == "make-lf-work-the-machine")
        #expect(project.tasks[2].reference.workspace == nil)
        #expect(project.tasks[2].reference.issueUrl == nil)
        #expect(project.tasks[3].reference.workspace?.branch == "jack-heart/now-available-research")
        #expect(roadmap.waves[1].projects.unavailableReason?.contains("lf pm sync") == true)
    }

    @Test("memory fact fixture preserves every evidence-kind receipt")
    func memoryFactFixturePreservesReceipts() throws {
        let data = try loadFixtureData("receipt.json")
        let fact = try JSONDecoder().decode(MemoryFact.self, from: data)

        #expect(fact.fact == "Workers report through the memory stream, not the journal.")
        #expect(fact.receipts.map(\.kind) == [.chatTurn, .workerReport, .trace, .pm, .pr])
        // A PR reference keeps its `@sha`; its wave differs from the others (the
        // cross-wave case doctor detects downstream).
        let pr = try #require(fact.receipts.last)
        #expect(pr.reference == "loopflow/loopflow#912@abc1234")
        #expect(pr.wave == "auditability")
        #expect(pr.token == "pr:loopflow/loopflow#912@abc1234")

        // Round-trips: re-encode and decode to the same value.
        let reencoded = try JSONEncoder().encode(fact)
        let decoded = try JSONDecoder().decode(MemoryFact.self, from: reencoded)
        #expect(decoded == fact)
    }

    @Test("child activity preserves typed delivery evidence")
    func childActivityPreservesTypedDeliveryEvidence() throws {
        let data = try loadFixtureData("child_control_activity.json")
        let activity = try JSONDecoder().decode(ChildControlActivity.self, from: data)

        #expect(activity.subject == .task)
        #expect(activity.subjectId == "INF-123")
        #expect(activity.kind == .prOpened)
        #expect(activity.title == "Opened PR #1073")
    }
    @Test("Launch surface fixture preserves attach and attention")
    func launchSurfaceFixtureRoundTrips() throws {
        let data = try loadFixtureData("launch_surface.json")
        let surface = try JSONDecoder().decode(LaunchSurfaceRecord.self, from: data)

        #expect(surface.id == "launch_00000000000000000000000000000001")
        #expect(surface.work.kind == "task")
        #expect(surface.status == .live)
        #expect(surface.attention?.kind == "user")
        #expect(surface.argv == ["tmux", "attach-session", "-t", "lf-task"])

        let encoded = try JSONEncoder().encode(surface)
        let decoded = try JSONDecoder().decode(LaunchSurfaceRecord.self, from: encoded)
        #expect(decoded == surface)
    }

    private func loadFixture(_ name: String, sourceFile: String = #filePath) throws -> [String: Any] {
        let data = try loadFixtureData(name, sourceFile: sourceFile)
        let json = try JSONSerialization.jsonObject(with: data)
        return try #require(json as? [String: Any])
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
