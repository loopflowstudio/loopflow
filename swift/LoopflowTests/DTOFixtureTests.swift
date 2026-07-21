import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("PM snapshot fixture preserves repository Team and Project ownership")
    func pmShowFixturePreservesOwnership() async throws {
        let data = try loadFixtureData("pm_show.json")
        let json = String(decoding: data, as: UTF8.self)
        let query = RegistryQuery { args, _ in
            #expect(args == ["pm", "show", "--wave", "survival/infrastructure", "--json", "--no-sync"])
            return json
        }

        let plan = try await query.plan(
            wave: "survival/infrastructure",
            objective: "Keep mail flowing.",
            cwd: "/fixture"
        )
        #expect(plan.projects.map(\.id) == ["gmail"])
        #expect(plan.projects[0].title == "Gmail")
        #expect(plan.projects[0].krs[0].proof == .holds)
    }

    @Test("Turn spend fixture preserves additive identity and absent measurements")
    func turnSpendFixtureRoundTrips() throws {
        let data = try loadFixtureData("turn_spend.json")
        let turns = try JSONDecoder().decode([TurnSpend].self, from: data)

        #expect(turns.map(\.id) == ["turn-1", "turn-2"])
        #expect(turns[0].invocationId == "invocation-1")
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

    @Test("Activity fixture preserves process state and exact output evidence")
    func activityFixtureRoundTrips() throws {
        let data = try loadFixtureData("activity_snapshot.json")
        let snapshot = try JSONDecoder().decode(ActivitySnapshot.self, from: data)

        #expect(snapshot.schemaVersion == 1)
        #expect(snapshot.fastWindowSeconds == 300)
        #expect(snapshot.aggregate.measuredOutputTokens == 48_200)
        #expect(snapshot.aggregate.outputTokensPerSecondFast == 4.0)
        #expect(snapshot.nodes.filter { $0.kind == .providerLaunch }.map(\.state)
            == [.working, .stalled])
        #expect(snapshot.nodes.filter { $0.kind == .providerLaunch }.map(\.wave)
            == ["product", "product"])
        #expect(snapshot.nodes.filter { $0.kind == .providerLaunch }.map(\.project)
            == ["loopflow-api", "loopflow-api"])
        #expect(snapshot.nodes.filter { $0.kind == .providerLaunch }.map(\.task)
            == ["W2-144", "W2-144"])
        #expect(snapshot.providerProcesses[0].claim == .orphaned)

        let encoded = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(ActivitySnapshot.self, from: encoded)
        #expect(decoded == snapshot)
    }

    @Test("Work Activity fixture preserves proof links and typed facts")
    func workActivityFixturePreservesProof() throws {
        let data = try loadFixtureData("work_activity_snapshot.json")
        let snapshot = try JSONDecoder().decode(WorkActivitySnapshot.self, from: data)

        #expect(snapshot.limit == 50)
        #expect(snapshot.items.map(\.subject) == [
            "W2-144", "W2-144", "W2-144", "product", "mac-surface-ux",
        ])
        #expect(snapshot.items[0].fact.invocationId == "invocation-product-run")
        #expect(snapshot.items[1].fact.github?.number == 1144)
        #expect(snapshot.items[1].fact.github?.url.host == "github.com")
        if case .prMergeRequested(_, let request, _) = snapshot.items[2].fact {
            #expect(request.requestedAt == "2026-07-21T18:35:51Z")
        } else {
            Issue.record("expected a typed PR merge request")
        }
        #expect(snapshot.items[3].work.kind == .wave)
        if case .steerIssued(_, .run(let id)) = snapshot.items[3].fact {
            #expect(id == "run_00000000000000000000000000000001")
        } else {
            Issue.record("expected a Run-authored Steer")
        }
        #expect(snapshot.items[4].fact == .workCreated)
    }

    @Test("Context Lab fixture preserves missing coverage and trace identity")
    func contextLabFixturePreservesResearchTruth() throws {
        let data = try loadFixtureData("context_lab_snapshot.json")
        let snapshot = try JSONDecoder().decode(ContextLabSnapshot.self, from: data)

        #expect(snapshot.totals.runs == 1)
        #expect(snapshot.totals.invocations == 1)
        #expect(snapshot.totals.initialPromptTokens == 1_000)
        #expect(snapshot.totals.lifetimeInputTokens == 2_400)
        #expect(snapshot.totals.medianPeakContextPercent == 45)
        #expect(snapshot.coverage.unknownTurns == 1)
        #expect(snapshot.coverage.sourceObservableInvocations == 1)
        #expect(snapshot.aggregateRoot.children[0].children[0].children.count == 1)
        #expect(snapshot.query.projects == ["context"])
        #expect(snapshot.query.steeredOnly)
        #expect(snapshot.query.currentRevisionOnly)
        #expect(snapshot.invocations[0].task == "W2-71")
        #expect(snapshot.invocations[0].turns[1].suppliedContextTokens == nil)
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

        #expect(detail.wave.home.id == "home_00000000000000000000000000000001")
        #expect(detail.wave.home.route == "ssh://jack@mini-heart")
        #expect(!detail.wave.paused)
        // The Home runtime evidence carries the state and the one contextual action.
        #expect(detail.homeRuntime.state == .running)
        #expect(detail.homeRuntime.action == .attach(endpoint: "127.0.0.1:7777"))
        #expect(detail.projects[0].project.slug == "release-feedback")
        #expect(detail.unavailableProjects[0].workId == "proj_e972b70272fbb5e91c096ebe657f9f9b")
        #expect(detail.unavailableProjects[0].projectId == "f56c583c-c360-4dc4-ba12-4b5a02268623")
        #expect(detail.unavailableProjects[0].projectSlug == "technical-architecture")
        #expect(detail.unavailableProjects[0].status == .abandoned)
        #expect(detail.unavailableProjects[0].owner == .wave)
        #expect(detail.unavailableProjects[0].tasks[0].taskIdentifier == "W2-127")
        #expect(detail.unavailableProjects[0].tasks[0].status == .ready)
        #expect(detail.unavailableProjects[0].tasks[0].owner == .wave)
        #expect(detail.projects[0].tasks.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.projects[0].tasks[0].prs.compactMap(\.publication?.github?.number) == [912])
        #expect(detail.projects[0].tasks[0].activePr == "pr_33333333333333333333333333333333")
        #expect(detail.projects[0].tasks[0].prs[0].publication?.merge?.afterMerge == .completeTask)
        #expect(detail.projects[0].directive?.version == 1)
        #expect(detail.projects[0].tasks[0].directive?.version == 2)
        #expect(detail.projects[0].tasks[0].directive?.incorporatedAt != nil)
        #expect(detail.projects[0].tasks[0].reference.workspace?.slug == "infrastructure-task")
        #expect(detail.projects[0].tasks[0].reference.workspace?.worktree == "/src/loopflow.infrastructure.task")
        #expect(detail.projects[0].tasks[0].reference.issueUrl?.host == "linear.app")
        // The leased body observation rides the runtime snapshot on the wire. A
        // Task waiting on review reads NeedsInput, owned by a human.
        #expect(detail.projects[0].tasks[0].runtime?.observation.category == .needsInput)
        #expect(detail.projects[0].tasks[0].runtime?.observation.owner == .user)
        #expect(detail.projects[0].runtime?.observation.category == .needsInput)
        #expect(detail.projects[0].tasks[1].runtime == nil)
        #expect(detail.projects[0].tasks[1].reference.issueUrl == nil)
        #expect(detail.projects[0].tasks[1].reference.workspace == nil)
        #expect(detail.runs.items[0].traceId == "run-1")
        #expect(detail.runs.items[0].skill == "task/pursue")
        #expect(detail.runs.items[0].suppliedContextTokens == 3000)
        #expect(detail.runs.items[0].status == "ok")
        #expect(detail.attention.items[0].subject == "INF-123")
        #expect(detail.attention.items[0].owner == .user)
        #expect(detail.attention.items[0].reason == "merge pull request head 333333333333 on GitHub")
        #expect(detail.attention.items[0].ageSeconds == 7200)
        #expect(detail.projects[0].tasks[0].attention.level == .red)
        #expect(detail.projects[0].tasks[0].attention.reason == "merge pull request head 333333333333 on GitHub")

        var legacy = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var legacyWave = try #require(legacy["wave"] as? [String: Any])
        legacyWave.removeValue(forKey: "paused")
        legacy["wave"] = legacyWave
        let legacyData = try JSONSerialization.data(withJSONObject: legacy)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(WaveDetailSnapshot.self, from: legacyData)
        }
    }

    @Test("roadmap fixture preserves sections and durable Task references")
    func roadmapFixturePreservesTaskReferences() throws {
        let data = try loadFixtureData("roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)

        #expect(roadmap.generatedAt == "2026-07-15T00:00:00Z")
        #expect(roadmap.waves.count == 2)
        let product = try #require(roadmap.waves.first)
        #expect(product.wave.name == "product")
        #expect(product.wave.paused)
        #expect(product.unavailableProjects[0].workId == "proj_e972b70272fbb5e91c096ebe657f9f9b")
        #expect(product.unavailableProjects[0].projectSlug == "technical-architecture")
        #expect(product.unavailableProjects[0].tasks[0].taskIdentifier == "W2-127")
        #expect(product.unavailableProjects[0].tasks[0].recovery.contains("lf work abandon task task_40fbeea"))
        let project = try #require(product.projects.items.first)
        #expect(project.tasks.map(\.section) == [.now, .needsAttention, .available, .later])
        #expect(project.tasks.map(\.attention.level) == [.green, .red, .black, .black])
        #expect(project.tasks[0].reference.workspace?.slug == "make-lf-work-the-machine")
        #expect(project.tasks[2].reference.workspace == nil)
        #expect(project.tasks[2].reference.issueUrl == nil)
        #expect(project.tasks[3].reference.workspace?.branch == "jack-heart/now-available-research")
        #expect(roadmap.waves[1].projects.unavailableReason?.contains("lf pm sync") == true)
    }

    @Test("child activity preserves typed delivery evidence")
    func childActivityPreservesTypedDeliveryEvidence() throws {
        let data = try loadFixtureData("child_control_activity.json")
        let activity = try JSONDecoder().decode(ChildControlActivity.self, from: data)

        #expect(activity.subject == .task)
        #expect(activity.subjectId == "INF-123")
        #expect(activity.workId == "ts_22222222222222222222222222222222")
        #expect(activity.kind == .prOpened)
        #expect(activity.title == "Opened PR #1073")
    }
    @Test("Invocation surface fixture preserves Run ownership and attach")
    func invocationSurfaceFixtureRoundTrips() throws {
        let data = try loadFixtureData("invocation_surface.json")
        let surface = try JSONDecoder().decode(InvocationSurfaceRecord.self, from: data)

        #expect(surface.id == "invocation_00000000000000000000000000000001")
        #expect(surface.work.kind == .task)
        #expect(surface.status == .active)
        #expect(surface.run.containment == .tmux(name: "lf-task"))
        #expect(surface.run.cwd == "/src/loopflow.task")
        #expect(surface.argv == ["tmux", "attach-session", "-t", "lf-task"])

        let encoded = try JSONEncoder().encode(surface)
        let decoded = try JSONDecoder().decode(InvocationSurfaceRecord.self, from: encoded)
        #expect(decoded == surface)
    }

    @Test("Turn and Ask fixtures preserve the targeted exchange")
    func askExchangeFixturesRoundTrip() throws {
        let turn = try JSONDecoder().decode(
            TurnRecord.self,
            from: loadFixtureData("turn.json")
        )
        #expect(turn.state == .active)
        #expect(turn.basis.revision == 4)

        let ask = try JSONDecoder().decode(
            AskExchangeRecord.self,
            from: loadFixtureData("ask_exchange.json")
        )
        #expect(ask.turnId == turn.id)
        #expect(ask.route == .parent(WorkReference(
            kind: .project,
            id: "proj_00000000000000000000000000000001"
        )))
        #expect(ask.answer?.author == .run(id: "run_00000000000000000000000000000001"))
        #expect(ask.answer?.text == "The live blocking exchange.")

        let encoded = try JSONEncoder().encode(ask)
        let decoded = try JSONDecoder().decode(AskExchangeRecord.self, from: encoded)
        #expect(decoded == ask)
    }

    @Test("Work status fixture preserves every status and wait kind")
    func workStatusFixtureRoundTrips() throws {
        let data = try loadFixtureData("work_statuses.json")
        let statuses = try JSONDecoder().decode([WorkStatus].self, from: data)
        func wait(at index: Int) -> WorkWait? {
            guard case .waiting(let wait) = statuses[index] else { return nil }
            return wait
        }

        #expect(statuses[0] == .ready)
        #expect(statuses[1] == .running(runID: "run_00000000000000000000000000000001"))
        #expect(try #require(wait(at: 2)).on == .input(after: WorkBasis(
            epochID: "epoch_00000000000000000000000000000004",
            revision: 7
        )))
        #expect(try #require(wait(at: 3)).on == .time(notBefore: "2026-07-17T13:00:00Z"))
        #expect(try #require(wait(at: 4)).on == .event(WorkEventReference(
            source: "github",
            id: "check-42"
        )))
        #expect(try #require(wait(at: 5)).on == .child(WorkReference(
            kind: .task,
            id: "task_0000000000000000000000000000000e"
        )))
        #expect(try #require(wait(at: 6)).on == .capability(WorkCapabilityReference(
            kind: "deploy",
            key: "production"
        )))
        let effectWait = try #require(wait(at: 7))
        #expect(effectWait.on == .effect(WorkEffectReference(
            kind: "message",
            idempotencyKey: "release-ready"
        )))
        #expect(effectWait.resolvedAt == "2026-07-17T12:06:00Z")
        #expect(statuses[8] == .done)
        #expect(statuses[9] == .abandoned)

        let encoded = try JSONEncoder().encode(statuses)
        let decoded = try JSONDecoder().decode([WorkStatus].self, from: encoded)
        #expect(decoded == statuses)
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
