import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("RepoState Interactive Session Routing")
struct RepoStateInteractiveSessionTests {
    @Test("running wave with optimistic interactive start routes directly to chat")
    func optimisticInteractiveStartRoutesToChat() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-running", status: .running)
        state.waveStore.set(wave)

        #expect(state.shouldShowInteractiveChat(for: wave) == false)

        state.setOptimisticInteractiveSessionStart(for: wave.id, isStarting: true)
        #expect(state.isOptimisticallyStartingInteractiveSession(for: wave.id))
        #expect(state.shouldShowInteractiveChat(for: wave))
        #expect(state.chatState(for: wave.id).awaitingSessionJoin)

        state.setOptimisticInteractiveSessionStart(for: wave.id, isStarting: false)
        #expect(state.shouldShowInteractiveChat(for: wave) == false)
        #expect(state.chatState(for: wave.id).awaitingSessionJoin == false)
    }

    @Test("waiting wave routes to chat without optimistic state")
    func waitingWaveRoutesToChat() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-waiting", status: .waiting)
        state.waveStore.set(wave)

        #expect(state.shouldShowInteractiveChat(for: wave))
    }

    private func makeWave(id: String, status: WaveStatus) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: id,
                repo: "/tmp/repo",
                flow: "design-ship-review",
                direction: [],
                area: ["."],
                stimuli: [],
                status: status,
                iteration: 0
            )
        )
    }
}
