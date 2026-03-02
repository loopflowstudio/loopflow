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
        triggers: [Trigger] = []
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
                iteration: 0
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

    @Test("Row shows trigger signal label")
    func showsTriggerSignal() throws {
        let wave = makeWave(triggers: [Trigger(signal: .repo, flow: "integrate")])
        let row = makeRow(wave: wave)

        let triggerText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-trigger").text()
        #expect(try triggerText.string() == "repo")
    }

    @Test("Row shows ci-fix for ci_failure trigger")
    func showsCiFixTrigger() throws {
        let wave = makeWave(triggers: [Trigger(signal: .ciFailure, flow: "ci-fix")])
        let row = makeRow(wave: wave)

        let triggerText = try row.inspect().find(viewWithAccessibilityIdentifier: "wave-trigger").text()
        #expect(try triggerText.string() == "ci-fix")
    }

}
