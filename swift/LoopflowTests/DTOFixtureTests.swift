import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("Task work retains conversations, managed Flows and command history")
    func taskWorkFixture() throws {
        let data = try loadFixtureData("task_work.json")
        let work = try JSONDecoder().decode(TaskWork.self, from: data)
        #expect(work.sessions.count == 2)
        #expect(!work.sessions[0].managed)
        #expect(work.sessions[1].kind == "flow_review")
        #expect(work.flows[0].managed)
        #expect(!work.execs.isEmpty)
        #expect(try JSONDecoder().decode(TaskWork.self, from: JSONEncoder().encode(work)) == work)
    }

    @Test("Task planning retains the provider branch before execution exists")
    func taskPlanningBranchFixture() throws {
        let states = try JSONDecoder().decode([TaskStatus].self, from: loadFixtureData("task_status.json"))
        #expect(states[0].planning?.item.branchName == "dev/fix-1-existing")
        #expect(states[0].execution == nil)
        #expect(states[0].planning?.project?.flow == "feature")
        #expect(states[0].planning?.project?.status == .started)
    }

    @Test("Session page retains complete enumeration and requires entries")
    func sessionPageFixture() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session_page.json")
        let data = try Data(contentsOf: url)
        let page = try JSONDecoder().decode(SessionPage.self, from: data)
        #expect(page.entries.count == 1)
        #expect(page.next == nil)
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(SessionPage.self, from: Data(#"{"next":null}"#.utf8))
        }
    }

    @Test("Exec pages retain command outcomes, caller evidence and continuation")
    func execPageFixture() throws {
        let data = try loadFixtureData("exec_page.json")
        let page = try JSONDecoder().decode(ExecPage.self, from: data)
        #expect(page.entries[0].exitCode == 42)
        #expect(page.entries[0].viaAgent == nil)
        #expect(page.entries[1].parentExecID == page.entries[0].id)
        #expect(page.entries[1].outcome == nil)
        #expect(page.next?.id == page.entries[1].id)
        #expect(try JSONDecoder().decode(ExecPage.self, from: JSONEncoder().encode(page)) == page)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(ExecPage.self, from: Data("{}".utf8))
        }
    }

    @Test("Context report keeps unknown sources distinct from zero")
    func contextReportFixture() throws {
        let report = try JSONDecoder().decode(ContextReport.self, from: loadFixtureData("context_report.json"))
        let steers = report.steps[0].sources[4]
        #expect(steers.source == "steers")
        #expect(steers.count == 384)
        #expect(steers.overBudget)
        #expect(steers.authors == ["user"])
        #expect(report.steps[0].overAssembledBudget)
        #expect(report.steps[1].sources.allSatisfy { $0.tokens == nil })
        #expect(report.steps[1].gaps == ["this input has no retained capture"])
        #expect(report.totals.count == 9)
    }

    @Test("Conversation history retains native evidence and unknown driver")
    func sessionHistoryFixture() throws {
        let data = try loadFixtureData("session_history.json")
        let events = try JSONDecoder().decode([SessionEvent].self, from: data)
        #expect(events[1].kind == .completed)
        #expect(events[1].execID == nil)
        #expect(events[2].kind == .observed)
        #expect(events[2].providerThread == nil)
        #expect(events[2].providerTurn == nil)
        #expect(events[0].payload == .object([
            "total": .object(["inputTokens": .integer(40), "outputTokens": .integer(10)]),
            "last": .object(["inputTokens": .integer(20), "outputTokens": .integer(5)]),
        ]))
        #expect(try JSONDecoder().decode([SessionEvent].self, from: JSONEncoder().encode(events)) == events)
    }

    @Test("Active Sessions preserve exact attribution, waiting clients, and evidence gaps")
    func activeSessionsFixture() async throws {
        let data = try loadFixtureData("active_runs.json")
        let snapshot = try JSONDecoder().decode(ActiveSessionsSnapshot.self, from: data)
        #expect(snapshot.discovery == .ready)
        #expect(snapshot.sessions[0].work == snapshot.task)
        #expect(snapshot.sessions[0].processes[0].state == .waiting)
        #expect(snapshot.gaps.count == 1)
        #expect(try JSONDecoder().decode(ActiveSessionsSnapshot.self, from: JSONEncoder().encode(snapshot)) == snapshot)
        var missing = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        missing.removeValue(forKey: "discovery")
        let incomplete = try JSONSerialization.data(withJSONObject: missing)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(ActiveSessionsSnapshot.self, from: incomplete)
        }
    }

    @Test("Task comments keep the complete thread and require every field")
    func taskCommentsFixture() throws {
        let data = try loadFixtureData("task_comments.json")
        let thread = try JSONDecoder().decode(TaskComments.self, from: data)
        #expect(thread.identifier == "LOO-291")
        #expect(thread.comments.map(\.id) == ["c-bot", "c-1", "c-2"])
        #expect(thread.comments.map(\.author) == [.integration, .person(name: "Maya"), .person(name: nil)])
        #expect(thread.comments[0].createdAt == nil)
        #expect(thread.comments[1].body == "- first\n  - nested\n\n[spec](https://example.com)")
        var root = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var comments = try #require(root["comments"] as? [[String: Any]])
        comments[0].removeValue(forKey: "created_at")
        root["comments"] = comments
        let incomplete = try JSONSerialization.data(withJSONObject: root)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(TaskComments.self, from: incomplete)
        }
    }

    @Test("Wave plan uses the same chapter as status")
    func planUsesChapterSnapshot() async throws {
        let json = String(decoding: try loadFixtureData("wave_detail.json"), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            #expect(args == ["wave", "status", "infrastructure", "--json"])
            return json
        }
        let plan = try await query.plan(wave: "infrastructure", objective: "Make releases boring.", cwd: "/fixture")
        #expect(plan.currentProject?.flow == "task-design")
        #expect(plan.currentProject?.krs.count == 1)
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
        if case .inputCompletionRecorded(_, let captured, let status) = snapshot.items[0].fact {
            #expect(captured == 12)
            #expect(status == "ok")
        } else {
            Issue.record("expected a recorded Session outcome")
        }
        #expect(snapshot.items[1].fact.github?.number == 1144)
        #expect(snapshot.items[1].fact.github?.url.host == "github.com")
        if case .prMergeRequested(_, let request, _) = snapshot.items[2].fact {
            #expect(request.requestedAt == "2026-07-21T18:35:51Z")
        } else {
            Issue.record("expected a typed PR merge request")
        }
        #expect(snapshot.items[3].work.kind == .wave)
        if case .steerIssued(_, .imported(let id)) = snapshot.items[3].fact {
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

        // The Home runtime evidence carries the state and the one contextual action.


        #expect(detail.currentProject?.flow == "task-design")
        #expect(detail.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(detail.unavailableTasks[0].status == .ready)
        #expect(detail.unavailableTasks[0].owner == .wave)
        #expect(detail.tasks.items.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.tasks.items[0].task.state == "unstarted")
        #expect(detail.tasks.items[0].task.completedAt == nil)
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
        #expect(detail.runs.items[0].id == "run_00000000000000000000000000000001:12")
        #expect(detail.runs.items[0].skill == "task/pursue")
        #expect(detail.runs.items[0].taskPrId == "pr_33333333333333333333333333333333")
        #expect(detail.runs.items[0].firstProviderAttemptAt == 1784052010)
        #expect(detail.runs.items[0].usage.inputTokens == 12000)
        #expect(detail.runs.items[0].recordedOutcome == "completed")
        #expect(detail.tasks.items[0].condition.state == .waiting)
        #expect(detail.tasks.items[0].condition.reason == "merge pull request head 333333333333 on GitHub")
        #expect(detail.tasks.items[0].actions.recommended == .openPr)
        #expect(detail.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")
        #expect(detail.metricPortfolio.metrics[0].evidence == .met(
            value: 1,
            sourceWindowStart: "2026-08-13T18:00:00Z",
            sourceWindowEnd: "2026-08-20T18:00:00Z"
        ))

        var missingHome = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var wave = try #require(missingHome["wave"] as? [String: Any])
        wave.removeValue(forKey: "home")
        missingHome["wave"] = wave
        let missingHomeData = try JSONSerialization.data(withJSONObject: missingHome)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(WaveDetailSnapshot.self, from: missingHomeData)
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
        #expect(product.currentProject?.flow == "feature")

        #expect(product.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")

        #expect(product.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(product.unavailableTasks[0].status == .ready)
        #expect(product.unavailableTasks[0].recovery.contains("lf task status task_40fbeea"))
        let tasks = product.tasks.items
        #expect(tasks.map(\.section) == [.now, .waiting, .available, .later])
        #expect(tasks.map(\.condition.state) == [.clear, .waiting, .clear, .clear])
        #expect(tasks[0].reference.workspace?.slug == "make-lf-work-the-machine")
        #expect(tasks[2].reference.workspace == nil)
        #expect(tasks[2].reference.issueUrl == nil)
        #expect(tasks[3].reference.workspace?.branch == "jack-heart/now-available-research")
        // Start evidence is required: a prepared checkout is not started work.
        #expect(tasks.map { $0.runtime?.started } == [false, true, nil, true])
        #expect(roadmap.waves[1].tasks.unavailableReason?.contains("lf repo refresh") == true)

    }

    @Test("metric portfolio fixture preserves every closed evidence payload")
    func metricPortfolioFixturePreservesEvidence() throws {
        let data = try loadFixtureData("metric_portfolio.json")
        let portfolio = try JSONDecoder().decode(MetricPortfolio.self, from: data)

        #expect(portfolio.metrics.count == 11)
        #expect(portfolio.metrics.contains {
            if case .unknown(.targetUnavailable(value: 1.0, sourceWindowStart: _, sourceWindowEnd: _)) = $0.evidence { true } else { false }
        })
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

    @Test("Metadata does not manufacture a live state or current occurrence")
    func sessionMetadataFixture() throws {
        let session = try JSONDecoder().decode(SessionRecord.self, from: loadFixtureData("session_metadata.json"))
        #expect(session.state == .unknown)
        guard case .step(_, _, _, let node, let iterations, let occurrence) = session.flowMembership else {
            Issue.record("Retain the known Flow membership")
            return
        }
        #expect(node == nil)
        #expect(iterations == nil)
        #expect(occurrence == .unknown)
        #expect(session.flowMembership.label.contains("position unavailable"))
        #expect(session.terminalIds.isEmpty)
        #expect(try JSONDecoder().decode(SessionRecord.self, from: JSONEncoder().encode(session)) == session)
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
        #expect(session.action(.complete)?.unavailableReason == "The session agent has not marked this ready")

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
                  step: "review-design", node: 1, iterations: [[]], occurrence: .current),
            .step(flow: "feature", invocationId: "00000000-0000-0000-0000-0000000000f2",
                  step: "implement", node: 6, iterations: [[2, 1], [1]], occurrence: .earlier),
            .independent,
            .unknown(reason: "Run run_00000000000000000000000000000004 predates recorded Flow membership"),
            .step(flow: "feature", invocationId: "00000000-0000-0000-0000-0000000000f1",
                  step: "implement", node: 2, iterations: [[2, 1]], occurrence: .past),
            .step(flow: "feature", invocationId: "00000000-0000-0000-0000-0000000000f0",
                  step: "implement", node: nil, iterations: nil, occurrence: .past),
            .step(flow: "feature", invocationId: "00000000-0000-0000-0000-0000000000f2",
                  step: "demo", node: 9, iterations: [[0, 0]], occurrence: .current),
        ])
        let graphs = try JSONDecoder().decode([String: FlowGraph].self,
            from: loadFixtureData("session_membership_graphs.json"))
        for session in sessions {
            if case let .step(_, invocation, step, node?, _, _) = session.flowMembership {
                let graph = try #require(graphs[invocation])
                #expect(graph.node(node)?.label == step)
            }
        }
        #expect(sessions.last?.titleSource == .unavailable)
        #expect(sessions.map(\.provider) == ["codex", "claude", "claude", "claude", "claude", "claude", nil])
        #expect(sessions[1].flowMembership.label == "feature / implement · iteration (2, 1) / (1) · earlier")
        #expect(sessions[4].flowMembership.label == "feature / implement · iteration (2, 1) · past run")
        #expect(sessions[5].flowMembership.label == "feature / implement · iteration unavailable · past run")
        let legacy: SessionFlowMembership = .step(flow: "feature", invocationId: "old", step: "demo",
                                                 node: nil, iterations: nil, occurrence: .past)
        #expect(legacy.label == "feature / demo · iteration unavailable · past run")
        #expect(try JSONDecoder().decode(SessionFlowMembership.self,
                                       from: JSONEncoder().encode(legacy)) == legacy)
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

    @Test("Every Session kind retains its identity without a Run field")
    func sessionIdentityHasNoRun() throws {
        let data = try loadFixtureData("session.json")
        for kind in ["conversation", "ask", "flow"] {
            var value = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            value["kind"] = kind
            let session = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            let encoded = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(session)) as? [String: Any])
            #expect(encoded["run_id"] == nil)
            #expect(encoded["id"] as? String == session.id)
            value.removeValue(forKey: "title_source")
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
            "session", "connect", "task_00000000000000000000000000000001:task-design:review_kickoff:0"
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

@Test("Session input history retains distinct native results and unknown Exec")
func sessionInputHistoryFixture() throws {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/session_history_summary.json")
    let value = try JSONDecoder().decode(SessionHistory.self, from: Data(contentsOf: url))
    #expect(value.providers.count == 2)
    #expect(value.providers[0].outcome == "failed")
    #expect(value.providers[0].execId == nil)
    #expect(value.providers[0].usage.inputTokens == nil)
    #expect(value.providers[1].outcome == "completed")
    #expect(value.providers[1].usage.inputTokens == 0)
    #expect(value.status == "failed → completed")
}
