#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("New Wave sheet")
@MainActor
struct CreateWaveSheetTests {
    @Test func offersAnOptionalObjectiveBesideTheName() throws {
        let view = CreateWaveSheet(
            repos: [PortfolioRepo(path: "/repo", error: nil)], initialRepoPath: "/repo",
            onCreate: { _, _, _ in try added() }, onCancel: {})
        let objective = try view.inspect().find(viewWithAccessibilityIdentifier: "new-wave-objective")
        #expect(try objective.textField().labelView().text().string() == "What this Wave is for")
        #expect(try view.inspect().find(text: "Objective (optional)").string() == "Objective (optional)")
    }

    @Test func leftoversStayOpenWithTheRustLineAndADoneButton() throws {
        let receipt = try added()
        var done = false
        let view = WaveAddedLeftovers(added: receipt) { done = true }
        #expect(try view.inspect().find(text: "Added research").string() == "Added research")
        let summary = try view.inspect().find(viewWithAccessibilityIdentifier: "new-wave-summary")
        #expect(try summary.find(text: receipt.summary[0]).string() == receipt.summary[0])
        try view.inspect().find(viewWithAccessibilityIdentifier: "new-wave-done").button().tap()
        #expect(done)
    }
}

private func added() throws -> WaveAdded {
    try RegistryQuery.decode(
        WaveAdded.self, from: String(decoding: loadFixtureData("wave_added.json"), as: UTF8.self))
}
#endif
