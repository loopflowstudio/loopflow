import Foundation
@testable import Loopflow
import Testing

/// A Machine whose replies the test hands out one at a time, each to the
/// next write sent, waiting for that write to arrive.
@MainActor
final class HeldMachine {
    private(set) var sent: [String] = []
    private var replies: [CheckedContinuation<Revisions?, any Error>] = []
    private var waiting: [CheckedContinuation<Void, Never>] = []

    func send(_ write: ServedWrite, _ requestID: String) async throws -> Revisions? {
        sent.append(requestID)
        return try await withCheckedThrowingContinuation { reply in
            replies.append(reply)
            if !waiting.isEmpty { waiting.removeFirst().resume() }
        }
    }

    func answer(_ revisions: Revisions?) async {
        await nextReply().resume(returning: revisions)
        await settle()
    }

    func fail(_ error: any Error) async {
        await nextReply().resume(throwing: error)
        await settle()
    }

    private func nextReply() async -> CheckedContinuation<Revisions?, any Error> {
        await nextSend()
        return replies.removeFirst()
    }

    /// Waits until a write is waiting for its reply.
    func nextSend() async {
        if replies.isEmpty { await withCheckedContinuation { waiting.append($0) } }
    }

    /// Lets the change's task run to its next suspension.
    func settle() async {
        for _ in 0..<50 { await Task.yield() }
    }
}

/// A change shows in the click and stays until a reading covers the revisions
/// its answer advanced; a refusal returns the row at once.
@MainActor
struct PendingChangesTests {
    private let repo = "repo_11111111111111111111111111111111"
    private let machine = HeldMachine()
    private let notices = NoticeCenter()

    private func changes() -> PendingChanges {
        let machine = machine
        return PendingChanges(send: { try await machine.send($0, $1) })
    }

    /// Three started Tasks, `LOO-1`…`LOO-3`, in order.
    private func plan(_ states: [String] = ["idle", "working"], at planning: Int64? = 4) throws -> Reading<WorkViewPlan> {
        var json = try #require(JSONSerialization.jsonObject(with: loadFixtureData("view_plan.json")) as? [String: Any])
        let members: [[String: Any]] = (1...3).map { n in
            ["source": ["started"], "work": ["repository": repo, "kind": "task", "id": "task_\(n)"]]
        }
        var resolution = try #require(json["resolution"] as? [String: Any])
        resolution["members"] = members
        resolution["unavailable"] = []
        json["resolution"] = resolution
        json["projection"] = ["visible": members, "suppressed": []]
        json["contents"] = members.enumerated().map { index, member in
            ["work": member["work"]!, "title": "Task \(index + 1)", "states": states, "identifier": "LOO-\(index + 1)",
             "working_group": states.contains("auto") ? "shipping" : "active", "graph_step": NSNull(), "primary_session": NSNull(), "location": NSNull(), "argv": NSNull(), "unavailable": [], "open_children": 0, "face": NSNull(),
             "pane_revision": NSNull()]
        }
        let value = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: json))
        return .available(value, readAt: planning.map(at))
    }

    /// A planning row from the shared fixture: `section`, owned when `owner` is set.
    private func task(_ id: String, section: String = "working", wave: String? = "product",
                      owner: String? = "Ada") throws -> RoadmapTask {
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
        let rows = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_history_rows.json"))) as? [[String: Any]])
        var row = rows[0]
        var plan = try #require(row["task"] as? [String: Any])
        plan["id"] = id
        plan["identifier"] = id
        plan["wave_id"] = wave.map { $0 as Any } ?? NSNull()
        row["task"] = plan
        row["section"] = section
        row["lifecycle"] = "idle"
        let ownerJSON: Any = owner.map { person -> Any in
            ["person": person, "person_id": NSNull(), "person_key": NSNull(),
             "machine": ["id": NSNull(), "label": NSNull()]] as [String: Any]
        } ?? NSNull()
        row["runtime"] = ["work_id": id, "status": "idle", "reason": "Running", "updated_at": "2026-10-26T12:00:00Z",
                          "provider": "claude", "completion_pending": NSNull(), "owner": ownerJSON] as [String: Any]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    private func identifiers(_ plan: WorkViewPlan?) -> [String] { plan?.contents.compactMap(\.identifier) ?? [] }

    private func at(_ planning: Int64) -> Revisions { Revisions(["planning": planning, "sessions": 9]) }

    private func complete(_ changes: PendingChanges, _ issue: String) {
        changes.make(.complete(task: issue), subject: issue, verb: "completed",
                     write: .taskComplete(directory: "/repo", issue: issue, children: nil), notices: notices, show: nil)
    }

    @Test func pauseKeepsTheWorkingRowAndARefusalRestoresItsGroup() async throws {
        let changes = changes()
        let row = try task("LOO-2")
        let started = try plan(["open", "working", "moving", "busy", "auto"])
        changes.setPause(.pause, issue: row.task.identifier, directory: "/repo", notices: notices, show: nil) {}
        #expect(changes.controlRequest(for: row.task.identifier) == "Pausing…")
        let held = try #require(changes.apply(to: [row], readAt: at(4)).first)
        #expect(held.section == .working && held.workingGroup == .paused && held.lifecycle == .paused && held.runtime?.owner == row.runtime?.owner)
        #expect(held.selectionStates.contains(.paused) && !held.selectionStates.contains(.moving))
        let contents = try #require(changes.apply(to: started)?.contents)
        #expect(Set(contents[1].states) == [.open, .working, .paused, .busy, .auto]
                && contents[1].workingGroup == .paused)
        #expect(contents[1].status?.word == "Busy")
        #expect(contents[0].states.contains(.working))
        await machine.fail(ServerError(code: "conflict", message: "Task moved", data: .null))
        #expect(changes.apply(to: started)?.contents == started.value?.contents)
        #expect(changes.apply(to: [row], readAt: at(4)) == [row])

        changes.setPause(.unpause, issue: held.task.identifier, directory: "/repo", notices: notices, show: nil) {}
        let released = try #require(changes.apply(to: [held], readAt: at(4)).first)
        #expect(released.section == .working && released.selectionStates.contains(.idle) && !released.selectionStates.contains(.paused))
        let unpaused = changes.apply(to: contents, readAt: at(4))
        #expect(Set(unpaused[1].states) == [.open, .working, .busy, .idle, .auto])
        #expect(unpaused[1].workingGroup == .paused)
        await machine.answer(nil)
    }

    @Test func anAnsweredPauseStaysPendingUntilAReadingShowsIt() async throws {
        let changes = changes()
        changes.read("plan", try plan(at: 4))
        changes.setPause(.pause, issue: "LOO-2", directory: "/repo", notices: notices, show: nil) {}
        await machine.answer(at(5))
        // Between the answer and the reading that shows the hold, the control stays settling.
        #expect(changes.controlRequest(for: "LOO-2") == PendingChanges.pausing)
        changes.read("plan", try plan(at: 5))
        #expect(changes.controlRequest(for: "LOO-2") == nil)
    }

    @Test func completeHidesTheRowUntilACoveringReading() async throws {
        let changes = changes()
        complete(changes, "LOO-2")
        // Before any answer, every reading hides it.
        #expect(identifiers(changes.apply(to: try plan())) == ["LOO-1", "LOO-3"])
        #expect(changes.apply(to: try plan())?.projection.visible.count == 2)
        changes.read("tasks", try plan())
        await machine.answer(Revisions(["planning": 5]))
        // A stale reading still shows the row as the Machine had it: the patch holds.
        changes.read("tasks", try plan())
        #expect(identifiers(changes.apply(to: try plan())) == ["LOO-1", "LOO-3"])
        #expect(changes.changes.count == 1)
        // The covering reading replaces the patch, whatever it says.
        changes.read("tasks", try plan(at: 5))
        #expect(changes.changes.isEmpty)
        #expect(identifiers(changes.apply(to: try plan(at: 5))) == ["LOO-1", "LOO-2", "LOO-3"])
    }

    @Test func aChangeWaitsForEveryLiveReadingOfItsMachine() async throws {
        let changes = changes()
        changes.read("sidebar", try plan())
        changes.read("board", try plan())
        changes.make(.abandon(task: "LOO-1"), subject: "LOO-1", verb: "abandoned",
                     write: .taskAbandon(directory: "/repo", issue: "LOO-1"), notices: notices, show: nil)
        await machine.answer(Revisions(["planning": 5]))
        changes.read("sidebar", try plan(at: 5))
        #expect(changes.changes.count == 1)
        // The board's reading at 4 does not show it yet, the sidebar's at 5 does.
        #expect(identifiers(changes.apply(to: try plan())) == ["LOO-2", "LOO-3"])
        #expect(identifiers(changes.apply(to: try plan(at: 5))).count == 3)
        changes.ended("board")
        #expect(changes.changes.isEmpty)
    }

    @Test func aRefusalReturnsTheRowAndPostsOneNotice() async throws {
        let changes = changes()
        var shown = false
        changes.make(.complete(task: "LOO-3"), subject: "LOO-3", verb: "completed",
                     write: .taskComplete(directory: "/repo", issue: "LOO-3", children: nil), notices: notices,
                     show: Notice.Action("Show") { shown = true })
        #expect(identifiers(changes.apply(to: try plan())) == ["LOO-1", "LOO-2"])
        await machine.fail(ServerError(code: "conflict", message: "its PR #12 is open", data: .null))
        #expect(identifiers(changes.apply(to: try plan())) == ["LOO-1", "LOO-2", "LOO-3"])
        #expect(notices.current?.text == "LOO-3 couldn't be completed: its PR #12 is open")
        #expect(notices.current?.action?.title == "Show")
        notices.current?.action?.perform()
        #expect(shown)
    }

    @Test func aRefusalWhileOtherWritesMovedTheStoreWaitsForTheReading() async throws {
        let changes = changes()
        changes.make(.complete(task: "LOO-1"), subject: "LOO-1", verb: "completed",
                     write: .taskComplete(directory: "/repo", issue: "LOO-1", children: nil), notices: notices, show: nil)
        // Another write's planning sync advanced planning while this one failed.
        await machine.fail(ServerError(code: "outcome_unknown", message: "its PR #12 is open", data: .null,
                                       revisions: Revisions(["planning": 6])))
        changes.read("tasks", try plan(at: 5))
        #expect(identifiers(changes.apply(to: try plan(at: 5))) == ["LOO-2", "LOO-3"])
        #expect(notices.current?.text == "LOO-1 couldn't be completed: its PR #12 is open")
        changes.read("tasks", try plan(at: 6))
        #expect(changes.changes.isEmpty)
    }

    @Test func aCommandThatChangedNothingDropsAtOnce() async throws {
        let changes = changes()
        changes.make(.claim(task: "LOO-1", owner: nil), subject: "LOO-1", verb: "claimed",
                     write: .taskClaim(directory: "/repo", issue: "LOO-1"), notices: notices, show: nil)
        changes.read("tasks", try plan())
        await machine.answer(Revisions([:]))
        #expect(changes.changes.isEmpty)
    }

    @Test func aPendingWorkflowChangeStaysUntilTheTaskStreamCoversItsAnswer() async throws {
        let changes = changes()
        changes.read("planning", Reading.available("plan", readAt: Revisions(["planning": 4, "processes": 4])))
        changes.read("task", Reading.available("work", readAt: Revisions(["planning": 4, "processes": 4])))
        changes.make(.pause(task: "LOO-1"), subject: "LOO-1", verb: "paused",
                     write: .taskPause(directory: "/repo", issue: "LOO-1"), notices: notices, show: nil)
        await machine.answer(Revisions(["planning": 5, "processes": 5]))
        // Planning catches up first; the task stream is still at 4, so the chip keeps "Pausing…" instead of flashing.
        changes.read("planning", Reading.available("plan", readAt: Revisions(["planning": 5, "processes": 5])))
        #expect(changes.controlRequest(for: "LOO-1") == PendingChanges.pausing)
        // Once the task stream covers the answer, the pending change drops.
        changes.read("task", Reading.available("work", readAt: Revisions(["planning": 5, "processes": 5])))
        #expect(changes.controlRequest(for: "LOO-1") == nil)
        #expect(changes.changes.isEmpty)

        // Ending the task stream (switching away from the Task) releases any change held only by it.
        changes.read("task", Reading.available("work", readAt: Revisions(["planning": 5])))
        changes.make(.run(task: "LOO-1"), subject: "LOO-1", verb: "started",
                     write: .taskRun(directory: "/repo", issue: "LOO-1", graph: nil, from: nil, agent: nil, approve: false),
                     notices: notices, show: nil)
        await machine.answer(Revisions(["planning": 6]))
        changes.read("planning", Reading.available("plan", readAt: Revisions(["planning": 6])))
        #expect(changes.controlRequest(for: "LOO-1") == PendingChanges.starting)
        changes.ended("task")
        #expect(changes.changes.isEmpty)
    }

    @Test func sectionChangesFollowTheTableAndMoveLeavesOtherWaves() async throws {
        let changes = changes()
        for (command, issue) in [(PendingCommand.unassign(task: "LOO-1"), "LOO-1"), (.reopen(task: "LOO-2"), "LOO-2"),
                                 (.move(task: "LOO-3", wave: "other"), "LOO-3")] {
            changes.make(command, subject: issue, verb: "changed", write: .taskReopen(directory: "/repo", issue: issue),
                         notices: notices, show: nil)
        }
        let shown = try plan(["idle", "working"], at: nil)
        let patched = changes.apply(to: shown, wave: "product")
        #expect(patched?.contents.first { $0.identifier == "LOO-1" }?.states == [.idle, .backlog])
        #expect(patched?.contents.first { $0.identifier == "LOO-2" }?.states == [.open, .idle, .working])
        #expect(identifiers(patched) == ["LOO-1", "LOO-2"])
        // A list of every Wave keeps a moved Task.
        #expect(identifiers(changes.apply(to: shown)).count == 3)
    }

    @Test func planningRowsTakeEveryCommandSoCountsFollow() async throws {
        let changes = changes()
        let rows = [try task("LOO-1"), try task("LOO-2"), try task("LOO-3", section: "backlog", owner: nil),
                    try task("LOO-4", wave: "other")]
        let working = { (rows: [RoadmapTask]) in rows.filter { $0.section == .working }.map(\.task.identifier) }
        changes.make(.complete(task: "LOO-1"), subject: "LOO-1", verb: "completed",
                     write: .taskComplete(directory: "/repo", issue: "LOO-1", children: nil), notices: notices, show: nil)
        let ada = try JSONDecoder().decode(TaskOwner.self, from: Data(#"{"person":"Ada","person_key":"ada@example.com","person_id":null,"machine":{"id":"home_1","label":"laptop"}}"#.utf8))
        changes.make(.claim(task: "LOO-3", owner: ada), subject: "LOO-3", verb: "claimed",
                     write: .taskClaim(directory: "/repo", issue: "LOO-3"), notices: notices, show: nil)
        changes.make(.unassign(task: "LOO-2"), subject: "LOO-2", verb: "unassigned",
                     write: .taskUnassign(directory: "/repo", issue: "LOO-2"), notices: notices, show: nil)
        changes.make(.move(task: "LOO-4", wave: "product"), subject: "LOO-4", verb: "moved",
                     write: .taskMove(directory: "/repo", issue: "LOO-4", wave: "product", machine: nil), notices: notices, show: nil)
        let product = changes.apply(to: Array(rows.prefix(3)), readAt: at(4), wave: "product", inventory: rows)
        #expect(working(product) == ["LOO-3", "LOO-4"])
        #expect(product.first { $0.task.identifier == "LOO-1" }?.section == .done)
        #expect(product.first { $0.task.identifier == "LOO-2" }?.runtime?.owner == nil)
        #expect(product.first { $0.task.identifier == "LOO-3" }?.runtime?.owner == ada)
        #expect(product.first { $0.task.identifier == "LOO-4" }?.task.waveId == "product")
        #expect(changes.apply(to: [rows[3]], readAt: at(4), wave: "other").isEmpty)
    }

    @Test func workingDragsPreviewImmediatelyAndRetryInOrder() async throws {
        let changes = changes()
        let initial = fixtureWorkingPlan(["a", "b", "c"])
        func drag(_ source: String, before target: String, machine: String) -> Task<Void, Never> {
            let member = WorkWorkspaceKey(repository: initial.repository, kind: .task, id: source)
            return changes.make(.reorder(machine: machine, member: member, target: target, side: .before),
                subject: source, verb: "reordered", write: .desktopDashboardMove(directory: "/repo", board: "working-tasks",
                    member: member.selector, destination: "task:" + target, side: .before), notices: notices, show: nil)
        }
        func order(_ machine: String) -> [String]? {
            changes.workingOrder(initial, machine: machine, readAt: at(4))?.resolution.members.map(\.work.id)
        }
        let first = drag("c", before: "a", machine: "one")
        await machine.fail(URLError(.networkConnectionLost))
        let other = drag("b", before: "a", machine: "two")
        #expect(order("one") == ["c", "a", "b"])
        #expect(order("two") == ["b", "a", "c"])
        let last = drag("b", before: "c", machine: "one")
        #expect(order("one") == ["b", "c", "a"])
        let ids = changes.changes.map(\.requestID)
        for revision in 5...7 { await machine.answer(at(Int64(revision))) }
        for write in [first, other, last] { await write.value }
        #expect(machine.sent.filter { $0 != ids[1] } == [ids[0], ids[0], ids[2]])
        #expect(order("one") == ["b", "c", "a"])
    }

    @Test(arguments: [nil, URLError.networkConnectionLost, .notConnectedToInternet, .cannotConnectToHost,
                      .cannotFindHost, .dnsLookupFailed, .timedOut])
    func aLostLinkSendsAgainUnderTheSameRequestID(_ code: URLError.Code?) async throws {
        let changes = changes()
        complete(changes, "LOO-2")
        if let code {
            await machine.fail(URLError(code))
        } else {
            await machine.fail(RegistryQueryError(ServerLink.disconnected))
        }
        // The change sends again only after it marked the link lost.
        await machine.nextSend()
        #expect(changes.link == .blip)
        #expect(changes.acceptsChanges)
        // The answer goes to the second send, under the first one's request ID.
        await machine.answer(Revisions(["planning": 5]))
        #expect(machine.sent.count == 2)
        #expect(Set(machine.sent).count == 1)
        #expect(changes.link == .connected)
        #expect(changes.changes.first?.answer == Revisions(["planning": 5]))
    }

    @Test(arguments: [URLError.cancelled, .badURL, .serverCertificateUntrusted, .userAuthenticationRequired])
    func aURLRefusalDoesNotResend(_ code: URLError.Code) async throws {
        let changes = changes()
        complete(changes, "LOO-2")
        await machine.fail(URLError(code))
        #expect(changes.changes.isEmpty)
        #expect(machine.sent.count == 1)
        #expect(notices.current?.text.contains("couldn't be completed") == true)
    }

    @Test func requestedShowsInTheClickAndARefusalPostsOverIt() async throws {
        let changes = changes()
        changes.request("Keep theirs", subject: "LOO-5", write: .taskReopen(directory: "/repo", issue: "LOO-5"), notices: notices)
        #expect(notices.current?.text == "LOO-5 Keep theirs requested")
        await machine.fail(ServerError(code: "conflict", message: "it has no checkout", data: .null))
        #expect(notices.current?.text == "LOO-5 Keep theirs couldn't be requested: it has no checkout")
    }

    @Test func autoTurnsInTheClickAndARefusalTurnsItBack() async throws {
        let changes = changes()
        let row = try task("LOO-2")
        changes.make(.auto(task: "LOO-2", on: true), subject: "LOO-2", verb: "put on auto",
                     write: .taskAutoOn(directory: "/repo", issue: "LOO-2"), notices: notices, show: nil)
        #expect(changes.autoRequest(for: "LOO-2") == true && notices.current == nil)
        let shipping = try #require(changes.apply(to: [row], readAt: at(4)).first)
        #expect(shipping.selectionStates.contains(.auto))
        let contents = try #require(changes.apply(to: try plan())?.contents)
        #expect(contents[1].states.contains(.auto) && contents[1].workingGroup == .shipping)
        await machine.fail(ServerError(code: "conflict", message: "it has no checkout", data: .null))
        #expect(changes.autoRequest(for: "LOO-2") == nil)
        #expect(changes.apply(to: [row], readAt: at(4)) == [row])
        #expect(notices.current?.text == "LOO-2 couldn't be put on auto: it has no checkout")
    }

    @Test func revisionsCoverOnlyWhenEveryDomainIsReached() {
        let answer = Revisions(["planning": 5, "sessions": 2])
        #expect(Revisions(["planning": 5, "sessions": 2, "usage": 1]).covers(answer))
        #expect(!Revisions(["planning": 6]).covers(answer))
        #expect(Revisions([:]).covers(Revisions([:])))
    }

    @Test func aReadingNeverGivesWayToOneReadEarlier() throws {
        let newer = try plan(at: 6)
        #expect(!newer.yields(to: try plan(at: 5)))
        #expect(try plan(at: 5).yields(to: newer))
        #expect(newer.yields(to: newer.failed("offline")))
    }

    @Test func aNewEventReplacesTheLastAndARefreshedOneRestarts() {
        let center = NoticeCenter()
        center.post(Notice("LOO-5 completed"))
        center.post(Notice("Tasks couldn't be read: offline", key: "task_list"))
        #expect(center.current?.text == "Tasks couldn't be read: offline")
        let posted = center.posted
        center.post(Notice("Tasks couldn't be read: offline", key: "task_list"))
        #expect(center.posted == posted + 1)
        center.clear(key: "task_list")
        #expect(center.current == nil)
    }

    @Test func aReadPostsOnlyWhenItTurnsIntoAFailureOverALiveLink() {
        let center = NoticeCenter()
        center.read("sessions", "Sessions", from: nil, to: "offline", link: .blip)
        #expect(center.current == nil)
        center.read("sessions", "Sessions", from: nil, to: "offline", link: .connected)
        #expect(center.current?.text == "Sessions couldn't be read: offline")
        let posted = center.posted
        center.read("sessions", "Sessions", from: "offline", to: "still offline", link: .connected)
        #expect(center.posted == posted)
        center.read("sessions", "Sessions", from: "still offline", to: nil, link: .connected)
        #expect(center.current == nil)
    }

    @Test func aFailureThatOutlastsALostLinkPostsOnceItReturns() {
        let center = NoticeCenter()
        center.read("sessions", "Sessions", from: nil, to: "offline", link: .away)
        center.read("sessions", "Sessions", from: "offline", to: "offline", link: .away)
        #expect(center.current == nil)
        center.read("sessions", "Sessions", from: "offline", to: "still offline", link: .connected)
        #expect(center.current?.text == "Sessions couldn't be read: still offline")
        let posted = center.posted
        center.read("sessions", "Sessions", from: "still offline", to: "still offline", link: .connected)
        #expect(center.posted == posted)
        center.read("sessions", "Sessions", from: "still offline", to: nil, link: .connected)
        #expect(center.current == nil)
        center.read("sessions", "Sessions", from: nil, to: nil, link: .connected)
        #expect(center.posted == posted)
    }

    @Test func aConditionStandsUnderEventsUntilItEnds() {
        let center = NoticeCenter()
        center.post(Notice("The Machine is away", key: "away", kind: .condition))
        center.post(Notice("LOO-2 couldn't be completed: offline"))
        #expect(center.current?.text == "LOO-2 couldn't be completed: offline")
        center.expire(center.posted)
        #expect(center.current?.text == "The Machine is away")
        center.post(Notice("The Machine is away", key: "away", kind: .condition))
        #expect(center.conditions.count == 1)
        center.clear(key: "away")
        #expect(center.current == nil)
    }
}
