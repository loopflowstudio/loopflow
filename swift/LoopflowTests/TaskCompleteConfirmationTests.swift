#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor @Suite struct TaskCompleteConfirmationTests {
    @Test func theSheetNamesWhereChildrenMoveAndWhatSettlingDoes() throws {
        let parent = try roadmapTask(condition: "clear", reason: "Merged · 2 children open")
        let id = parent.task.identifier
        #expect(TaskChildPolicy.question(issue: id, openChildren: 1) == "Complete \(id)? 1 child is open")
        #expect(TaskChildPolicy.question(issue: id, openChildren: 3) == "Complete \(id)? 3 children are open")
        #expect(TaskCompleteConfirmation.moveTitle(to: parent) == "Move children to \(id)")
        #expect(TaskCompleteConfirmation.moveTitle(to: nil) == "Move children to top level")
        #expect(TaskCompleteConfirmation.moveDetail(to: parent).contains("under \(id)"))
        #expect(TaskCompleteConfirmation.moveDetail(to: nil).contains("at the top level"))

        let unmerged = try roadmapTask(condition: "clear", reason: "Open")
        #expect(TaskCompleteConfirmation.defaultPolicy(descendants: [unmerged]) == .rehome)
        let detail = TaskCompleteConfirmation.settleDetail(descendants: [unmerged])
        #expect(detail.contains("Closes PRs and deletes branches for \(unmerged.task.identifier)"))
        #expect(!detail.contains("cannot be reopened"))

        #expect(TaskCompleteConfirmation.confirmLabel(policy: .rehome, allMerged: false) == "Complete & Move")
        #expect(TaskCompleteConfirmation.confirmRole(policy: .rehome, allMerged: false) == .success)
        #expect(TaskCompleteConfirmation.confirmLabel(policy: .abandon, allMerged: false) == "Complete & Abandon")
        #expect(TaskCompleteConfirmation.confirmRole(policy: .abandon, allMerged: false) == .destructive)
        #expect(TaskCompleteConfirmation.confirmLabel(policy: .abandon, allMerged: true) == "Complete & Finish")
        #expect(TaskCompleteConfirmation.confirmRole(policy: .abandon, allMerged: true) == .success)
    }
}
#endif
