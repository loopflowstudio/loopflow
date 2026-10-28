import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test func pausedTaskStateIsUnfinished() throws {
        let state = try JSONDecoder().decode(TaskState.self, from: loadFixtureData("task_deferred.json"))
        #expect(state == .deferred)
        #expect(!state.isTerminal)
        #expect(try JSONDecoder().decode(TaskState.self, from: JSONEncoder().encode(state)) == state)
    }

    @Test func taskFilesFramesKeepInvalidationSeparateFromHeartbeat() throws {
        let data = try loadFixtureData("task_files_frames.json")
        let frames = try JSONDecoder().decode([TaskFilesFrame].self, from: data)
        #expect(frames.map(\.paths) == [["note.txt", ".git/index", ""], []])
        #expect(frames[0].request == frames[1].request)
        #expect(frames[0].checkout == "/repo/task")
        #expect(try JSONDecoder().decode([TaskFilesFrame].self, from: JSONEncoder().encode(frames)) == frames)
    }

    @Test func taskLocationsRetainFreshnessAndUnavailableEvidence() throws {
        let data = try loadFixtureData("task_locations.json")
        let readings = try JSONDecoder().decode([TaskLocationObservation].self, from: data)
        #expect(readings[0].location == .recorded(taskID: readings[0].taskID, checkout: "/repo/task"))
        #expect(readings[1].location == .recorded(taskID: readings[1].taskID, checkout: nil))
        #expect(readings[2].location == .unrecorded)
        #expect(readings[3].location == .unavailable(reason: "peer unreachable"))
        #expect(try JSONDecoder().decode([TaskLocationObservation].self, from: JSONEncoder().encode(readings)) == readings)
    }

    @Test func repositoryIdentityPreservesExplicitLocalLocators() throws {
        let data = try loadFixtureData("repository_identity.json")
        let identity = try JSONDecoder().decode(RepositoryIdentity.self, from: data)
        #expect(identity.locators.count == 2)
        #expect(identity.id == identity.locators[1])
        #expect(try JSONDecoder().decode(RepositoryIdentity.self, from: JSONEncoder().encode(identity)) == identity)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire.removeValue(forKey: "locators")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(RepositoryIdentity.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func sessionConnectExplanationsPreserveIntentAndUnavailableEvidence() throws {
        let data = try loadFixtureData("session_connect_explanations.json")
        let reports = try JSONDecoder().decode([SessionConnectExplanation].self, from: data)
        #expect(reports[0].action?.intent == .start)
        #expect(reports[0].action?.mode == .refuse)
        #expect(reports[1].action?.mode == .takeover)
        #expect(reports[1].action?.prepareOnly == true)
        #expect(reports[2].action?.intent == .connectOrResume)
        #expect(reports[2].action?.mode == .try)
        #expect(!reports[2].unavailable.isEmpty)
        #expect(reports[3].state == nil)
        #expect(reports[3].action == nil)
        #expect(reports[3].actions.isEmpty)
        #expect(try JSONDecoder().decode([SessionConnectExplanation].self, from: JSONEncoder().encode(reports)) == reports)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        wire[0].removeValue(forKey: "actions")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode([SessionConnectExplanation].self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func workLinksCarryReadableLabelsRoutesAndConversationRelation() throws {
        let data = try loadFixtureData("work_links.json")
        let links = try JSONDecoder().decode([WorkLink].self, from: data)
        #expect(links[0].conversation == .otherConversation)
        #expect(links[0].target.executionMachine != nil)
        #expect(links[1].target.kind == .wave)
        #expect(links[1].route.hosts == [.terminal])
        #expect(try JSONDecoder().decode([WorkLink].self, from: JSONEncoder().encode(links)) == links)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        wire[0].removeValue(forKey: "fallback_argv")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode([WorkLink].self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func taskPlanningExplanationsPreserveIntentAndEffects() throws {
        let data = try loadFixtureData("task_planning_explanations.json")
        let reports = try JSONDecoder().decode([TaskPlanningExplanation].self, from: data)
        #expect(reports[0].action == .create(title: "Keep drafts", description: "Retain input", project: "project-fixture"))
        #expect(reports[1].action == .edit(revision: 3, fields: ["title", "assignee"]))
        #expect(reports[2].action == .comment(message: "Keep the draft", steer: true, refresh: false))
        #expect(reports[3].action == .comment(message: nil, steer: false, refresh: true))
        #expect(reports[4].action == nil)
        #expect(!reports[4].impediments.isEmpty)
        #expect(reports[5].action == .move(wave: "destination-wave", project: "destination-project", previousProject: "source-project"))
        #expect(reports[6].action == .save(path: "scratch/note.md", revision: "observed-file-revision", draftBytes: 42))
        #expect(try JSONDecoder().decode([TaskPlanningExplanation].self, from: JSONEncoder().encode(reports)) == reports)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        wire[0].removeValue(forKey: "effects")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode([TaskPlanningExplanation].self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func taskCheckoutExplanationsKeepProposalsSeparateFromAdmission() throws {
        let data = try loadFixtureData("task_checkout_explanations.json")
        let reports = try JSONDecoder().decode([TaskCheckoutExplanation].self, from: data)
        #expect(reports[0].action?.behavior == .prepare)
        #expect(reports[1].action?.behavior == .reuse)
        #expect(reports[2].action?.behavior == .restore)
        #expect(reports[3].action == nil)
        #expect(!reports[0].unavailable.isEmpty)
        #expect(try JSONDecoder().decode([TaskCheckoutExplanation].self, from: JSONEncoder().encode(reports)) == reports)
    }

    @Test func taskRunExplanationsPreserveActionsAndMissingEvidence() throws {
        let data = try loadFixtureData("task_run_explanations.json")
        let reports = try JSONDecoder().decode([TaskRunExplanation].self, from: data)
        #expect(reports[0].action == .run(graph: "feature", takeUp: true, from: "start", stopsAt: "explore"))
        #expect(reports[1].action == .wait(graph: "feature", takeUp: false, step: "demo"))
        #expect(reports[2].action == .end(graph: "proof", takeUp: false))
        #expect(reports[3].action == nil)
        #expect(try JSONDecoder().decode([TaskRunExplanation].self, from: JSONEncoder().encode(reports)) == reports)
        // A run to the end says so with an explicit null.
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        var action = try #require(wire[0]["action"] as? [String: Any])
        action.removeValue(forKey: "stops_at")
        wire[0]["action"] = action
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode([TaskRunExplanation].self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func contextExplanationPreservesAvailabilityAndProvenance() throws {
        let data = try loadFixtureData("context_explanation.json")
        let report = try JSONDecoder().decode(ContextExplanation.self, from: data)
        #expect(report.checkout == .unbound)
        #expect(report.executionMachine == .unavailable(reason: "No local execution location; peer execution has not been observed"))
        #expect(report.planningObservedAt == nil)
        #expect(try JSONDecoder().decode(ContextExplanation.self, from: JSONEncoder().encode(report)) == report)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire.removeValue(forKey: "task")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(ContextExplanation.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func desktopOpeningDoesNotConfuseLaunchWithUsability() throws {
        let data = try loadFixtureData("desktop_openings.json")
        let readings = try JSONDecoder().decode([DesktopOpening].self, from: data)
        #expect(readings.map(\.status) == [.opening, .usable, .failed])
        #expect(readings[0].reason == nil)
        #expect(readings[2].reason != nil)
        #expect(try JSONDecoder().decode([DesktopOpening].self, from: JSONEncoder().encode(readings)) == readings)
    }

    @Test func desktopTextDistinguishesEmptyAndUnavailable() throws {
        let data = try loadFixtureData("desktop_text_readings.json")
        let readings = try JSONDecoder().decode([DesktopTextReading].self, from: data)
        #expect(readings[0].result == .available(text: "", truncated: false))
        #expect(readings[1].result == .available(text: "héλ🙂", truncated: true))
        #expect(readings[2].result == .unavailable(reason: .missingSurface))
        let original = try #require(JSONSerialization.jsonObject(with: data) as? NSArray)
        let encoded = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(readings)) as? NSArray)
        #expect(original == encoded)
        for reading in readings {
            let wire = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(reading)) as? [String: Any])
            for owner in ["request", "result"] {
                let payload = try #require(wire[owner] as? [String: Any])
                for field in payload.keys {
                    var missing = wire, value = payload
                    value.removeValue(forKey: field)
                    missing[owner] = value
                    #expect(throws: DecodingError.self) {
                        try JSONDecoder().decode(DesktopTextReading.self, from: JSONSerialization.data(withJSONObject: missing))
                    }
                }
            }
        }
    }

    @Test func desktopActionRequestsAndReceiptRoundTrip() throws {
        let data = try loadFixtureData("desktop_action.json")
        let wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let requestsData = try JSONSerialization.data(withJSONObject: try #require(wire["requests"]))
        let requests = try JSONDecoder().decode([DesktopActionRequest].self, from: requestsData)
        #expect(requests[0].action == "goto")
        #expect(requests[0].pane == nil)
        #expect(requests[2].pane?.tab == "shell-tab")
        let encoded = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(requests)) as? NSArray)
        #expect(encoded == (wire["requests"] as? NSArray))
        let request = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(requests[2])) as? [String: Any])
        for field in ["repository", "window", "action", "args"] {
            var missing = request
            missing.removeValue(forKey: field)
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(DesktopActionRequest.self, from: JSONSerialization.data(withJSONObject: missing))
            }
        }
        let receiptData = try JSONSerialization.data(withJSONObject: try #require(wire["receipt"]))
        let receipt = try JSONDecoder().decode(DesktopActionReceipt.self, from: receiptData)
        #expect(receipt.workspace?.layout.children.last?.children.last?.tabs.last?.url == "http://localhost:5173")
        #expect(receipt.appliesTo == .window)
        #expect(try JSONDecoder().decode(DesktopActionReceipt.self, from: JSONEncoder().encode(receipt)) == receipt)
        var missingScope = try #require(wire["receipt"] as? [String: Any])
        missingScope.removeValue(forKey: "applies_to")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(DesktopActionReceipt.self, from: JSONSerialization.data(withJSONObject: missingScope))
        }
    }

    @Test func repoPeopleCarryEmailsAndOptionalGitHubUsers() throws {
        let people = try JSONDecoder().decode([RepoPerson].self, from: loadFixtureData("repo_people.json"))
        #expect(people.map(\.email) == ["maya@example.com", "sam@example.com", "unknown@example.com"])
        #expect(people[0].github?.name == "Maya Lin")
        #expect(people[1].github?.name == nil && people[1].name == nil)
        #expect(people[2].github == nil)
        let encoded = try JSONEncoder().encode(people)
        #expect(try JSONDecoder().decode([RepoPerson].self, from: encoded) == people)
    }

    @Test func desktopInspectionRetainsUnavailableWorkAndExactPanes() throws {
        let data = try loadFixtureData("desktop_inspection.json")
        let report = try JSONDecoder().decode(DesktopInspection.self, from: data)
        #expect(report.openings.first?.status == .failed)
        #expect(report.openings.first?.reason == "Repository path is unavailable")
        var missingOpenings = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        missingOpenings.removeValue(forKey: "openings")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(DesktopInspection.self, from: JSONSerialization.data(withJSONObject: missingOpenings))
        }
        let window = try #require(report.windows.first)
        #expect(window.destination == .entered(from: .dashboard("attention"), work: try #require(window.workspaces.first).work))
        #expect(window.history == DesktopHistory(back: 1, forward: 0))
        #expect(report.windows[1].destination == .workingTasks)
        #expect(window.reading == "unavailable")
        #expect(window.openWork == [try #require(window.workspaces.first).work])
        #expect(window.task?.actions == nil)
        #expect(window.task?.roadmapGeneratedAt == "2026-10-08T19:00:00Z")
        #expect(window.task?.conditionObservedAt == "2026-10-08T18:59:58Z")
        #expect(window.session?.id == "session-one")
        #expect(window.session?.actions?.map(\.kind) == [.open, .moveHere])
        #expect(window.session?.actions?.first?.unavailableReason != nil)
        #expect(window.session?.observedAt == nil)
        #expect(report.windows[1].task?.actions?.recommended == .resume)
        #expect(report.windows[1].task?.runControl?.unavailable == "Task checkout is on another Machine")
        #expect(window.workspaces.first?.hiddenPanes == ["files-pane"])
        #expect(window.viewParts.isEmpty)
        #expect(report.windows[1].viewParts.map(\.name) == ["board", "session", "tiles"])
        #expect(report.windows[1].viewParts[0].doneDays == 7)
        #expect(report.windows[1].viewParts[0].section == "backlog")
        #expect(report.windows[1].viewParts[2].ratio == 0.382)
        #expect(report.windows[1].viewParts.map(\.zoomed) == [false, false, true])
        #expect(report.windows[1].viewParts[2].page == 2 && report.windows[1].viewParts[2].pages == 3)
        #expect(report.windows[1].viewParts.map(\.hidden) == [false, true, false])
        #expect(window.workspaces.first?.layout.children.first?.subject == "session-one")
        #expect(window.workspaces.first?.layout.children.first?.surface == "native-surface-incarnation")
        #expect(window.workspaces.first?.layout.children.last?.surface == nil)
        #expect(window.list.first?.identifier == "LOO-519")
        #expect(window.actions.map(\.name) == ["back", "forward"])
        #expect(try JSONDecoder().decode(DesktopInspection.self, from: JSONEncoder().encode(report)) == report)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var windows = try #require(wire["windows"] as? [[String: Any]])
        windows[0].removeValue(forKey: "reading")
        wire["windows"] = windows
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(DesktopInspection.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test("Direction preserves named and historical authors")
    func workAuthors() throws {
        let authors = try JSONDecoder().decode([WorkAuthor].self, from: loadFixtureData("work_authors.json"))
        #expect(authors == [.user, .named(id: "Fixture author"), .captured(id: 3), .imported(id: "historical")])
        #expect(try JSONDecoder().decode([WorkAuthor].self, from: JSONEncoder().encode(authors)) == authors)
    }

    @Test("Activity retains unknown record identity without a PID")
    func activityFixture() throws {
        let snapshot = try JSONDecoder().decode(ActivitySnapshot.self, from: loadFixtureData("activity_snapshot.json"))
        let unknown = snapshot.nodes[2]
        #expect(unknown.state == .unknown)
        #expect(unknown.pid == nil)
        #expect(unknown.parentId == snapshot.nodes[0].id)
        #expect(unknown.id == "33333333-3333-4333-8333-333333333333")
        #expect(try JSONDecoder().decode(ActivitySnapshot.self, from: JSONEncoder().encode(snapshot)) == snapshot)
    }

    @Test("Composed delivery keeps completion independent of the running Graph and its arrival")
    func composedTaskLifecycle() throws {
        let snapshots = try TaskLifecycleCapture.load(loadFixtureData("task_lifecycle.json"))
        let merged = try #require(snapshots["merged"])
        let completed = try #require(snapshots["completed"])
        let arrived = try #require(snapshots["arrived"])
        #expect(merged.row.runtime?.status != .done)
        #expect(completed.row.runtime?.status == .done)
        #expect(arrived.row.runtime?.status == .done)
        let before = try merged.taskPart()
        let during = try completed.taskPart()
        let after = try arrived.taskPart()
        #expect(before.work.graph == during.work.graph)
        guard case .run(_, _, true) = during.work.graph?.position else {
            Issue.record("Completed Task lost its live run")
            return
        }
        #expect(after.work.graph?.position == .at(step: "end"))
        #expect(before.graphProcesses.count == 1 && after.graphProcesses.count == 1)
        #expect(before.graphProcesses[0].entry.id == after.graphProcesses[0].entry.id)
        #expect(after.graphProcesses[0].steps.allSatisfy { $0.outcome == "succeeded" })
        for capture in [merged, completed, arrived] {
            #expect(capture.row.pr?.id == merged.row.pr?.id)
        }
    }

    @Test("Task completion preserves execution and dated unstarted Tasks")
    func taskDeliveryFixture() throws {
        let rows = try JSONDecoder().decode([String: RoadmapTask].self, from: loadFixtureData("task_delivery_rows.json"))
        let pending = try #require(rows["pending"])
        #expect(pending.runtime?.completionPending?.contains("lf task complete") == true)
        #expect(rows["completed_running"]?.runtime?.status == .done)
        #expect(rows["completed_running"]?.execution == pending.execution)
        let due = try #require(rows["due"])
        #expect(due.runtime == nil)
        #expect(due.task.dueDate == "2026-10-08")
        #expect(!due.task.isSuccessful)
    }

    @Test("Planning delivery retains uncertainty, errors and both conflict values")
    func planningSyncFixture() throws {
        let sync = try JSONDecoder().decode(PlanningSyncStatus.self, from: loadFixtureData("planning_sync.json"))
        #expect(sync.connected)
        #expect(sync.changes[0].state == .pending)
        #expect(sync.changes[1].state == .uncertain)
        #expect(sync.changes[1].error == "Reply lost; awaiting readback")
        #expect(sync.changes[2].field == "task_title")
        #expect(sync.changes[3].field == "task_assignee")
        #expect(sync.changes[2].localValue == .string("Local title"))
        #expect(sync.changes[2].linearValue == .string("Linear title"))
        #expect(sync.changes[3].text.contains("provider: null"))
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(PlanningSyncStatus.self, from: Data("{}".utf8))
        }
    }

    @Test("Task overview reads its graph without diagnostic history")
    func taskOverviewFixture() throws {
        let overview = try JSONDecoder().decode(TaskOverview.self, from: loadFixtureData("task_overview.json"))
        #expect(overview.graph != nil)
        #expect(overview.sessions.count == 2)
        #expect(overview.graphProcesses.count == 1)
    }

    @Test("Task work retains conversations, Graphs and command history")
    func taskWorkFixture() throws {
        let data = try loadFixtureData("task_work.json")
        let work = try JSONDecoder().decode(TaskWork.self, from: data)
        #expect(work.sessions.count == 2)
        #expect(work.sessions[0].graphLfProcessId == nil)
        #expect(work.sessions[1].graphLfProcessId == work.graphProcesses[0].id)
        #expect(work.graphProcesses[0].name == "pursue")
        #expect(work.graphProcesses[0].state == .current)
        #expect(!work.processes.isEmpty)
        let graph = try #require(work.graph)
        #expect(graph.name == "feature")
        #expect(graph.interactiveSteps.map(\.name) == ["review-design", "demo"])
        #expect(graph.interactiveSteps.map(\.description) == ["you review the plan", "you try the change"])
        #expect(graph.position == .run(step: "review-design", lfProcessId: work.graphProcesses[0].id, running: true))
        #expect(graph.waitingStep == nil)
        #expect(try JSONDecoder().decode(TaskWork.self, from: JSONEncoder().encode(work)) == work)
        // Every position field is required.
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var wireGraph = try #require(wire["graph"] as? [String: Any])
        wireGraph["position"] = ["kind": "run", "step": "review-design", "lf_process_id": "p"]
        wire["graph"] = wireGraph
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(TaskWork.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test("A Graph process keeps its launched graph and every step, and requires each field")
    func graphDetailFixture() throws {
        let data = try loadFixtureData("graph_detail.json")
        let detail = try JSONDecoder().decode(GraphProcessDetail.self, from: data)
        #expect(detail.entry.state == .current)
        #expect(detail.steps.map(\.label) == ["implement", "compress", "sync", "realign", "loop-or-next", "implement"])
        #expect(detail.steps.last?.completedAt == nil)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire.removeValue(forKey: "steps")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(GraphProcessDetail.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test("A Graph whose process exited early reads as stopped and requires its name")
    func graphInventoryFixture() throws {
        struct Page: Decodable { let entries: [GraphProcessInventoryEntry] }
        let data = try loadFixtureData("graph_page.json")
        let graph = try #require(try JSONDecoder().decode(Page.self, from: data).entries.first)
        #expect(graph.state == .stopped)
        #expect(graph.endedAt == 18)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var entry = try #require((wire["entries"] as? [[String: Any]])?.first)
        entry.removeValue(forKey: "name")
        wire["entries"] = [entry]
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(Page.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test("Session states follow process evidence")
    func sessionStatesFixture() throws {
        let states = try JSONDecoder().decode([SessionState].self, from: loadFixtureData("session_states.json"))
        #expect(states == [.unknown, .active, .detached, .closed])
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(SessionState.self, from: Data(#""interrupted""#.utf8))
        }
    }

    @Test("Session page retains complete enumeration and requires entries")
    func sessionPageFixture() throws {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session_page.json")
        let data = try Data(contentsOf: url)
        let page = try JSONDecoder().decode(SessionPage.self, from: data)
        #expect(page.entries.count == 1)
        #expect(page.entries[0].agentProcessId == "44444444-4444-4444-8444-444444444444")
        #expect(page.entries[0].account == "work")
        #expect(page.entries[0].programStatus?.summary?.state == .blocked)
        #expect(page.entries[0].programStatus?.summary?.kind == .question)
        #expect(page.entries[0].programStatus?.summary?.msg == "Use **literal** text?")
        #expect(page.next == nil)
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(SessionPage.self, from: Data(#"{"next":null}"#.utf8))
        }
    }

    @Test("Process pages retain command outcomes, caller evidence and continuation")
    func processPageFixture() throws {
        let data = try loadFixtureData("process_page.json")
        let page = try JSONDecoder().decode(LfProcessPage.self, from: data)
        #expect(page.entries[0].exitCode == 42)
        #expect(page.entries[0].viaAgent == nil)
        #expect(page.entries[0].pid == nil)
        #expect(page.entries[1].pid == 4242)
        #expect(page.entries[1].kind == .agent)
        #expect(page.entries[1].agentSessionID == "conversation")
        #expect(page.entries[1].osStartedAt == 1790640000)
        #expect(page.entries[1].parentLfProcessId == page.entries[0].id)
        #expect(page.entries[1].outcome == nil)
        #expect(page.next?.id == page.entries[1].id)
        #expect(try JSONDecoder().decode(LfProcessPage.self, from: JSONEncoder().encode(page)) == page)
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(LfProcessPage.self, from: Data("{}".utf8))
        }
    }

    @Test("Context report keeps unknown sources distinct from zero")
    func contextReportFixture() throws {
        let report = try JSONDecoder().decode(ContextReport.self, from: loadFixtureData("context_report.json"))
        let steers = report.steps[0].sources[4]
        #expect(steers.source == "steers")
        #expect(steers.count == 384)
        #expect(!steers.overBudget)
        #expect(steers.authors == ["user"])
        #expect(report.steps[1].sources.allSatisfy { $0.tokens == nil })
        #expect(report.steps[1].gaps == ["this input has no retained capture"])
        #expect(report.totals.count == 9)
    }

    @Test("Conversation history retains native evidence and unknown LfProcess")
    func sessionHistoryFixture() throws {
        let data = try loadFixtureData("session_history.json")
        let events = try JSONDecoder().decode([SessionEvent].self, from: data)
        #expect(events[0].agentSession == AgentSessionId(rawValue: "thread_fixture"))
        #expect(events[1].kind == .completed)
        #expect(events[1].lfProcessId == nil)
        #expect(events[2].kind == .observed)
        #expect(events[2].agentSession == nil)
        #expect(events[2].providerTurn == nil)
        #expect(events[0].payload == .object([
            "total": .object(["inputTokens": .integer(40), "outputTokens": .integer(10)]),
            "last": .object(["inputTokens": .integer(20), "outputTokens": .integer(5)]),
        ]))
        #expect(try JSONDecoder().decode([SessionEvent].self, from: JSONEncoder().encode(events)) == events)
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

    @Test("Configured Project selection survives competing statuses and provider renames")
    func configuredProjectSelection() throws {
        var root = try #require(JSONSerialization.jsonObject(with: loadFixtureData("wave_detail.json")) as? [String: Any])
        var projects = try #require(root["projects"] as? [String: Any])
        let rows = try #require(projects["items"] as? [[String: Any]])
        var previous = try #require(rows.first)
        previous["current"] = false
        var selected = previous
        selected["id"] = "configured-project"
        selected["name"] = "Summer work — customer requests"
        selected["current"] = true
        selected["workflow"] = ""
        projects["items"] = [previous, selected]
        root["projects"] = projects
        let status = try JSONDecoder().decode(WaveDetailSnapshot.self, from: JSONSerialization.data(withJSONObject: root))
        #expect(status.currentProject?.id == "configured-project")
        #expect(status.projects.items.count == 2)
        selected.removeValue(forKey: "current")
        projects["items"] = [selected]
        root["projects"] = projects
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(WaveDetailSnapshot.self, from: JSONSerialization.data(withJSONObject: root))
        }
    }

    @Test("Work Activity fixture carries Session records tagged with Work")
    func workActivityFixtureCarriesSessionRecords() throws {
        let data = try loadFixtureData("work_activity_snapshot.json")
        let snapshot = try JSONDecoder().decode(WorkActivitySnapshot.self, from: data)

        #expect(snapshot.truncated == false)
        #expect(snapshot.items.map(\.subject) == ["W2-144", "W2-144", "product"])
        if case .inputCompletionRecorded(_, let captured, let status) = snapshot.items[0].fact {
            #expect(captured == 12)
            #expect(status == "ok")
        } else {
            Issue.record("expected a recorded Session outcome")
        }
        if case .providerHistoryRecorded(_, _, _, let process, let status) = snapshot.items[1].fact {
            #expect(process == "00000001-0000-4000-8000-000000000000")
            #expect(status == "completed")
        } else {
            Issue.record("expected a provider turn")
        }
        #expect(snapshot.items[2].work.kind == .wave)
        #expect(snapshot.items[2].fact == .inputCaptured(sessionId: "run_00000000000000000000000000000002", captured: 4))
    }

    @Test("unplaced Wave fixture retains planning without a Machine")
    func unplacedWaveFixture() throws {
        let wave = try JSONDecoder().decode(WaveSnapshot.self, from: loadFixtureData("unplaced_wave.json"))
        #expect(wave.machine == nil)
        #expect(wave.toWave().id == wave.id)
        #expect(wave.toWave().name == "shared")
        #expect(wave.activeTasks == 1)
    }

    @Test("wave detail fixture preserves Project and Task identity")
    func waveDetailFixturePreservesHierarchy() throws {
        let data = try loadFixtureData("wave_detail.json")
        let detail = try JSONDecoder().decode(WaveDetailSnapshot.self, from: data)

        #expect(detail.projectReadiness.state == .ready)
        #expect(detail.projectReadiness.projectId == detail.currentProject?.id)
        #expect(detail.projectReadiness.activation == nil)
        #expect(detail.wave.machine?.id == "home_00000000000000000000000000000001")
        #expect(detail.wave.machine?.route == "ssh://jack@mini-heart")
        #expect(detail.wave.machine?.label == "mini")

        // The Machine runtime evidence carries the state and the one contextual action.


        #expect(detail.currentProject?.sync.changes[1].field == "task_order")
        #expect(detail.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(detail.unavailableTasks[0].status == .unstarted)
        #expect(detail.unavailableTasks[0].owner == .wave)
        #expect(detail.tasks.items.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.tasks.items[0].task.state == "unstarted")
        #expect(detail.tasks.items[0].task.sync.changes[0].field == "creation")
        #expect(detail.tasks.items[0].task.id == detail.tasks.items[0].runtime?.workId)
        #expect(detail.tasks.items[0].task.linearId == "issue-1")
        #expect(detail.tasks.items[0].task.completedAt == nil)
        #expect(detail.tasks.items[0].pr?.publication?.github?.number == 912)
        #expect(detail.tasks.items[0].pr?.id == "pr_33333333333333333333333333333333")
        #expect(detail.tasks.items[0].pr?.publication?.merge?.mode == .user)
        #expect(detail.tasks.items.map(\.section) == [.waiting, .notStarted])
        #expect(detail.tasks.items[0].reference.workspace?.slug == "infrastructure-task")
        #expect(detail.tasks.items[0].reference.workspace?.worktree == "/src/loopflow.infrastructure.task")
        #expect(detail.tasks.items[0].reference.issueUrl?.host == "linear.app")
        // Ready is durable Work status. Historical failure evidence stays
        // visible without replacing that present-tense state.
        #expect(detail.tasks.items[0].runtime?.status == .unstarted)
        #expect(detail.tasks.items[0].runtime?.reason == "ready")
        #expect(detail.tasks.items[1].runtime == nil)
        #expect(detail.tasks.items[1].reference.issueUrl == nil)
        #expect(detail.tasks.items[1].reference.workspace == nil)
        #expect(detail.history.items[0].id == "run_00000000000000000000000000000001:12")
        #expect(detail.history.items[0].skill == "task/pursue")
        #expect(detail.history.items[0].taskPrId == "pr_33333333333333333333333333333333")
        #expect(detail.history.items[0].firstProviderAttemptAt == 1784052010)
        #expect(detail.history.items[0].usage.inputTokens == 12000)
        #expect(detail.history.items[0].recordedOutcome == "completed")
        #expect(detail.tasks.items[0].condition.state == .waiting)
        #expect(detail.tasks.items[0].condition.reason == "merge pull request head 333333333333 on GitHub")
        #expect(detail.tasks.items[0].actions.recommended == .openPr)
        #expect(detail.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")
        #expect(detail.metricPortfolio.metrics[0].evidence == .met(
            value: 1,
            sourceWindowStart: "2026-08-13T18:00:00Z",
            sourceWindowEnd: "2026-08-20T18:00:00Z"
        ))
    }

    @Test("roadmap fixture preserves sections and durable Task references")
    func roadmapFixturePreservesTaskReferences() throws {
        let data = try loadFixtureData("roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)

        #expect(roadmap.generatedAt == "2026-07-15T00:00:00Z")
        #expect(roadmap.waves.count == 2)
        let product = try #require(roadmap.waves.first)
        #expect(product.wave.name == "product")

        #expect(product.metricPortfolio.metrics[0].identity.metricId == "task-loop-trust")

        #expect(product.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(product.unavailableTasks[0].status == .unstarted)
        #expect(product.unavailableTasks[0].recovery.contains("lf task status task_40fbeea"))
        let tasks = product.tasks.items
        #expect(tasks.map(\.section) == [.now, .waiting, .notStarted, .later])
        #expect(tasks.map(\.condition.state) == [.clear, .waiting, .clear, .clear])
        #expect(tasks[0].reference.workspace?.slug == "make-lf-work-the-machine")
        #expect(tasks[0].reference.workspace?.machineLabel == "build-server")
        #expect(tasks[2].reference.workspace == nil)
        #expect(tasks[2].reference.issueUrl == nil)
        #expect(tasks[3].reference.workspace?.branch == "jack-heart/now-available-research")
        // Start evidence is required: a prepared checkout is not started work.
        #expect(tasks.map { $0.runtime.map { $0.owner != nil } } == [false, true, nil, true])
        // The wire carries each Task once; Waves share them by the Task's Wave.
        #expect(roadmap.tasks.items.count == 4)
        #expect(roadmap.waves[1].tasks.items.isEmpty)

    }

    @Test("closed Task page carries Wave status rows and their continuation")
    func closedTaskPageCarriesRowsAndContinuation() throws {
        let page = try JSONDecoder().decode(ClosedTaskPage.self, from: loadFixtureData("closed_task_page.json"))
        let task = try #require(page.tasks.first)
        #expect(task.task.isSuccessful)
        #expect(task.task.completedAt == "2026-07-14T18:30:00Z")
        #expect(page.next == task.id)
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
        guard case .step(_, _, _, let node, let iterations, let occurrence) = session.graphMembership else {
            Issue.record("Retain the known Graph membership")
            return
        }
        #expect(node == nil)
        #expect(iterations == nil)
        #expect(occurrence == .unknown)
        #expect(session.graphMembership.label.contains("position unavailable"))
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
        #expect(session.preview?.providerTurn == "turn-fixture")
        #expect(session.preview?.observedAt == 1700000000)
        #expect(session.titleSource == .human)
        #expect(session.detail == "review-design")
        #expect(session.state == .active)
        #expect(session.workPath == "product / LOO-291")
        #expect(session.actions.map(\.kind) == [.open, .moveHere])

        let encoded = try JSONEncoder().encode(sessions)
        let decoded = try JSONDecoder().decode([SessionRecord].self, from: encoded)
        #expect(decoded == sessions)
    }

    @Test("Session Graph membership mirrors every Rust projection and is required")
    func sessionGraphMembershipFixture() throws {
        let data = try loadFixtureData("session_memberships.json")
        let sessions = try JSONDecoder().decode([SessionRecord].self, from: data)
        #expect(sessions.map(\.graphMembership) == [
            .step(graph: "task-design", graphLfProcessId: "00000000-0000-0000-0000-00000000f10w",
                  step: "review-design", node: 1, iterations: [[]], occurrence: .current),
            .step(graph: "feature", graphLfProcessId: "00000000-0000-0000-0000-0000000000f2",
                  step: "implement", node: 6, iterations: [[2, 1], [1]], occurrence: .earlier),
            .independent,
            .unknown(reason: "Run run_00000000000000000000000000000004 predates recorded Graph membership"),
            .step(graph: "feature", graphLfProcessId: "00000000-0000-0000-0000-0000000000f1",
                  step: "implement", node: 2, iterations: [[2, 1]], occurrence: .past),
            .step(graph: "feature", graphLfProcessId: "00000000-0000-0000-0000-0000000000f0",
                  step: "implement", node: nil, iterations: nil, occurrence: .past),
            .step(graph: "feature", graphLfProcessId: "00000000-0000-0000-0000-0000000000f2",
                  step: "demo", node: 9, iterations: [[0, 0]], occurrence: .current),
        ])
        let graphs = try JSONDecoder().decode([String: CompiledGraph].self,
            from: loadFixtureData("session_membership_graphs.json"))
        for session in sessions {
            if case let .step(_, invocation, step, node?, _, _) = session.graphMembership {
                let graph = try #require(graphs[invocation])
                #expect(graph.node(node)?.label == step)
            }
        }
        #expect(sessions.last?.titleSource == .unavailable)
        #expect(sessions.map(\.provider) == ["codex", "claude", "claude", "claude", "claude", "claude", nil])
        #expect(sessions[1].graphMembership.label == "feature / implement · loop 1 pass 3, loop 2 pass 2, loop 3 pass 2 · earlier")
        #expect(sessions[4].graphMembership.label == "feature / implement · loop 1 pass 3, loop 2 pass 2 · past run")
        #expect(sessions[5].graphMembership.label == "feature / implement · pass unavailable · past run")
        let legacy: SessionGraphMembership = .step(graph: "feature", graphLfProcessId: "old", step: "demo",
                                                 node: nil, iterations: nil, occurrence: .past)
        #expect(legacy.label == "feature / demo · pass unavailable · past run")
        #expect(try JSONDecoder().decode(SessionGraphMembership.self,
                                       from: JSONEncoder().encode(legacy)) == legacy)
        let objects = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for var value in objects {
            value.removeValue(forKey: "graph_membership")
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            }
        }
        let decoded = try JSONDecoder().decode([SessionRecord].self, from: JSONEncoder().encode(sessions))
        #expect(decoded == sessions)
    }

    @Test("canonical_ids: structured Session IDs decode canonically; native bytes stay exact")
    func canonicalIdsSessionWire() throws {
        let session = try JSONDecoder().decode(SessionRecord.self, from: loadFixtureData("canonical_ids_session.json"))
        #expect(session.id == "session_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        #expect(session.taskIds == ["task_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"])
        #expect(session.waveId == "dddddddd-dddd-4ddd-8ddd-dddddddddddd")
        #expect(session.agentProcessId == "ffffffff-ffff-4fff-8fff-ffffffffffff")
        #expect(session.work?.id == "task_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        #expect(session.workspace?.machineId == "home_eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
        #expect(session.workspace?.taskId == "task_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        #expect(session.workspace?.worktree == "/Case/Worktree")
        guard case .step(_, let graph, _, _, _, _) = session.graphMembership else {
            Issue.record("expected a Graph step")
            return
        }
        #expect(graph == "cccccccc-cccc-4ccc-8ccc-cccccccccccc")
        #expect(session.preview?.providerThread == "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA")
        #expect(session.preview?.providerTurn == "Turn-A")
        #expect(session.openArgv.last == "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        #expect(try JSONDecoder().decode(SessionRecord.self, from: JSONEncoder().encode(session)) == session)
    }

    @Test("Every Session kind retains its identity without a Run field")
    func sessionIdentityHasNoRun() throws {
        let data = try loadFixtureData("session.json")
        for kind in ["conversation", "graph"] {
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

    @Test("Historical Graph Session feedback survives without completion controls")
    func graphSessionFixtureRoundTrips() throws {
        let session = try JSONDecoder().decode(
            SessionRecord.self,
            from: loadFixtureData("session.json")
        )

        #expect(session.state == .unknown)
        #expect(session.availability == .available)
        #expect(!session.taskPrimary)
        #expect(session.titleSource == .generated)
        #expect(session.actions.map(\.kind) == [.open])
        #expect(session.actions.allSatisfy { $0.unavailableReason == nil })
        #expect(session.statusLabel == "Available")
        #expect(session.readySummary == "The design now reflects Jack's requested changes.")
        #expect(session.openArgv.suffix(3) == [
            "session", "connect", "task_00000000000000000000000000000001:task-design:review_kickoff:0"
        ])

        let encoded = try JSONEncoder().encode(session)
        let decoded = try JSONDecoder().decode(SessionRecord.self, from: encoded)
        #expect(decoded == session)
    }

    @Test("Peer planning keeps independent receipts")
    func peerPlanningStatus() throws {
        let data = try loadFixtureData("peer_planning_status.json")
        let statuses = try JSONDecoder().decode([PeerPlanningStatus].self, from: data)
        #expect(statuses[0].pendingLocal)
        #expect(statuses[0].publicationState == "unconfirmed")
        #expect(statuses[0].importedRevision == "retained-import")
        #expect(statuses[0].fetchedRevision == "newer-fetch")
        #expect(statuses[0].conflicts[0].target == "retained-task")
        #expect(statuses[1].pendingLocal == true)
        #expect(statuses[1].active)
        for field in ["conflicts", "pending_local"] {
            var object = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])[1]
            object.removeValue(forKey: field)
            #expect(throws: (any Error).self) {
                try JSONDecoder().decode(PeerPlanningStatus.self, from: JSONSerialization.data(withJSONObject: object))
            }
        }
    }

    @Test("Watch frames decode through their part and keep the wire text a saved workspace needs")
    func watchFramesDecode() throws {
        let entries = try #require(JSONSerialization.jsonObject(with: loadFixtureData("watch_frames.json"))
            as? [[String: Any]])
        let scope = WorkScope(repo: "/src/loopflow", headless: false, task: "LOO-1",
                              activity: WorkActivityScope(wave: nil, project: nil, task: "LOO-1"))
        let lines = try entries.map { try JSONSerialization.data(withJSONObject: try #require($0["frame"])) }
        let parts = try entries.map { try #require(WorkPart(rawValue: $0["part"] as? String ?? "")) }
        let frames = try zip(parts, lines).enumerated().map { index, entry in
            try WorkFrame.decode(part: entry.0, scope: scope, sequence: index + 1, line: entry.1)
        }
        #expect(frames.map(\.content.part) == ["planning", "selections", "sessions", "task", "work_activity",
                                                "activity", "task", "peer_planning"])
        #expect(frames[0].answers == nil)
        #expect(frames[2].answers == 7)
        guard case .planning(let planning?) = frames[0].content,
              case .selections(let plans?) = frames[1].content,
              case .sessions(let sessions?) = frames[2].content,
              case .workActivity(let activity?) = frames[4].content,
              case .peerPlanning(let peers?) = frames[7].content else {
            Issue.record("fixture parts changed shape")
            return
        }
        #expect(planning.waves.isEmpty)
        #expect(plans.first?.dashboardID == "picked")
        #expect(sessions.repo == "/src/loopflow")
        #expect(!sessions.includesHeadless)
        #expect(activity.scope.task == "LOO-1")
        #expect(peers.repo == "/src/loopflow")
        #expect(peers.destinations.isEmpty)
        guard case .task(nil) = frames[3].content else {
            Issue.record("a failed reading has no body")
            return
        }
        #expect(frames[3].unavailable == "Task LOO-1 is not registered")
        guard case .task(let read?) = frames[6].content else {
            Issue.record("a read Task part carries its work and Graph processes")
            return
        }
        #expect(read.task == "LOO-1")
        #expect(read.graphProcesses.count == read.work.graphProcesses.count)
        #expect(read.diagnostics == nil)

        // Saved text restores through the decoders one-shot reads use.
        let roadmap = try #require(frames[0].wire?.roadmap)
        #expect(try RegistryQuery.decode(RoadmapSnapshot.self, from: roadmap).waves.isEmpty)
        let page = try #require(frames[2].wire?.sessionPage)
        #expect(try RegistryQuery.decode(SessionPage.self, from: page).next == nil)

        for field in ["answers", "unavailable", "body"] {
            var missing = try #require(entries[0]["frame"] as? [String: Any])
            missing.removeValue(forKey: field)
            #expect(throws: (any Error).self) {
                try WorkFrame.decode(part: .planning, scope: scope, sequence: 1,
                                     line: JSONSerialization.data(withJSONObject: missing))
            }
        }
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

    @Test("Task acquisition evidence is required independently of delivery")
    func taskAcquisitionEvidence() throws {
        let root = try loadFixture("local_task.json")
        let detail = try #require(root["task"] as? [String: Any])
        var task = try #require(detail["task"] as? [String: Any])
        for state in ["available", "invalid", "removed", "unavailable"] {
            task["acquisition"] = ["state": state, "observed_at": 17]
            let decoded = try JSONDecoder().decode(TaskPlanningSnapshot.self,
                from: JSONSerialization.data(withJSONObject: task))
            #expect(decoded.acquisition.state.rawValue == state)
            #expect(decoded.acquisition.observedAt == 17)
            #expect(decoded.id == task["id"] as? String)
        }
        task.removeValue(forKey: "acquisition")
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(TaskPlanningSnapshot.self,
                from: JSONSerialization.data(withJSONObject: task))
        }
    }

    private func loadFixture(_ name: String) throws -> [String: Any] {
        let data = try loadFixtureData(name)
        let json = try JSONSerialization.jsonObject(with: data)
        return try #require(json as? [String: Any])
    }
}

func loadFixtureData(_ name: String) throws -> Data {
    let testFile = URL(fileURLWithPath: #filePath)
    let fixtures = testFile
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto")
        .appendingPathComponent(name)
    return try Data(contentsOf: fixtures)
}

@Test("Session input history retains distinct native results and unknown Process")
func sessionInputHistoryFixture() throws {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/session_history_summary.json")
    let value = try JSONDecoder().decode(SessionHistory.self, from: Data(contentsOf: url))
    #expect(value.providers.count == 2)
    #expect(value.providers[0].outcome == "failed")
    #expect(value.providers[0].lfProcessId == nil)
    #expect(value.providers[0].usage.inputTokens == nil)
    #expect(value.providers[1].outcome == "completed")
    #expect(value.providers[1].usage.inputTokens == 0)
    #expect(value.status == "failed → completed")
}

// Written by the Rust composed lifecycle test from one disposable Home.
struct TaskLifecycleCapture {
    let row: RoadmapTask
    /// `lf task status FIX-1 --json` as its watch would carry it.
    let status: [String: Any]

    static func load(_ data: Data) throws -> [String: TaskLifecycleCapture] {
        let phases = try #require(JSONSerialization.jsonObject(with: data) as? [String: [String: Any]])
        return try phases.mapValues { capture in
            TaskLifecycleCapture(
                row: try JSONDecoder().decode(RoadmapTask.self,
                                              from: JSONSerialization.data(withJSONObject: try #require(capture["row"]))),
                status: try #require(capture["status"] as? [String: Any]))
        }
    }

    func taskPart() throws -> WorkFrame.TaskPart {
        let line: [String: Any] = ["answers": NSNull(), "unavailable": NSNull(), "body": status]
        let frame = try WorkFrame.decode(
            part: .task, scope: WorkScope(repo: nil, headless: false, task: "FIX-1", activity: nil),
            sequence: 1, line: JSONSerialization.data(withJSONObject: line))
        guard case .task(let part?) = frame.content else {
            throw CocoaError(.coderReadCorrupt)
        }
        return part
    }
}
