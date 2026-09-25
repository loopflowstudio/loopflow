import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("Active Runs preserve exact attribution, waiting clients, and evidence gaps")
    func activeRunsFixture() async throws {
        let data = try loadFixtureData("active_runs.json")
        let snapshot = try JSONDecoder().decode(ActiveRunsSnapshot.self, from: data)
        #expect(snapshot.discovery == .ready)
        #expect(snapshot.runs[0].work == snapshot.task)
        #expect(snapshot.runs[0].processes[0].state == .waiting)
        #expect(snapshot.gaps.count == 1)
        #expect(try JSONDecoder().decode(ActiveRunsSnapshot.self, from: JSONEncoder().encode(snapshot)) == snapshot)
        var missing = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        missing.removeValue(forKey: "discovery")
        let incomplete = try JSONSerialization.data(withJSONObject: missing)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(ActiveRunsSnapshot.self, from: incomplete)
        }
    }

    @Test("Wave plan uses the same chapter as status")
    func planUsesChapterSnapshot() async throws {
        let json = String(decoding: try loadFixtureData("wave_detail.json"), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            #expect(args == ["status", "infrastructure", "--json"])
            return json
        }
        let plan = try await query.plan(wave: "infrastructure", objective: "Make releases boring.", cwd: "/fixture")
        #expect(plan.chapter?.flows.recommended == "task-design")
        #expect(plan.chapter?.krs.count == 1)
    }

    @Test("Chapter history retains dated evidence for a moved Task")
    func chapterHistoryRetainsBoundaryEvidence() throws {
        let snapshot = try JSONDecoder().decode(ChapterSnapshot.self, from: loadFixtureData("chapter_snapshot.json"))
        let history = try JSONDecoder().decode([ChapterHistoryEntry].self, from: loadFixtureData("chapter_history.json"))
        #expect(snapshot.closedAt == snapshot.observedAt)
        #expect(snapshot.tasks[0].disposition == "move")
        #expect(!snapshot.tasks[0].task.completed)
        #expect(history[0].id == snapshot.id)
        #expect(history[1].closedAt == nil)
    }

    @Test("Activity fixture preserves exact OS-live process state")
    func activityFixtureRoundTrips() throws {
        let data = try loadFixtureData("activity_snapshot.json")
        let snapshot = try JSONDecoder().decode(ActivitySnapshot.self, from: data)

        #expect(snapshot.schemaVersion == 1)
        #expect(snapshot.nodes.filter { $0.kind == .providerProcess }.map(\.state)
            == [.working, .stalled])
        #expect(snapshot.nodes.filter { $0.kind == .providerProcess }.map(\.wave)
            == ["product", "product"])
        #expect(snapshot.nodes.filter { $0.kind == .providerProcess }.map(\.worktree)
            == ["/src/loopflow.task", "/src/loopflow.task"])
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
        if case .runFinished(let identity, let status) = snapshot.items[0].fact {
            #expect(identity.primaryId == "run_00000000000000000000000000000001")
            #expect(status == "ok")
        } else {
            Issue.record("expected a typed Run finish")
        }
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
        #expect(detail.chapter?.flows.recommended == "task-design")
        #expect(detail.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(detail.unavailableTasks[0].status == .ready)
        #expect(detail.unavailableTasks[0].owner == .wave)
        #expect(detail.tasks.items.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.tasks.items[0].prs.compactMap(\.publication?.github?.number) == [912])
        #expect(detail.tasks.items[0].activePr == "pr_33333333333333333333333333333333")
        #expect(detail.tasks.items[0].prs[0].publication?.merge?.afterMerge == .completeTask)
        #expect(detail.tasks.items[0].directive?.version == 2)
        #expect(detail.tasks.items[0].directive?.incorporatedAt != nil)
        #expect(detail.tasks.items[0].reference.workspace?.slug == "infrastructure-task")
        #expect(detail.tasks.items[0].reference.workspace?.worktree == "/src/loopflow.infrastructure.task")
        #expect(detail.tasks.items[0].reference.issueUrl?.host == "linear.app")
        // Ready is durable Work status. Historical failure evidence stays
        // visible without replacing that present-tense state.
        #expect(detail.tasks.items[0].runtime?.status == .ready)
        #expect(detail.tasks.items[0].runtime?.reason == "ready")
        #expect(detail.tasks.items[1].runtime == nil)
        #expect(detail.tasks.items[1].reference.issueUrl == nil)
        #expect(detail.tasks.items[1].reference.workspace == nil)
        #expect(detail.runs.items[0].id == "run_00000000000000000000000000000001")
        #expect(detail.runs.items[0].skill == "task/pursue")
        #expect(detail.runs.items[0].usage.inputTokens == 12000)
        #expect(detail.runs.items[0].outcome == "completed")
        #expect(detail.tasks.items[0].condition.state == .waiting)
        #expect(detail.tasks.items[0].condition.reason == "merge pull request head 333333333333 on GitHub")
        #expect(detail.tasks.items[0].actions.recommended == .openPr)
        #expect(detail.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")
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
        #expect(product.chapter?.flows.recommended == nil)
        #expect(product.wave.paused)
        #expect(product.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")
        #expect(product.wave.enabled)
        #expect(product.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(product.unavailableTasks[0].status == .ready)
        #expect(product.unavailableTasks[0].recovery.contains("lf work abandon task task_40fbeea"))
        let tasks = product.tasks.items
        #expect(tasks.map(\.section) == [.now, .waiting, .available, .later])
        #expect(tasks.map(\.condition.state) == [.clear, .waiting, .clear, .clear])
        #expect(tasks[0].reference.workspace?.slug == "make-lf-work-the-machine")
        #expect(tasks[2].reference.workspace == nil)
        #expect(tasks[2].reference.issueUrl == nil)
        #expect(tasks[3].reference.workspace?.branch == "jack-heart/now-available-research")
        #expect(roadmap.waves[1].tasks.unavailableReason?.contains("lf pm sync") == true)
        #expect(!roadmap.waves[1].wave.enabled)
    }

    @Test("metric portfolio fixture preserves every closed evidence payload")
    func metricPortfolioFixturePreservesEvidence() throws {
        let data = try loadFixtureData("metric_portfolio.json")
        let portfolio = try JSONDecoder().decode(MetricPortfolio.self, from: data)

        #expect(portfolio.metrics.count == 10)
        #expect(portfolio.contractIssues.count == 5)
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
    @Test("Sessions fixture preserves the unresolved Session projection")
    func sessionsFixtureRoundTrips() throws {
        let sessions = try JSONDecoder().decode(
            [SessionRecord].self,
            from: loadFixtureData("sessions.json")
        )
        let session = try #require(sessions.first)
        #expect(session.work == .task(
            id: "task_00000000000000000000000000000001"
        ))
        #expect(session.title == "Simplify cross-Work questions")
        #expect(session.titleSource == .human)
        #expect(session.detail == "review-design")
        #expect(session.state == .active)
        #expect(session.workPath == "product / LOO-291")
        #expect(session.action(.approve)?.unavailableReason == "The session agent has not marked this ready")

        let encoded = try JSONEncoder().encode(sessions)
        let decoded = try JSONDecoder().decode([SessionRecord].self, from: encoded)
        #expect(decoded == sessions)
    }

    @Test("Session Flow membership mirrors every Rust projection and is required")
    func sessionFlowMembershipFixture() throws {
        let data = try loadFixtureData("session_memberships.json")
        let sessions = try JSONDecoder().decode([SessionRecord].self, from: data)
        #expect(sessions.map(\.flowMembership) == [
            .step(flow: "task-design", invocationId: "00000000-0000-0000-0000-00000000f10w",
                  step: "review-design", stepIndex: 1, iteration: 0, current: true),
            .step(flow: "feature", invocationId: "00000000-0000-0000-0000-0000000000f2",
                  step: "implement", stepIndex: 2, iteration: 3, current: false),
            .independent,
            .unknown(reason: "Run run_00000000000000000000000000000004 predates recorded Flow membership"),
            .step(flow: "feature", invocationId: "00000000-0000-0000-0000-0000000000f2",
                  step: "demo", stepIndex: 7, iteration: 0, current: true),
        ])
        #expect(sessions.last?.titleSource == .unavailable)
        #expect(sessions[1].flowMembership.label == "feature / implement · iteration 3 · earlier")
        let objects = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for var value in objects {
            value.removeValue(forKey: "flow_membership")
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            }
        }
        let decoded = try JSONDecoder().decode([SessionRecord].self, from: JSONEncoder().encode(sessions))
        #expect(decoded == sessions)
    }

    @Test("Every Session kind has a required Run reference independent of its boundary ID")
    func sessionRequiresRun() throws {
        let data = try loadFixtureData("session.json")
        for kind in ["interactive", "ask", "flow"] {
            var value = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            value["kind"] = kind
            let session = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            #expect(session.runId == "run_00000000000000000000000000000001")
            #expect(session.runId != session.id)
            var untitled = value
            untitled.removeValue(forKey: "title_source")
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: untitled))
            }
            value.removeValue(forKey: "run_id")
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            }
            value["run_id"] = NSNull()
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            }
        }
    }

    @Test("Flow Session fixture preserves readiness and its open command")
    func flowSessionFixtureRoundTrips() throws {
        let session = try JSONDecoder().decode(
            SessionRecord.self,
            from: loadFixtureData("session.json")
        )

        #expect(session.state == .ready)
        #expect(session.titleSource == .generated)
        #expect(session.actions.map(\.kind) == [.open, .complete])
        #expect(session.actions.allSatisfy { $0.unavailableReason == nil })
        #expect(session.readySummary == "The design now reflects Jack's requested changes.")
        #expect(session.openArgv.suffix(3) == [
            "session", "open", "task_00000000000000000000000000000001:task-design:review_kickoff:0"
        ])

        let encoded = try JSONEncoder().encode(session)
        let decoded = try JSONDecoder().decode(SessionRecord.self, from: encoded)
        #expect(decoded == session)
    }

    @Test("Work status fixture preserves every status")
    func workStatusFixtureRoundTrips() throws {
        let data = try loadFixtureData("work_statuses.json")
        let statuses = try JSONDecoder().decode([WorkStatus].self, from: data)

        #expect(statuses.count == 3)
        #expect(statuses[0] == .ready)
        #expect(statuses[1] == .done)
        #expect(statuses[2] == .abandoned)

        let encoded = try JSONEncoder().encode(statuses)
        let decoded = try JSONDecoder().decode([WorkStatus].self, from: encoded)
        #expect(decoded == statuses)
    }

    private func loadFixture(_ name: String, sourceFile: String = #filePath) throws -> [String: Any] {
        let data = try loadFixtureData(name, sourceFile: sourceFile)
        let json = try JSONSerialization.jsonObject(with: data)
        return try #require(json as? [String: Any])
    }

    @Test("Governance bodies decode without historical Flow coordinates")
    func governanceBody() throws {
        let data = try loadFixtureData("resident_deltas.json")
        let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let deltas = try #require(json["deltas"] as? [[String: Any]])
        var value = try #require(deltas.first?["body"] as? [String: Any])
        let body = try JSONDecoder().decode(BodyProvenance.self, from: JSONSerialization.data(withJSONObject: value))
        #expect(body.invocationId == nil)
        #expect(body.stepIndex == nil)
        #expect(body.flow == nil)
        #expect(body.iteration == nil)
        #expect(body.step == "wave/operate")
        let failed = try ChatTurn(id: "failed", role: .assistant, authorName: nil, text: "", status: .failed,
                              items: [], createdAt: body.startedAt, body: body, activity: nil)
        value["body_id"] = "different-attempt"
        let laterBody = try JSONDecoder().decode(BodyProvenance.self, from: JSONSerialization.data(withJSONObject: value))
        let later = try ChatTurn(id: "later", role: .assistant, authorName: nil, text: "", status: .completed,
                             items: [], createdAt: body.startedAt, body: laterBody, activity: nil)
        let failures = attemptFailurePresentations(turns: [failed, later])
        #expect(failures[failed.id]?.state == .failed)
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
