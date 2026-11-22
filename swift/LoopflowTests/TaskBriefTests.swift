#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
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

    @Test func stateNamesTheTopLevelStepTheBarMakesStrong() throws {
        let steps: [TaskGraph.Step] = [.init(name: "plan", interactive: false), .init(name: "build", interactive: false),
                                       .init(name: "ship-review", interactive: true)]
        let building = TaskGraph(name: "code", interactiveSteps: [],
            position: .run(step: "plan", at: "implement", lfProcessId: "p", running: true),
            steps: steps, currentStep: 1, sentence: "build is running implement.")
        let moving = try roadmapTask(condition: "clear", reason: "ready", lifecycle: "moving",
                                     nextMove: "The loop chose another pass on the concept review")
        let state = TaskBrief.state(moving, graph: building)
        #expect(state.lead == "Moving, at build.")
        #expect(state.reason == "The loop chose another pass on the concept review.")
        // Parked at a step it waits on someone: the row's word, not the runtime's.
        let parked = try roadmapTask(condition: "waiting", reason: "ready", lifecycle: "active", recommended: "run")
        #expect(TaskBrief.state(parked, graph: building).lead == "Waiting, at build.")
        let unstarted = TaskGraph(name: "code", interactiveSteps: [], position: .at(step: TaskGraph.start),
            steps: steps, currentStep: nil, sentence: "Not started.")
        let fresh = try roadmapTask(condition: "waiting", reason: "ready", lifecycle: "unstarted", recommended: "run",
                                    nextMove: "", graphHasRun: false)
        #expect(TaskBrief.state(fresh, graph: unstarted).lead == "Unstarted.")
        #expect(TaskBrief.state(fresh, graph: nil) == ("Unstarted.", ""))
    }

    @Test("The Brief says what the Task needs once: not again when the state line ends with it")
    func needIsSaidOnce() throws {
        #expect(TaskBrief.need(try roadmapTask(condition: "waiting", reason: "ready", nextMove: "ready")) == nil)
        #expect(TaskBrief.need(try roadmapTask(condition: "blocked", reason: "checkout has uncommitted changes", nextMove: "ready"))
            == "Blocked: checkout has uncommitted changes")
        #expect(TaskBrief.need(try roadmapTask(condition: "clear", reason: "ready", nextMove: "ready")) == nil)
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
