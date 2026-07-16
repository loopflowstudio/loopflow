import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
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

    @Test("claim citation fixture preserves Project/KR identity")
    func claimCitationFixturePreservesReceipts() throws {
        let data = try loadFixtureData("claim_citation.json")
        let citation = try JSONDecoder().decode(ClaimCitation.self, from: data)

        #expect(citation.claimId == "95159066-9098-4d0b-8903-01459dc7ec14#0")
        #expect(citation.receipts.map(\.kind) == [.pm, .pr])
        let decoded = try JSONDecoder().decode(
            ClaimCitation.self,
            from: JSONEncoder().encode(citation)
        )
        #expect(decoded == citation)
    }

    @Test("child control activity preserves typed command evidence")
    func childControlActivityPreservesTypedCommandEvidence() throws {
        let data = try loadFixtureData("child_control_activity.json")
        let activity = try JSONDecoder().decode(ChildControlActivity.self, from: data)

        #expect(activity.subject == .task)
        #expect(activity.subjectId == "INF-123")
        #expect(activity.kind == .controlApplied)
        #expect(activity.directiveVersion == nil)
        #expect(activity.effect == .liveSteer)
        #expect(activity.source == .wave(id: "11111111-1111-4111-8111-111111111111"))
    }

    @Test("interactive handoff attach fixture preserves presentation instructions")
    func interactiveHandoffAttachFixtureRoundTrips() throws {
        let data = try loadFixtureData("interactive_handoff_attach.json")
        let attach = try JSONDecoder().decode(InteractiveHandoffAttach.self, from: data)

        #expect(attach.sessionId == "ih_00000000000000000000000000000001")
        #expect(attach.status == .attached)
        #expect(attach.cwd == "/src/loopflow.interactive-handoff")
        #expect(attach.host == "localhost")
        #expect(attach.environment["LF_HOME"] == "/Users/jack/.lf")
        #expect(attach.argv == ["tmux", "attach-session", "-t", "lf-task-interactive"])

        let encoded = try JSONEncoder().encode(attach)
        let decoded = try JSONDecoder().decode(InteractiveHandoffAttach.self, from: encoded)
        #expect(decoded == attach)
    }

    @Test("interactive handoff list fixture preserves census identity and age")
    func interactiveHandoffListFixtureRoundTrips() throws {
        let data = try loadFixtureData("interactive_handoff_list.json")
        let rows = try JSONDecoder().decode([InteractiveHandoffListRow].self, from: data)

        #expect(rows.count == 2)
        let waiting = try #require(rows.first)
        #expect(waiting.sessionId == "ih_00000000000000000000000000000001")
        #expect(waiting.parentKind == "task")
        #expect(waiting.parentId == "ts_00000000000000000000000000000002")
        #expect(waiting.status == .waiting)
        #expect(waiting.status.isActive)
        #expect(waiting.home == "jack@local")
        #expect(waiting.ageSecs == 90)

        let attached = rows[1]
        #expect(attached.parentKind == "project")
        #expect(attached.providerSessionId == nil)
        // An unreadable timestamp keeps age nil, never a fabricated zero.
        #expect(attached.ageSecs == nil)
        #expect(attached.status.isActive)

        let encoded = try JSONEncoder().encode(rows)
        let decoded = try JSONDecoder().decode([InteractiveHandoffListRow].self, from: encoded)
        #expect(decoded == rows)
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
