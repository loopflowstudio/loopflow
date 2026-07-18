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
            status: .running(runID: "run_test"),
            live: true
        ))

        #expect(wave.id == "wave-123")
        #expect(wave.displayName == "infrastructure")
        #expect(wave.repo == "/tmp/repo")
        #expect(wave.lens.color == .green)
        #expect(wave.isRegistered)
    }

    @Test("authored waves can exist before runtime registration")
    func authoredWaveHasNoRegistryRow() {
        let wave = WaveViewModel(
            api: Wave(
                id: "authored:/tmp/repo#infrastructure",
                name: "infrastructure",
                repo: "/tmp/repo",
                status: .ready
            ),
            isRegistered: false
        )

        #expect(!wave.isRegistered)
        // No runtime reading exists yet, so the lens is unknown-with-reason —
        // never a silent black and never a local-session guess.
        #expect(wave.lens.color == .unknown)
        #expect(!wave.lens.reason.isEmpty)
    }

    @Test("objective tagline uses the first authored line")
    func objectiveTaglineUsesFirstLine() {
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-123",
                name: "infrastructure",
                repo: "/tmp/repo",
                status: .ready
            ),
            plan: WavePlan(objective: "\nMake releases boring.\nKeep them observable.")
        )

        #expect(wave.objectiveTagline == "Make releases boring.")
    }
}
