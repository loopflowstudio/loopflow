import Testing
@testable import Loopflow

struct TaskStatusTests {
    private func content(_ states: [WorkSelectionState], session: String? = "session", unavailable: [String] = []) -> WorkViewPlan.Content {
        WorkViewPlan.Content(work: .init(repository: "repo", kind: .task, id: "task"), title: "Task",
            states: states, identifier: nil, primarySession: session, location: nil, argv: nil, unavailable: unavailable)
    }

    @Test func theMostUrgentStateNamesTheTask() {
        #expect(content([.working, .busy, .blocked]).status.label == "Blocked")
        #expect(content([.working, .busy, .available]).status.label == "Running")
        #expect(content([.working, .available]).status.label == "Ready")
        #expect(content([.working, .auto]).status.label == "Open")
        #expect(content([.working]).status.label == "Open")
    }

    @Test func noConversationOrAnUnavailableReadingSaySo() {
        #expect(content([.blocked], session: nil).status.label == "No conversation yet")
        #expect(content([.busy], unavailable: ["gone"]).status.label == "Unavailable")
    }
}
