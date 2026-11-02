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
            onCreate: { _, _, _ in }, onCancel: {})
        let objective = try view.inspect().find(viewWithAccessibilityIdentifier: "new-wave-objective")
        #expect(try objective.textField().labelView().text().string() == "What this Wave is for")
        #expect(try view.inspect().find(text: "Objective (optional)").string() == "Objective (optional)")
    }
}
#endif
