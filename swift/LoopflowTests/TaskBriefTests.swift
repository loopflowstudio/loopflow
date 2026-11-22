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

    @Test @MainActor func outlineListsAcceptanceAndHeadings() {
        #expect(TaskBrief.outline(Self.brief).map(\.title) == ["Acceptance", "Exclusions"])
        #expect(TaskBrief.outline("Just one paragraph.").isEmpty)
    }
}
#endif
