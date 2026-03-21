// UI tests for WaveRow view.

import SwiftUI
import Testing
import ViewInspector
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("Wave Row View")
struct WaveRowViewTests {
    private func makeWave(
        name: String = "swift-falcon",
        area: [String] = ["src/"],
        flow: String = "build",
        triggers: [Trigger] = [],
        hasDiff: Bool = false,
        diffStat: String? = nil
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: "test-wave-id",
                name: name,
                repo: "/tmp/repo",
                flow: flow,
                area: area,
                triggers: triggers,
                status: .idle,
                iteration: 0,
                diffStat: diffStat
            ),
            hasDiff: hasDiff
        )
    }

    private func makeRow(
        wave: WaveViewModel,
        isSelected: Bool = false
    ) -> WaveRow {
        WaveRow(
            wave: wave,
            isSelected: isSelected,
            onSelect: {},
            isEditingAnyName: .constant(false)
        )
    }

    @Test("Row shows wave display name")
    func showsDisplayName() throws {
        let wave = makeWave(name: "aurora-melody")
        let row = makeRow(wave: wave)

        let nameText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-name").text()
        #expect(try nameText.string() == "aurora-melody")
    }

    @Test("Row shows diff indicator when wave has diff")
    func showsDiffIndicator() throws {
        let wave = makeWave(hasDiff: true, diffStat: "3 files changed, 42 insertions(+), 8 deletions(-)")
        let row = makeRow(wave: wave)

        let diffText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-diff").text()
        #expect(try diffText.string() == "+42 −8")
    }

    @Test("Row hides diff indicator when no diff")
    func hidesDiffWhenNoDiff() throws {
        let wave = makeWave(hasDiff: false)
        let row = makeRow(wave: wave)

        #expect(throws: Error.self) {
            try row.inspect().find(viewWithAccessibilityIdentifier: "wave-diff")
        }
    }
}
