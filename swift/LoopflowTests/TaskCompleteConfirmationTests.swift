#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor @Suite struct TaskCompleteConfirmationTests {
    @Test func theDialogNamesWhereChildrenMoveAndWhatAbandoningDoes() throws {
        let parent = try roadmapTask(condition: "clear", reason: "Merged · 2 children open")
        let id = parent.task.identifier
        #expect(TaskChildPolicy.question(issue: id, openChildren: 1) == "Complete \(id)? 1 child is open")
        #expect(TaskChildPolicy.question(issue: id, openChildren: 3) == "Complete \(id)? 3 children are open")
        #expect(TaskCompleteConfirmation.moveLabel(to: parent) == "Complete & Move Children to \(id)")
        #expect(TaskCompleteConfirmation.moveLabel(to: nil) == "Complete & Move Children to Top Level")

        let unmerged = try roadmapTask(condition: "clear", reason: "Open")
        let message = TaskCompleteConfirmation.message(descendants: [unmerged], destination: nil)
        #expect(message.contains("Move keeps its children open at the top level."))
        #expect(message.contains("Abandoning closes PRs and deletes branches for \(unmerged.task.identifier)"))
        #expect(!message.contains("cannot be reopened"))
    }
}
#endif
