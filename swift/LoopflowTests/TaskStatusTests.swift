import Testing
@testable import Loopflow

struct TaskStatusTests {
    private func content(_ states: [WorkSelectionState], session: String? = "session", unavailable: [String] = []) -> WorkViewPlan.Content {
        WorkViewPlan.Content(work: .init(repository: "repo", kind: .task, id: "task"), title: "Task",
            states: states, identifier: nil, graphStep: nil, primarySession: session, location: nil, argv: nil, unavailable: unavailable,
            openChildren: nil)
    }

    @Test("A row says what the Mac row says: the conversation's report, else the lifecycle")
    func oneVocabulary() {
        #expect(content([.working, .error]).status == .program(.error))
        #expect(content([.working, .error, .busy]).status == .program(.error))
        #expect(content([.working, .error, .busy, .blocked]).status == .program(.blocked))
        #expect(content([.working, .busy, .available]).status?.word == ProgramStatusState.working.label)
        #expect(content([.working, .available]).status?.word == ProgramStatusState.idle.label)
        #expect(content([.working, .idle, .auto]).status == .lifecycle(.idle))
        #expect(content([.working, .moving]).status?.word == "Moving")
        // A Task with no conversation says its lifecycle too, as the Machine reads it.
        #expect(content([.unstarted, .backlog], session: nil, unavailable: [
            "No primary Session is recorded", "Recorded execution location is unavailable",
        ]).status?.word == "Unstarted")
        #expect(content([.working]).status == nil)
    }

    @Test func aReadingWithNoStateSaysUnavailableWithItsReasons() {
        let status = content([.working], unavailable: ["gone"]).status
        #expect(status?.word == "Unavailable")
        #expect(status?.detail == "gone")
    }
}
