import Testing

@testable import Loopflow

@Suite("Work Census")
struct WorkCensusTests {
    @Test("Optional Launch identity is the complete openability contract")
    func launchIdentityControlsOpenability() {
        let openable = activity(launchId: "launch-1")
        let viewOnly = activity(launchId: nil)

        #expect(openable.isOpenable)
        #expect(openable.launchId == "launch-1")
        #expect(!viewOnly.isOpenable)
        #expect(viewOnly.launchId == nil)
    }

    private func activity(launchId: String?) -> WorkActivity {
        WorkActivity(
            id: "task-1",
            kind: .task,
            title: "INF-123",
            subtitle: nil,
            parentRowId: nil,
            provider: nil,
            model: nil,
            home: nil,
            worktree: nil,
            step: nil,
            ageSecs: nil,
            reason: nil,
            nextOwner: nil,
            tint: .neutral,
            evidence: .observed,
            launchId: launchId
        )
    }
}
