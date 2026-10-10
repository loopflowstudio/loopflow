import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixtures for the `lf` and per-Wave listener contracts consumed by
/// the Mac app.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test func taskFilesFramesKeepInvalidationSeparateFromHeartbeat() throws {
        let data = try loadFixtureData("task_files_frames.json")
        let frames = try JSONDecoder().decode([TaskFilesFrame].self, from: data)
        #expect(frames.map(\.changed) == [true, false])
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
        #expect(reports[1].action?.mode == .replace)
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

    @Test func desktopOpenExplanationsPreserveProposalsAndImpediments() throws {
        let data = try loadFixtureData("desktop_open_explanations.json")
        let reports = try JSONDecoder().decode([DesktopOpenExplanation].self, from: data)
        #expect(reports[0].url?.contains("diff=true") == true)
        #expect(reports[1].url == nil)
        #expect(!reports[1].impediments.isEmpty)
        #expect(try JSONDecoder().decode([DesktopOpenExplanation].self, from: JSONEncoder().encode(reports)) == reports)
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
        #expect(reports[5].action == .refile(wave: "destination-wave", project: "destination-project", previousProject: "source-project"))
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

    @Test func taskMoveExplanationsPreservePositionsAndCompletionUncertainty() throws {
        let data = try loadFixtureData("task_move_explanations.json")
        let reports = try JSONDecoder().decode([TaskMoveExplanation].self, from: data)
        #expect(reports[0].action?.from == .edge(index: 1, processLfid: "process-retained", running: true))
        #expect(reports[0].action?.to == "start")
        #expect(reports[0].action?.reason == "Restart Workflow")
        #expect(reports[1].action?.workflow == nil)
        #expect(reports[1].action?.from == nil)
        #expect(reports[1].action?.to == "end")
        #expect(!reports[1].unavailable.isEmpty)
        #expect(reports[2].action == nil)
        #expect(try JSONDecoder().decode([TaskMoveExplanation].self, from: JSONEncoder().encode(reports)) == reports)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        var action = try #require(wire[0]["action"] as? [String: Any])
        action.removeValue(forKey: "to")
        wire[0]["action"] = action
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode([TaskMoveExplanation].self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test func taskRunExplanationsPreserveActionsAndMissingEvidence() throws {
        let data = try loadFixtureData("task_run_explanations.json")
        let reports = try JSONDecoder().decode([TaskRunExplanation].self, from: data)
        #expect(reports[0].action == .edge(workflow: "feature", takeUp: true, from: "start", to: "design", flow: "task-design"))
        #expect(reports[1].action == .edge(workflow: "feature", takeUp: false, from: "design", to: "end", flow: nil))
        #expect(reports[2].action == .flow("proof"))
        #expect(reports[3].action == nil)
        #expect(try JSONDecoder().decode([TaskRunExplanation].self, from: JSONEncoder().encode(reports)) == reports)
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

    @Test func paneCommandRequiresEveryTargetComponent() throws {
        let data = try loadFixtureData("desktop_pane_commands.json")
        let requests = try JSONDecoder().decode([DesktopPaneCommand].self, from: data)
        let original = try #require(JSONSerialization.jsonObject(with: data) as? NSArray)
        let encoded = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(requests)) as? NSArray)
        #expect(original == encoded)
        for request in requests {
            let wire = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any])
            let action = try #require(wire["action"] as? [String: Any])
            for field in action.keys {
                var missing = wire, payload = action
                payload.removeValue(forKey: field)
                missing["action"] = payload
                #expect(throws: DecodingError.self) {
                    try JSONDecoder().decode(DesktopPaneCommand.self, from: JSONSerialization.data(withJSONObject: missing))
                }
            }
        }
        let request = try #require(requests.first)
        #expect(request.action == .hide)
        #expect(request.target.incarnation == "occurrence-session-pane")
        #expect(try JSONDecoder().decode(DesktopPaneCommand.self, from: JSONEncoder().encode(request)) == request)
        let wire = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any])
        for field in ["repository", "window", "machine_id", "worktree", "pane", "incarnation"] {
            var missing = wire
            var target = try #require(missing["target"] as? [String: Any])
            target.removeValue(forKey: field)
            missing["target"] = target
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(DesktopPaneCommand.self, from: JSONSerialization.data(withJSONObject: missing))
            }
        }
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
        #expect(window.reading == "unavailable")
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
        #expect(window.workspaces.first?.layout.children.first?.subject == "session-one")
        #expect(window.workspaces.first?.layout.children.first?.surface == "native-surface-incarnation")
        #expect(window.workspaces.first?.layout.children.last?.surface == nil)
        #expect(try JSONDecoder().decode(DesktopInspection.self, from: JSONEncoder().encode(report)) == report)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var windows = try #require(wire["windows"] as? [[String: Any]])
        windows[0].removeValue(forKey: "reading")
        wire["windows"] = windows
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(DesktopInspection.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }


    @Test("Composed delivery keeps completion independent of the running Flow and its arrival")
    func composedTaskLifecycle() throws {
        let snapshots = try JSONDecoder().decode(
            [String: TaskLifecycleCapture].self, from: loadFixtureData("task_lifecycle.json"))
        let merged = try #require(snapshots["merged"])
        let completed = try #require(snapshots["completed"])
        let arrived = try #require(snapshots["arrived"])
        #expect(merged.row.runtime?.status != .done)
        #expect(completed.row.runtime?.status == .done)
        #expect(arrived.row.runtime?.status == .done)
        let before = try merged.taskPart()
        let during = try completed.taskPart()
        let after = try arrived.taskPart()
        #expect(before.work.workflow == during.work.workflow)
        guard case .edge(_, _, true) = during.work.workflow?.position else {
            Issue.record("Completed Task lost its live edge")
            return
        }
        #expect(after.work.workflow?.position == .node("end"))
        #expect(after.work.workflow?.history.last?.kind == .arrived)
        #expect(before.flowProcesses.count == 1 && after.flowProcesses.count == 1)
        #expect(before.flowProcesses[0].entry.id == after.flowProcesses[0].entry.id)
        #expect(after.flowProcesses[0].steps.allSatisfy { $0.outcome == "succeeded" })
        for capture in [merged, completed, arrived] {
            guard case .planning(let plan?) = capture.planningFrame.content else {
                Issue.record("Missing planning frame")
                return
            }
            let row = try #require(plan.roadmap.waves.flatMap { $0.tasks.items }
                .first { $0.task.identifier == "FIX-1" })
            #expect(row.runtime?.status == capture.row.runtime?.status)
            #expect(row.followThrough == capture.row.followThrough)
            #expect(row.pr?.id == merged.row.pr?.id)
        }
        #expect(completed.row.followThrough.links.count == 1)
        #expect(completed.row.followThrough.links[0].identifier.hasPrefix("lf-"))
        #expect(completed.row.followThrough.links[0].due == "2026-10-09")
        #expect(arrived.row.followThrough == completed.row.followThrough)
    }

    @Test("Follow-through remains pending until confirmed, with dated unstarted follow-ups")
    func taskDeliveryFixture() throws {
        let rows = try JSONDecoder().decode([String: RoadmapTask].self, from: loadFixtureData("task_delivery_rows.json"))
        let pending = try #require(rows["pending"])
        #expect(!pending.followThrough.resolved)
        #expect(pending.runtime?.completionPending?.contains("lf task complete") == true)
        #expect(rows["completed_running"]?.runtime?.status == .done)
        #expect(rows["completed_running"]?.execution == pending.execution)
        #expect(pending.followThrough.deliveryLabel(merged: true, done: false) == "Merged · Follow-through pending")
        let done = try #require(rows["done"])
        #expect(done.followThrough.resolved)
        #expect(done.followThrough.deliveryLabel(merged: true, done: true) == "Done · Follow-up W2-FOLLOW")
        #expect(done.followThrough.links.first?.url?.absoluteString == "https://linear.app/loopflow/issue/W2-FOLLOW")
        #expect(rows["none"]?.followThrough.resolved == true)
        let due = try #require(rows["due"])
        #expect(due.runtime == nil)
        #expect(due.task.dueDate == "2026-10-08")
        #expect(!due.task.completed)
        #expect(rows["unrelated"]?.task.followUpSources.isEmpty == true)
        #expect(due.task.followUpLabel == "Follow-up to W2-SOURCE · Due 2026-10-08")
    }
    @Test("Planning delivery retains uncertainty, errors and both conflict values")
    func planningSyncFixture() throws {
        let sync = try JSONDecoder().decode(PlanningSyncStatus.self, from: loadFixtureData("planning_sync.json"))
        #expect(sync.connected)
        #expect(sync.changes[0].state == .pending)
        #expect(sync.changes[1].state == .uncertain)
        #expect(sync.changes[1].error == "Reply lost; awaiting readback")
        #expect(sync.changes[2].localValue == .string("Local title"))
        #expect(sync.changes[2].linearValue == .string("Linear title"))
        #expect(sync.changes[3].text.contains("Linear: null"))
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(PlanningSyncStatus.self, from: Data("{}".utf8))
        }
    }

    @Test("Task work retains conversations, Flows and command history")
    func taskWorkFixture() throws {
        let data = try loadFixtureData("task_work.json")
        let work = try JSONDecoder().decode(TaskWork.self, from: data)
        #expect(work.sessions.count == 2)
        #expect(work.sessions[0].flowProcessLfid == nil)
        #expect(work.sessions[1].flowProcessLfid == work.flowProcesses[0].id)
        #expect(work.flowProcesses[0].name == "pursue")
        #expect(work.flowProcesses[0].state == .current)
        #expect(!work.processes.isEmpty)
        let workflow = try #require(work.workflow)
        #expect(workflow.nodes.map(\.name) == ["design", "demo"])
        #expect(workflow.nodes.map(\.description) == ["you review the plan", "you try the change"])
        #expect(workflow.edges.last?.launchName == "ship")
        // An edge that runs nothing is chosen by the node it enters.
        let research = try JSONDecoder().decode([WorkflowCatalogEntry].self, from: loadFixtureData("workflow_catalog.json"))
            .first { $0.name == "research" }?.workflow
        #expect(research?.edges.last?.flow == nil)
        #expect(research?.edges.last?.launchName == "end")
        #expect(workflow.position == .edge(index: 2, processLfid: work.flowProcesses[0].id, running: true))
        #expect(workflow.outgoing.isEmpty)
        #expect(workflow.history.map(\.kind) == [.set, .tookUp, .chose, .arrived, .chose])
        #expect(workflow.history.map(\.actor) == [.person, .person, .person, .edge, .conversation])
        // A move no registered process made names no Process.
        #expect(workflow.history[0].processLfid == nil)
        #expect(workflow.history.last?.sessionId == work.sessions[0].id)
        #expect(workflow.history.last?.note == "take the smaller approach")
        #expect(try JSONDecoder().decode(TaskWork.self, from: JSONEncoder().encode(work)) == work)
    }

    @Test("A Flow process keeps its launched graph and every step, and requires each field")
    func flowDetailFixture() throws {
        let data = try loadFixtureData("flow_detail.json")
        let detail = try JSONDecoder().decode(FlowProcessDetail.self, from: data)
        #expect(detail.entry.state == .current)
        #expect(detail.steps.map(\.label) == ["implement", "compress", "sync", "realign", "loop-or-next", "implement"])
        #expect(detail.steps.last?.completedAt == nil)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire.removeValue(forKey: "steps")
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(FlowProcessDetail.self, from: JSONSerialization.data(withJSONObject: wire))
        }
    }

    @Test("A Flow whose driver exited early reads as stopped and requires its name")
    func flowInventoryFixture() throws {
        struct Page: Decodable { let entries: [FlowProcessInventoryEntry] }
        let data = try loadFixtureData("flow_page.json")
        let flow = try #require(try JSONDecoder().decode(Page.self, from: data).entries.first)
        #expect(flow.state == .stopped)
        #expect(flow.endedAt == 18)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var entry = try #require((wire["entries"] as? [[String: Any]])?.first)
        entry.removeValue(forKey: "name")
        wire["entries"] = [entry]
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(Page.self, from: JSONSerialization.data(withJSONObject: wire))
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
        #expect(page.entries[0].providerGeneration == 1)
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
        #expect(page.entries[1].parentProcessLFID == page.entries[0].lfid)
        #expect(page.entries[1].outcome == nil)
        #expect(page.next?.lfid == page.entries[1].lfid)
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
        #expect(events[0].agentSession == AgentSessionId(rawValue: "thread_fixture"))
        #expect(events[1].kind == .completed)
        #expect(events[1].processLFID == nil)
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
        #expect(status.currentProject?.workflow == "")
        #expect(status.projects.items.count == 2)
        selected.removeValue(forKey: "current")
        projects["items"] = [selected]
        root["projects"] = projects
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(WaveDetailSnapshot.self, from: JSONSerialization.data(withJSONObject: root))
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
        #expect(plan.currentProject?.workflow == "task-design")
        #expect(plan.currentProject?.current == true)
        #expect(plan.currentProject?.krs.count == 1)
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
        #expect(detail.wave.machine?.repo == "src/project")

        // The Machine runtime evidence carries the state and the one contextual action.


        #expect(detail.currentProject?.workflow == "task-design")
        #expect(detail.currentProject?.sync?.changes[1].field == "task_order")
        #expect(detail.unavailableTasks[0].taskIdentifier == "W2-127")
        #expect(detail.unavailableTasks[0].status == .ready)
        #expect(detail.unavailableTasks[0].owner == .wave)
        #expect(detail.tasks.items.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.tasks.items[0].task.state == "unstarted")
        #expect(detail.tasks.items[0].task.sync?.changes[0].field == "creation")
        #expect(detail.tasks.items[0].task.completedAt == nil)
        #expect(detail.tasks.items[0].pr?.publication?.github?.number == 912)
        #expect(detail.tasks.items[0].pr?.id == "pr_33333333333333333333333333333333")
        #expect(detail.tasks.items[0].pr?.publication?.merge?.mode == .user)
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
        #expect(product.currentProject?.workflow == "feature")

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
        #expect(session.actions.map(\.kind) == [.open, .moveHere])

        let encoded = try JSONEncoder().encode(sessions)
        let decoded = try JSONDecoder().decode([SessionRecord].self, from: encoded)
        #expect(decoded == sessions)
    }

    @Test("Session Flow membership mirrors every Rust projection and is required")
    func sessionFlowMembershipFixture() throws {
        let data = try loadFixtureData("session_memberships.json")
        let sessions = try JSONDecoder().decode([SessionRecord].self, from: data)
        #expect(sessions.map(\.flowMembership) == [
            .step(flow: "task-design", flowProcessLfid: "00000000-0000-0000-0000-00000000f10w",
                  step: "review-design", node: 1, iterations: [[]], occurrence: .current),
            .step(flow: "feature", flowProcessLfid: "00000000-0000-0000-0000-0000000000f2",
                  step: "implement", node: 6, iterations: [[2, 1], [1]], occurrence: .earlier),
            .independent,
            .unknown(reason: "Run run_00000000000000000000000000000004 predates recorded Flow membership"),
            .step(flow: "feature", flowProcessLfid: "00000000-0000-0000-0000-0000000000f1",
                  step: "implement", node: 2, iterations: [[2, 1]], occurrence: .past),
            .step(flow: "feature", flowProcessLfid: "00000000-0000-0000-0000-0000000000f0",
                  step: "implement", node: nil, iterations: nil, occurrence: .past),
            .step(flow: "feature", flowProcessLfid: "00000000-0000-0000-0000-0000000000f2",
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
        #expect(sessions[1].flowMembership.label == "feature / implement · loop 1 pass 3, loop 2 pass 2, loop 3 pass 2 · earlier")
        #expect(sessions[4].flowMembership.label == "feature / implement · loop 1 pass 3, loop 2 pass 2 · past run")
        #expect(sessions[5].flowMembership.label == "feature / implement · pass unavailable · past run")
        let legacy: SessionFlowMembership = .step(flow: "feature", flowProcessLfid: "old", step: "demo",
                                                 node: nil, iterations: nil, occurrence: .past)
        #expect(legacy.label == "feature / demo · pass unavailable · past run")
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
        for kind in ["conversation", "flow"] {
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

    @Test("Historical Flow Session feedback survives without completion controls")
    func flowSessionFixtureRoundTrips() throws {
        let session = try JSONDecoder().decode(
            SessionRecord.self,
            from: loadFixtureData("session.json")
        )

        #expect(session.state == .unknown)
        #expect(session.attention == .waiting)
        #expect(!session.taskPrimary)
        #expect(session.titleSource == .generated)
        #expect(session.actions.map(\.kind) == [.open])
        #expect(session.actions.allSatisfy { $0.unavailableReason == nil })
        #expect(session.statusLabel == "Waiting")
        #expect(session.readySummary == "The design now reflects Jack's requested changes.")
        #expect(session.openArgv.suffix(3) == [
            "session", "connect", "task_00000000000000000000000000000001:task-design:review_kickoff:0"
        ])

        let encoded = try JSONEncoder().encode(session)
        let decoded = try JSONDecoder().decode(SessionRecord.self, from: encoded)
        #expect(decoded == session)
    }

    @Test("Peer planning retains unknown pending state and independent receipts")
    func peerPlanningStatus() throws {
        let data = try loadFixtureData("peer_planning_status.json")
        let statuses = try JSONDecoder().decode([PeerPlanningStatus].self, from: data)
        #expect(statuses[0].pendingLocal == nil)
        #expect(statuses[0].publicationState == "unconfirmed")
        #expect(statuses[0].importedRevision == "retained-import")
        #expect(statuses[0].fetchedRevision == "newer-fetch")
        #expect(statuses[0].conflicts[0].object.id == "retained-task")
        #expect(statuses[1].pendingLocal == true)
        #expect(statuses[1].active)
        var object = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])[1]
        object.removeValue(forKey: "conflicts")
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(PeerPlanningStatus.self, from: JSONSerialization.data(withJSONObject: object))
        }
    }

    @Test("Work frames decode every part and keep the wire text a saved workspace needs")
    func workspaceFramesDecode() throws {
        let data = try loadFixtureData("work_frame.json")
        let lines = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        let frames = try lines.map { try WorkFrame.decode(line: JSONSerialization.data(withJSONObject: $0)) }

        #expect(frames.map(\.content.part) == ["planning", "sessions", "task", "work_activity", "activity", "heartbeat", "task", "peer_planning"])
        #expect(frames.map(\.sequence) == [1, 2, 3, 4, 5, 6, 7, 8])
        guard case .peerPlanning(let peers?) = frames[7].content else {
            Issue.record("peer planning frame missing")
            return
        }
        #expect(peers.repo == "/src/loopflow")
        #expect(peers.destinations.isEmpty)
        #expect(frames[0].answers == nil)
        #expect(frames[1].answers == 7)
        #expect(frames[0].revisions?.planning == 911)
        #expect(frames[4].revisions == nil)
        guard case .task(nil) = frames[2].content else {
            Issue.record("a failed reading has no body")
            return
        }
        #expect(frames[2].unavailable == "Task LOO-1 is not registered")
        guard case .task(let read?) = frames[6].content else {
            Issue.record("a read Task part carries its work and Flow processes")
            return
        }
        #expect(read.task == "LOO-1")
        #expect(read.flowProcesses.isEmpty)
        guard case .sessions(let sessions?) = frames[1].content,
              case .workActivity(let activity?) = frames[3].content,
              case .heartbeat(let heartbeat) = frames[5].content else {
            Issue.record("fixture parts changed shape")
            return
        }
        #expect(sessions.repo == "/src/loopflow")
        #expect(!sessions.includesHeadless)
        #expect(activity.scope.task == "LOO-1")
        #expect(heartbeat.projections["planning"] == 3)

        // Saved text restores through the decoders one-shot reads use.
        let roadmap = try #require(frames[0].wire?.roadmap)
        #expect(try RegistryQuery.decode(RoadmapSnapshot.self, from: roadmap).waves.isEmpty)
        let page = try #require(frames[1].wire?.sessionPage)
        #expect(try RegistryQuery.decode(SessionPage.self, from: page).next == nil)

        var missing = lines[0]
        missing.removeValue(forKey: "answers")
        #expect(throws: (any Error).self) {
            try WorkFrame.decode(line: JSONSerialization.data(withJSONObject: missing))
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

@Test("Session input history retains distinct native results and unknown Process")
func sessionInputHistoryFixture() throws {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/dto/session_history_summary.json")
    let value = try JSONDecoder().decode(SessionHistory.self, from: Data(contentsOf: url))
    #expect(value.providers.count == 2)
    #expect(value.providers[0].outcome == "failed")
    #expect(value.providers[0].processLfid == nil)
    #expect(value.providers[0].usage.inputTokens == nil)
    #expect(value.providers[1].outcome == "completed")
    #expect(value.providers[1].usage.inputTokens == 0)
    #expect(value.status == "failed → completed")
}

// Written by the Rust composed lifecycle test from one disposable Home.
struct TaskLifecycleCapture: Decodable {
    let row: RoadmapTask
    let planningFrame: WorkFrame
    let taskFrame: WorkFrame

    enum CodingKeys: String, CodingKey {
        case row
        case planningFrame = "planning_frame"
        case taskFrame = "task_frame"
    }

    func taskPart() throws -> WorkFrame.TaskPart {
        guard case .task(let part?) = taskFrame.content else {
            throw CocoaError(.coderReadCorrupt)
        }
        return part
    }
}
