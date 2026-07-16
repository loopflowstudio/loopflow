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
        status: WaveStatus = .idle,
        live: Bool = false,
        activeTasks: Int = 0,
        activeProjects: Int = 0
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: "test-wave-id",
                name: name,
                repo: "/tmp/repo",
                status: status,
                live: live,
                activeTasks: activeTasks,
                activeProjects: activeProjects
            )
        )
    }

    private func makeRow(
        wave: WaveViewModel,
        isSelected: Bool = false,
        indentLevel: Int = 0
    ) -> WaveRow {
        WaveRow(
            wave: wave,
            isSelected: isSelected,
            onSelect: {},
            indentLevel: indentLevel
        )
    }

    @Test("Row shows wave display name")
    func showsDisplayName() throws {
        let wave = makeWave(name: "aurora-melody")
        let row = makeRow(wave: wave)

        let nameText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-name").text()
        #expect(try nameText.string() == "aurora-melody")
    }

    @Test("Row renders an operational lens")
    func showsLens() throws {
        let wave = makeWave(status: .running, live: true)
        let row = makeRow(wave: wave)

        // The lens is present, and its accessibility names the reason.
        let lens = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-lens")
        #expect(try lens.accessibilityLabel().string().contains("Running"))
    }

    @Test("Row shows open-task count only when nonzero")
    func showsOpenTaskCount() throws {
        let withTasks = makeRow(wave: makeWave(activeTasks: 3))
        _ = try withTasks.inspect().find(text: "3")

        let none = makeRow(wave: makeWave(activeTasks: 0))
        #expect(throws: (any Error).self) {
            _ = try none.inspect().find(text: "0")
        }
    }
}
