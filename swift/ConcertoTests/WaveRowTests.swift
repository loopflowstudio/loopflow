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
        flow: String = "ship",
        status: WaveStatus = .idle,
        iteration: Int = 0,
        stimuli: [Stimulus] = []
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: "test-wave-id",
                name: name,
                repo: "/tmp/repo",
                flow: flow,
                area: area,
                stimuli: stimuli,
                status: status,
                iteration: iteration
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

    @Test("Row shows flow badge")
    func showsFlowBadge() throws {
        let wave = makeWave(flow: "polish")
        let row = makeRow(wave: wave)

        let flowText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-flow").text()
        #expect(try flowText.string() == "polish")
    }

    @Test("Row shows area in secondary line")
    func showsArea() throws {
        let wave = makeWave(area: ["lib/core"])
        let row = makeRow(wave: wave)

        let areaText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-area").text()
        #expect(try areaText.string() == "lib/core")
    }

    @Test("Row shows cron schedule for cron waves")
    func showsCronSchedule() throws {
        let wave = makeWave(stimuli: [Stimulus(kind: .cron, cron: "0 9 * * *")])
        let row = makeRow(wave: wave)

        let cronText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-stimulus").text()
        #expect(try cronText.string() == "9am daily")
    }

    @Test("Row shows raw cron when not a known pattern")
    func showsRawCron() throws {
        let wave = makeWave(stimuli: [Stimulus(kind: .cron, cron: "*/15 * * * *")])
        let row = makeRow(wave: wave)

        let cronText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-stimulus").text()
        #expect(try cronText.string() == "*/15 * * * *")
    }

}
