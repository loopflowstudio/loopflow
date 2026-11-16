import Foundation
import Loopflow
import Testing

/// A change shows in the click and stays until a reading covers the revisions
/// its answer advanced; a refusal returns the row at once.
@MainActor
struct PendingChangesTests {
    private let repo = "repo_11111111111111111111111111111111"

    /// Three started Tasks, `LOO-1`…`LOO-3`, in order.
    private func plan(_ states: [String] = ["idle", "working"]) throws -> WorkViewPlan {
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
             "primary_session": NSNull(), "location": NSNull(), "argv": NSNull(), "unavailable": [], "face": NSNull(),
             "pane_revision": NSNull()]
        }
        return try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: json))
    }

    private func identifiers(_ plan: WorkViewPlan) -> [String] { plan.contents.compactMap(\.identifier) }

    private func at(_ planning: Int64) -> Revisions { Revisions(["planning": planning, "sessions": 9]) }

    @Test func completeHidesTheRowUntilACoveringReading() throws {
        let changes = PendingChanges()
        let shown = try plan()
        let id = changes.add(.complete(task: "LOO-2"))
        // Before any answer, every reading hides it.
        #expect(identifiers(changes.apply(to: shown, readAt: at(4))) == ["LOO-1", "LOO-3"])
        #expect(changes.apply(to: shown, readAt: at(4)).projection.visible.count == 2)
        changes.read("tasks", at: at(4))
        changes.answered(id, revisions: Revisions(["planning": 5]))
        // A stale reading still shows the row as the Machine had it: the patch holds.
        changes.read("tasks", at: at(4))
        #expect(identifiers(changes.apply(to: shown, readAt: at(4))) == ["LOO-1", "LOO-3"])
        #expect(changes.changes.count == 1)
        // The covering reading replaces the patch, whatever it says.
        changes.read("tasks", at: at(5))
        #expect(changes.changes.isEmpty)
        #expect(identifiers(changes.apply(to: shown, readAt: at(5))) == ["LOO-1", "LOO-2", "LOO-3"])
    }

    @Test func aChangeWaitsForEveryLiveReadingOfItsMachine() throws {
        let changes = PendingChanges()
        changes.read("sidebar", at: at(4))
        changes.read("board", at: at(4))
        let id = changes.add(.abandon(task: "LOO-1"))
        changes.answered(id, revisions: Revisions(["planning": 5]))
        changes.read("sidebar", at: at(5))
        #expect(changes.changes.count == 1)
        // The board's reading at 4 does not show it yet, the sidebar's at 5 does.
        #expect(changes.removes(task: "LOO-1", at: at(4)))
        #expect(!changes.removes(task: "LOO-1", at: at(5)))
        changes.ended("board")
        #expect(changes.changes.isEmpty)
    }

    @Test func aRefusalReturnsTheRowAtOnce() throws {
        let changes = PendingChanges()
        let id = changes.add(.complete(task: "LOO-3"))
        #expect(identifiers(changes.apply(to: try plan(), readAt: at(4))) == ["LOO-1", "LOO-2"])
        changes.refused(id)
        #expect(identifiers(changes.apply(to: try plan(), readAt: at(4))) == ["LOO-1", "LOO-2", "LOO-3"])
    }

    @Test func aPartlyDoneChangeIsKeptUntilCovered() throws {
        let changes = PendingChanges()
        let id = changes.add(.abandon(task: "LOO-1"))
        // Abandon saved, then failed to delete its branch: the answer still advanced planning.
        changes.answered(id, revisions: Revisions(["planning": 6]))
        changes.read("tasks", at: at(5))
        #expect(identifiers(changes.apply(to: try plan(), readAt: at(5))) == ["LOO-2", "LOO-3"])
        changes.read("tasks", at: at(6))
        #expect(changes.changes.isEmpty)
    }

    @Test func aCommandThatChangedNothingDropsAtTheNextReading() throws {
        let changes = PendingChanges()
        let id = changes.add(.claim(task: "LOO-1"))
        changes.read("tasks", at: at(4))
        changes.answered(id, revisions: Revisions([:]))
        #expect(changes.changes.isEmpty)
    }

    @Test func sectionChangesFollowTheTableAndMoveLeavesOtherWaves() throws {
        let changes = PendingChanges()
        changes.add(.unassign(task: "LOO-1"))
        changes.add(.reopen(task: "LOO-2"))
        changes.add(.move(task: "LOO-3", wave: "other"))
        let shown = try plan(["idle", "working"])
        let patched = changes.apply(to: shown, readAt: nil, wave: "product")
        #expect(patched.contents.first { $0.identifier == "LOO-1" }?.states == [.idle, .backlog])
        #expect(patched.contents.first { $0.identifier == "LOO-2" }?.states == [.open, .idle, .working])
        #expect(identifiers(patched) == ["LOO-1", "LOO-2"])
        // A list of every Wave keeps a moved Task.
        #expect(identifiers(changes.apply(to: shown, readAt: nil)).count == 3)
    }

    @Test func unansweredChangesAreResentUnderTheirRequestIDs() {
        let changes = PendingChanges()
        let first = changes.add(.claim(task: "LOO-1"))
        let second = changes.add(.complete(task: "LOO-2"))
        changes.answered(second, revisions: Revisions(["planning": 3]))
        #expect(changes.unanswered.map(\.requestID) == [first])
    }

    @Test func revisionsCoverOnlyWhenEveryDomainIsReached() {
        let answer = Revisions(["planning": 5, "sessions": 2])
        #expect(Revisions(["planning": 5, "sessions": 2, "usage": 1]).covers(answer))
        #expect(!Revisions(["planning": 6]).covers(answer))
        #expect(Revisions([:]).covers(Revisions([:])))
    }

    @Test func aNewNoticeReplacesTheLastAndARefreshedOneRestarts() {
        let center = NoticeCenter()
        center.post(Notice("LOO-5 Auto-Ship requested"))
        center.post(Notice("Tasks couldn't be read: offline", key: "task_list"))
        #expect(center.current?.text == "Tasks couldn't be read: offline")
        let posted = center.posted
        center.post(Notice("Tasks couldn't be read: offline", key: "task_list"))
        #expect(center.posted == posted + 1)
        center.clear(key: "task_list")
        #expect(center.current == nil)
    }
}
