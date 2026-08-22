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

    @Test("Activity fixture preserves process state and exact output evidence")
    func activityFixtureRoundTrips() throws {
        let data = try loadFixtureData("activity_snapshot.json")
        let snapshot = try JSONDecoder().decode(ActivitySnapshot.self, from: data)

        #expect(snapshot.schemaVersion == 1)
        #expect(snapshot.usage.windows == [5, 300, 3_600, 86_400])
        #expect(snapshot.usage.global?.interval(seconds: 86_400)?.outputTokens == 48_200)
        #expect(snapshot.usage.global?.interval(seconds: 5)?.outputTokensPerSecond == 4.0)
        #expect(snapshot.usage.global?.interval(seconds: 300)?.inputTokens == 100)
        #expect(snapshot.usage.global?.interval(seconds: 300)?.cacheReadTokens == 350)
        #expect(snapshot.usage.global?.interval(seconds: 300)?.peakInputTokens == 120_000)
        #expect(snapshot.usage.global?.interval(seconds: 300)?.costUsd == 0.2)
        #expect(snapshot.usage.globalHistory.reduce(0) { $0 + $1.outputTokens } == 80)
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
        #expect(detail.wave.enabled)
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
        // Ready is durable current intent. Historical failure evidence stays
        // visible without replacing that present-tense state.
        #expect(detail.projects[0].tasks[0].runtime?.current.state == .ready)
        #expect(detail.projects[0].tasks[0].runtime?.current.owner == .loopflow)
        #expect(detail.projects[0].tasks[0].runtime?.current.reason == "ready")
        #expect(detail.projects[0].runtime?.current.state == .ready)
        #expect(detail.projects[0].runtime?.current.reason == "ready")
        #expect(detail.projects[0].runtime?.lastFailure?.message.contains("credential") == true)
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
        #expect(detail.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")
        #expect(detail.metricPortfolio.metrics[0].projectId == detail.projects[0].project.id)
        #expect(detail.metricPortfolio.metrics[0].evidence == .met(
            value: 1,
            sourceWindowStart: "2026-08-13T18:00:00Z",
            sourceWindowEnd: "2026-08-20T18:00:00Z"
        ))

        var legacy = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var legacyWave = try #require(legacy["wave"] as? [String: Any])
        legacyWave.removeValue(forKey: "paused")
        legacy["wave"] = legacyWave
        let legacyData = try JSONSerialization.data(withJSONObject: legacy)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(WaveDetailSnapshot.self, from: legacyData)
        }

        var missingEnabled = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var waveWithoutEnabled = try #require(missingEnabled["wave"] as? [String: Any])
        waveWithoutEnabled.removeValue(forKey: "enabled")
        missingEnabled["wave"] = waveWithoutEnabled
        let missingEnabledData = try JSONSerialization.data(withJSONObject: missingEnabled)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(WaveDetailSnapshot.self, from: missingEnabledData)
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
        #expect(product.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")
        #expect(product.metricPortfolio.metrics[0].projectId == product.projects.items[0].project.id)
        #expect(product.wave.enabled)
        #expect(product.unavailableProjects[0].workId == "proj_e972b70272fbb5e91c096ebe657f9f9b")
        #expect(product.unavailableProjects[0].projectSlug == "technical-architecture")
        #expect(product.unavailableProjects[0].current.state == .abandoned)
        #expect(product.unavailableProjects[0].tasks[0].taskIdentifier == "W2-127")
        #expect(product.unavailableProjects[0].tasks[0].current.state == .ready)
        #expect(product.unavailableProjects[0].tasks[0].recovery.contains("lf work abandon task task_40fbeea"))
        let project = try #require(product.projects.items.first)
        #expect(project.tasks.map(\.section) == [.now, .needsAttention, .available, .later])
        #expect(project.tasks.map(\.attention.level) == [.green, .red, .black, .black])
        #expect(project.tasks[0].reference.workspace?.slug == "make-lf-work-the-machine")
        #expect(project.tasks[2].reference.workspace == nil)
        #expect(project.tasks[2].reference.issueUrl == nil)
        #expect(project.tasks[3].reference.workspace?.branch == "jack-heart/now-available-research")
        #expect(roadmap.waves[1].projects.unavailableReason?.contains("lf pm sync") == true)
        #expect(!roadmap.waves[1].wave.enabled)
    }

    @Test("metric portfolio fixture preserves every closed evidence payload")
    func metricPortfolioFixturePreservesEvidence() throws {
        let data = try loadFixtureData("metric_portfolio.json")
        let portfolio = try JSONDecoder().decode(MetricPortfolio.self, from: data)

        #expect(portfolio.metrics.count == 9)
        #expect(portfolio.contractIssues.count == 4)
        #expect(
            portfolio.metrics[0].description
                == "Fraction of qualifying events that settled successfully."
        )
        #expect(portfolio.metrics.map(\.stage).contains(.graduated))
        #expect(portfolio.metrics.map(\.stage).contains(.installed))
        #expect(portfolio.metrics.contains { if case .atMost = $0.target { true } else { false } })
        #expect(portfolio.metrics.contains { if case .never = $0.freshness { true } else { false } })
        #expect(portfolio.metrics.contains { if case .stale = $0.freshness { true } else { false } })
        #expect(portfolio.metrics.contains { if case .unavailable = $0.evidence { true } else { false } })
        #expect(portfolio.metrics.contains {
            if case .unknown(.revisionMismatch) = $0.evidence { true } else { false }
        })
        #expect(portfolio.metrics.contains {
            if case .unknown(.staleUnavailable) = $0.evidence { true } else { false }
        })

        var missing = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var metrics = try #require(missing["metrics"] as? [[String: Any]])
        metrics[0].removeValue(forKey: "description")
        missing["metrics"] = metrics
        let missingData = try JSONSerialization.data(withJSONObject: missing)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(MetricPortfolio.self, from: missingData)
        }

        var future = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var futureMetrics = try #require(future["metrics"] as? [[String: Any]])
        futureMetrics[0]["future_field"] = true
        future["metrics"] = futureMetrics
        let futureData = try JSONSerialization.data(withJSONObject: future)
        _ = try JSONDecoder().decode(MetricPortfolio.self, from: futureData)
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
        #expect(surface.status == .live)
        #expect(surface.current.liveness?.state == .present)
        #expect(surface.run.runtimeGeneration == 8)
        #expect(surface.run.containment == .tmux(name: "lf-task"))
        #expect(
            surface.run.trigger
                == .homeUpgrade(
                    upgradeId: "upgrade_00000000000000000000000000000007",
                    priorRunId: "run_00000000000000000000000000000008"
                )
        )
        #expect(surface.run.cwd == "/src/loopflow.task")
        #expect(surface.argv == ["tmux", "attach-session", "-t", "lf-task"])

        let encoded = try JSONEncoder().encode(surface)
        let decoded = try JSONDecoder().decode(InvocationSurfaceRecord.self, from: encoded)
        #expect(decoded == surface)
    }

    @Test("User Ask attention fixture preserves the durable queue projection")
    func askAttentionFixtureRoundTrips() throws {
        let attention = try JSONDecoder().decode(
            [AskAttentionRecord].self,
            from: loadFixtureData("ask_attention.json")
        )
        let ask = try #require(attention.first)
        #expect(ask.attention == .queued)
        #expect(ask.ask.origin.work == .task(
            id: "task_00000000000000000000000000000001"
        ))
        #expect(ask.ask.request == .intervention(prompt: "Connect Linear for this worktree"))
        #expect(ask.surface == nil)

        let encoded = try JSONEncoder().encode(attention)
        let decoded = try JSONDecoder().decode([AskAttentionRecord].self, from: encoded)
        #expect(decoded == attention)
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
