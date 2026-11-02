import Testing
@testable import Loopflow

@Suite("Wave")
struct WaveTests {
    @Test("view model exposes registry identity")
    func exposesRegistryIdentity() {
        let wave = WaveViewModel(api: Wave(
            id: "wave-123",
            name: "OpenAI-tools",
            repo: "/tmp/repo",
            status: .ready,
            
        ))

        #expect(wave.id == "wave-123")
        #expect(wave.displayName == "OpenAI-tools")
        #expect(wave.repo == "/tmp/repo")
        #expect(wave.lens.color == .black)
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
