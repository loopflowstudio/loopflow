// UI tests for WaveRow view.

import SwiftUI
import Testing
import ViewInspector
@testable import LoopflowMac
@testable import Loopflow

@MainActor
@Suite("Wave Row View")
struct WaveRowViewTests {
    private func makeWave(
        name: String = "swift-falcon",
        status: WaveStatus = .idle
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: "test-wave-id",
                name: name,
                repo: "/tmp/repo",
                status: status
            )
        )
    }

    private func makeRow(
        wave: WaveViewModel,
        isSelected: Bool = false
    ) -> WaveRow {
        WaveRow(
            wave: wave,
            isSelected: isSelected,
            onSelect: {}
        )
    }

    @Test("Row shows wave display name")
    func showsDisplayName() throws {
        let wave = makeWave(name: "aurora-melody")
        let row = makeRow(wave: wave)

        let nameText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-name").text()
        #expect(try nameText.string() == "aurora-melody")
    }

    @Test("Row shows Wave lifecycle status")
    func showsStatus() throws {
        let wave = makeWave(status: .running)
        let row = makeRow(wave: wave)

        let status = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-status").text()
        #expect(try status.string() == "Running")
    }
}
