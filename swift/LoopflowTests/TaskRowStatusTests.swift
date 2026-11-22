#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Where a Task stands")
struct TaskRowStatusTests {
    @Test("With no program reporting, a blocked or waiting Task says so and gives the reason", arguments: [
        ("blocked", "Blocked"), ("waiting", "Waiting"),
    ])
    func conditionNeedsSomeone(state: String, word: String) throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: state, reason: "merge the PR on GitHub"))
        #expect(standing.word == word)
        #expect(standing.tone == .warning)
        #expect(standing.line == "merge the PR on GitHub")
    }

    @Test("A Task whose Graph has never run is not waiting on anyone, opened or not: its lifecycle word, with no line",
          arguments: [("unstarted", "Unstarted"), ("idle", "Idle")])
    func awaitingItsFirstRunIsNotWaiting(lifecycle: String, word: String) throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: "waiting", reason: "ready", lifecycle: lifecycle,
                                                          recommended: "run", graphHasRun: false))
        #expect(standing.word == word)
        #expect(standing.tone == nil)
        #expect(standing.line == nil)
        let blocked = TaskRowStatus(task: try roadmapTask(condition: "blocked", reason: "the next launch is misconfigured",
                                                         lifecycle: lifecycle, graphHasRun: false))
        #expect(blocked.word == "Blocked" && blocked.line == "the next launch is misconfigured")
    }

    @Test("A finished Task waits for nobody, whatever its condition says")
    func finishedIsNotWaiting() throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: "waiting", reason: "ready", lifecycle: "abandoned"))
        #expect(standing.word == "Abandoned")
        #expect(standing.line == nil)
    }

    @Test("A clear Task shows its lifecycle word and no line")
    func clearShowsLifecycle() throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: "clear", reason: "ready", lifecycle: "idle"))
        #expect(standing.word == "Idle")
        #expect(standing.tone == nil)
        #expect(standing.line == nil)
    }

    @Test("A condition that could not be read adds no word, and still says why")
    func unknownAddsNoWord() throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: "unknown", reason: "checkout unreadable", lifecycle: "active"))
        #expect(standing.word == "Active")
        #expect(standing.line == "checkout unreadable")
    }

    @Test("A finished Task with no completion date says the date is unavailable")
    func undatedDone() throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: "clear", reason: "complete", lifecycle: "done"))
        #expect(standing.word == "Done, date unavailable")
    }

    @Test("A program's report is the word, over the condition; a blocked program's ask is the line")
    func programReportWins() throws {
        let waiting = try roadmapTask(condition: "waiting", reason: "merge the PR on GitHub")
        let busy = TaskRowStatus(task: waiting, status: try programStatus(reporting: .working, msg: nil))
        #expect(busy.word == "Busy")
        #expect(busy.tone == .running)
        // The Task still needs someone, whatever its program is doing.
        #expect(busy.line == "merge the PR on GitHub")

        let asking = TaskRowStatus(task: waiting, status: try programStatus(reporting: .blocked, msg: "Approve the plan?"))
        #expect(asking.word == "Blocked")
        #expect(asking.line == "Approve the plan?")
    }

    @Test("Unreadable Session readings give a reason and no word; the condition's line stays")
    func unavailableReadings() throws {
        let standing = TaskRowStatus(task: try roadmapTask(condition: "blocked", reason: "setup failed"), status: .unavailable("Offline"))
        #expect(standing.word == nil)
        #expect(standing.detail?.contains("Offline") == true)
        #expect(standing.line == "setup failed")
    }
}
#endif
