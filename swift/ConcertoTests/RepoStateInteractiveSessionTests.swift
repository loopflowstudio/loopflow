import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("RepoState Interactive Session Routing")
struct RepoStateInteractiveSessionTests {
    @Test("running wave with optimistic interactive start routes directly to session")
    func optimisticInteractiveStartRoutesToSession() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-running", status: .running)
        state.waveStore.set(wave)

        #expect(state.shouldShowInteractiveSession(for: wave) == false)

        state.setOptimisticInteractiveSessionStart(for: wave.id, isStarting: true)
        #expect(state.isOptimisticallyStartingInteractiveSession(for: wave.id))
        #expect(state.shouldShowInteractiveSession(for: wave))
        #expect(state.sessionState(for: wave.id).awaitingSessionJoin)

        state.setOptimisticInteractiveSessionStart(for: wave.id, isStarting: false)
        #expect(state.shouldShowInteractiveSession(for: wave) == false)
        #expect(state.sessionState(for: wave.id).awaitingSessionJoin == false)
    }

    @Test("waiting wave routes to session without optimistic state")
    func waitingWaveRoutesToSession() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-waiting", status: .waiting)
        state.waveStore.set(wave)

        #expect(state.shouldShowInteractiveSession(for: wave))
    }

    @Test("terminal auto-present flag is one-shot")
    func terminalAutoPresentFlagIsOneShot() {
        let state = RepoState()

        #expect(state.consumeAutoPresentTerminal(for: "wave-1") == false)

        state.markAutoPresentTerminal(for: "wave-1")

        #expect(state.consumeAutoPresentTerminal(for: "wave-1"))
        #expect(state.consumeAutoPresentTerminal(for: "wave-1") == false)
    }

    @Test("switching repos clears terminal auto-present state")
    func selectRemoteRepoClearsTerminalAutoPresentState() {
        let state = RepoState()
        state.markAutoPresentTerminal(for: "wave-1")

        state.selectRemoteRepo(path: "/remote/repo")

        #expect(state.consumeAutoPresentTerminal(for: "wave-1") == false)
    }

    private func makeWave(id: String, status: WaveStatus) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: id,
                repo: "/tmp/repo",
                flow: "ship",
                direction: [],
                area: ["."],
                triggers: [],
                status: status,
                iteration: 0
            )
        )
    }
}
