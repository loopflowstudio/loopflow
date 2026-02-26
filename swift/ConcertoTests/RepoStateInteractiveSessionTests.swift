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

    private func makeWave(id: String, status: WaveStatus) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: id,
                repo: "/tmp/repo",
                flow: "ship",
                direction: [],
                area: ["."],
                stimuli: [],
                status: status,
                iteration: 0
            )
        )
    }
}
