import Foundation
import Testing
@testable import Loopflow

@Suite("Loopflow state")
struct LoopflowStateTests {
    @Test("selected repository round-trips")
    func selectedRepositoryRoundTrips() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathComponent("loopflow-state.yaml")

        try saveLoopflowState(
            LoopflowState(selectedRepoPath: "/Users/jack/src/a \"quoted\" repo"),
            stateURL: url
        )

        #expect(
            loadLoopflowState(stateURL: url)?.selectedRepoPath
                == "/Users/jack/src/a \"quoted\" repo"
        )
    }

    @Test("missing state has no selection")
    func missingStateHasNoSelection() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathComponent("missing.yaml")

        #expect(loadLoopflowState(stateURL: url) == nil)
    }
}
