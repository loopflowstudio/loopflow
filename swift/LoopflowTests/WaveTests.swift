import SwiftUI
import Testing
@testable import Loopflow

@Suite("Wave")
struct WaveTests {
    @Test("view model exposes registry identity")
    func exposesRegistryIdentity() {
        let wave = WaveViewModel(api: Wave(
            id: "wave-123",
            name: "infrastructure",
            repo: "/tmp/repo",
            status: .running
        ))

        #expect(wave.id == "wave-123")
        #expect(wave.displayName == "infrastructure")
        #expect(wave.repo == "/tmp/repo")
        #expect(wave.statusText == "Running")
    }

    @Test("objective tagline uses the first authored line")
    func objectiveTaglineUsesFirstLine() {
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-123",
                name: "infrastructure",
                repo: "/tmp/repo",
                status: .idle
            ),
            plan: WavePlan(objective: "\nMake releases boring.\nKeep them observable.")
        )

        #expect(wave.objectiveTagline == "Make releases boring.")
    }

    @Test("status owns its visual treatment")
    func statusOwnsVisualTreatment() {
        #expect(WaveStatus.running.icon == "circle.fill")
        #expect(WaveStatus.running.color == .statusSuccess)
        #expect(WaveStatus.paused.icon == "pause.circle")
    }
}
