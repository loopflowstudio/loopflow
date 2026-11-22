import Testing
@testable import Loopflow

struct TaskStatusTests {
    private func content(_ states: [WorkSelectionState], session: String? = "session", unavailable: [String] = []) -> WorkViewPlan.Content {
        WorkViewPlan.Content(work: .init(repository: "repo", kind: .task, id: "task"), title: "Task",
            states: states, identifier: nil, primarySession: session, location: nil, argv: nil, unavailable: unavailable)
    }

    @Test("A row says what the Mac row says: the conversation's report, else the lifecycle")
    func oneVocabulary() {
        #expect(content([.working, .busy, .blocked]).status == .program(.blocked))
        #expect(content([.working, .busy, .available]).status?.word == ProgramStatusState.working.label)
        #expect(content([.working, .available]).status?.word == ProgramStatusState.idle.label)
        #expect(content([.working, .idle, .auto]).status == .lifecycle(.idle))
        #expect(content([.working, .moving]).status?.word == "Moving")
        // A Task with no conversation says its lifecycle too.
        #expect(content([.backlog, .unstarted], session: nil).status?.word == "Unstarted")
        #expect(content([.working]).status == nil)
    }

    @Test func anUnavailableReadingSaysSoWithItsReasons() {
        let status = content([.busy], unavailable: ["gone"]).status
        #expect(status?.word == "Unavailable")
        #expect(status?.detail == "gone")
    }
}
