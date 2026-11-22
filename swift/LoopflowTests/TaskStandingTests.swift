#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Where a Task stands")
struct TaskStandingTests {
    @Test("With no program reporting, a blocked or waiting Task says so and gives the reason", arguments: [
        ("blocked", "Blocked"), ("waiting", "Waiting"),
    ])
    func conditionNeedsSomeone(state: String, word: String) throws {
        let standing = TaskStanding(task: try task(condition: state, reason: "merge the PR on GitHub"), status: .unknown)
        #expect(standing.word == word)
        #expect(standing.tone == .warning)
        #expect(standing.line == "merge the PR on GitHub")
    }

    @Test("A Task nobody has started is not waiting on anyone: its word is Unstarted, with no line")
    func unstartedIsNotWaiting() throws {
        let standing = TaskStanding(task: try task(condition: "waiting", reason: "ready", lifecycle: "unstarted"), status: .unknown)
        #expect(standing.word == "Unstarted")
        #expect(standing.tone == nil)
        #expect(standing.line == nil)
        let blocked = TaskStanding(task: try task(condition: "blocked", reason: "the next launch is misconfigured", lifecycle: "unstarted"),
                                   status: .unknown)
        #expect(blocked.word == "Blocked" && blocked.line == "the next launch is misconfigured")
    }

    @Test("A clear Task shows its lifecycle word and no line")
    func clearShowsLifecycle() throws {
        let standing = TaskStanding(task: try task(condition: "clear", reason: "ready", lifecycle: "idle"), status: .unknown)
        #expect(standing.word == "Idle")
        #expect(standing.tone == nil)
        #expect(standing.line == nil)
    }

    @Test("A condition that could not be read adds no word, and still says why")
    func unknownAddsNoWord() throws {
        let standing = TaskStanding(task: try task(condition: "unknown", reason: "checkout unreadable", lifecycle: "active"),
                                    status: .unknown)
        #expect(standing.word == "Active")
        #expect(standing.line == "checkout unreadable")
    }

    @Test("A finished Task with no completion date says the date is unavailable")
    func undatedDone() throws {
        let standing = TaskStanding(task: try task(condition: "clear", reason: "complete", lifecycle: "done"), status: .unknown)
        #expect(standing.word == "Done, date unavailable")
    }

    @Test("A program's report is the word, over the condition; a blocked program's ask is the line")
    func programReportWins() throws {
        let waiting = try task(condition: "waiting", reason: "merge the PR on GitHub")
        let busy = TaskStanding(task: waiting, status: try reported(.working, msg: nil))
        #expect(busy.word == "Busy")
        #expect(busy.tone == .running)
        // The Task still needs someone, whatever its program is doing.
        #expect(busy.line == "merge the PR on GitHub")

        let asking = TaskStanding(task: waiting, status: try reported(.blocked, msg: "Approve the plan?"))
        #expect(asking.word == "Blocked")
        #expect(asking.line == "Approve the plan?")
    }

    @Test("Unreadable Session readings give a reason and no word; the condition's line stays")
    func unavailableReadings() throws {
        let standing = TaskStanding(task: try task(condition: "blocked", reason: "setup failed"), status: .unavailable("Offline"))
        #expect(standing.word == nil)
        #expect(standing.help?.contains("Offline") == true)
        #expect(standing.line == "setup failed")
    }

    @Test("A row named before planning answers says nothing about where it stands")
    func noTask() {
        let standing = TaskStanding(task: nil, status: .unknown)
        #expect(standing.word == nil)
        #expect(standing.line == nil)
    }

    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private func task(condition: String, reason: String, lifecycle: String = "idle") throws -> RoadmapTask {
        let wire = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))) as? [String: Any])
        let tasks = try #require((wire["tasks"] as? [String: Any])?["items"] as? [[String: Any]])
        var row = try #require(tasks.first)
        var state = try #require(row["condition"] as? [String: Any])
        state["state"] = condition
        state["reason"] = reason
        row["condition"] = state
        row["lifecycle"] = lifecycle
        var planning = try #require(row["task"] as? [String: Any])
        planning["completed_at"] = NSNull()
        row["task"] = planning
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    private func reported(_ state: ProgramStatusState, msg: String?) throws -> SessionProgramStatus {
        let report = ProgramStatusReport(state: state, id: nil, kind: nil, progress: nil, app: nil, title: nil, msg: msg)
        var value = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("session.json"))) as? [String: Any])
        value["program_status"] = try JSONSerialization.jsonObject(
            with: JSONEncoder().encode(ProgramStatusRecords(seen: true, records: [report])))
        let session = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
        return SessionProgramStatus(sessions: [session], unavailable: nil)
    }
}
#endif
