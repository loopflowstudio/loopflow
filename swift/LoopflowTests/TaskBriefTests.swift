#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Task brief")
struct TaskBriefTests {
    static let brief = """
    Make the views useful beside the conversation.

    **Acceptance:**

    * The brief is readable.

    Build on [LOO-489 · Multitask](<https://linear.app/x>), LOO-497 and LOO-416; LOO-402 owns attention. See LOO-489 again.
    This is LOO-515 itself.

    ## Exclusions

    Nothing else.
    """

    @Test func relatedTasksAreTheIdentifiersTheBriefNamesExceptItself() {
        #expect(TaskBrief.related(in: Self.brief, excluding: "LOO-515") == ["LOO-489", "LOO-497", "LOO-416", "LOO-402"])
        #expect(TaskBrief.related(in: "", excluding: "LOO-515").isEmpty)
    }

    @Test func stateAndReasonAreOneSentence() {
        let state = TaskBrief.state(runtime: "moving", step: "build", reason: "The loop chose another pass on the concept review")
        #expect(state.lead == "Moving, at build.")
        #expect(state.reason == "The loop chose another pass on the concept review.")
        #expect(TaskBrief.state(runtime: nil, step: nil, reason: "").lead == "Unstarted.")
    }

    @Test func relatedTasksAndOutlineAreCommaSeparatedLinks() throws {
        let one = try #require(URL(string: "loopflow://loopflow/task/LOO-497"))
        let two = try #require(URL(string: "loopflow://loopflow/task/LOO-416"))
        let links = TaskBrief.links([("LOO-497", one), ("LOO-416", two)], mono: true)
        #expect(String(links.characters) == "LOO-497, LOO-416")
        #expect(links.runs.compactMap(\.link) == [one, two])
    }

    @Test @MainActor func outlineListsAcceptanceAndHeadings() {
        #expect(TaskBrief.outline(Self.brief).map(\.title) == ["Acceptance", "Exclusions"])
        #expect(TaskBrief.outline("Just one paragraph.").isEmpty)
    }
}
#endif
