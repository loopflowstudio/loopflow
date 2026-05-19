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

    @Test("opening a terminal session focuses its wave and arms auto-present")
    func openTerminalSessionFocusesWave() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-1", status: .running)
        state.waveStore.set(wave)
        state.terminalWorkspaceStore.upsert(makeSession(id: "session-1", waveId: wave.id))

        state.openTerminalSession("session-1")

        #expect(state.selectedWaveId == wave.id)
        #expect(state.consumeAutoPresentTerminal(for: wave.id))
    }

    @Test("selecting a terminal session focuses its wave without auto-present")
    func selectTerminalSessionFocusesWaveWithoutAutoPresent() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-1", status: .running)
        state.waveStore.set(wave)
        state.terminalWorkspaceStore.upsert(makeSession(id: "session-1", waveId: wave.id))

        state.selectTerminalSession("session-1")

        #expect(state.selectedWaveId == wave.id)
        #expect(state.consumeAutoPresentTerminal(for: wave.id) == false)
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

    private func makeSession(id: String, waveId: String) -> TerminalSession {
        TerminalSession(
            id: id,
            waveId: waveId,
            step: "implement",
            agent: "claude",
            cwd: "/tmp/repo",
            tmuxName: "lf-test-\(id)",
            status: .pending,
            createdAt: .now
        )
    }

    // Covers the "click Ingest & build → terminal pane shows the live run"
    // path: when lfd emits a TerminalSession for a new wave run, the wave's
    // terminal pane must be repointed at that session's tmux name and armed
    // for auto-present, so the user lands on the running flow instead of the
    // empty default `lf-<waveId>-<paneId>` pane.
    @Test("new run terminal session repoints the wave's terminal pane and arms auto-present")
    func runTerminalSessionRepointsPaneAndArmsAutoPresent() {
        let state = RepoState()
        state.waveStore.onStatusChange = nil
        let wave = makeWave(id: "wave-for-run", status: .idle)
        state.waveStore.set(wave)

        // Seed the default terminal pane (happens when the wave is first opened).
        let seededPane = state.multiplexerStore.pane(ofType: .terminal, for: wave.id)
        ?? {
            _ = state.multiplexerStore.layout(for: wave.id)
            return state.multiplexerStore.pane(ofType: .terminal, for: wave.id)
        }()
        #expect(seededPane != nil, "wave layout should seed a terminal pane")
        let paneId = seededPane?.id ?? ""
        #expect(seededPane?.config.terminalSessionId == nil)

        let runSession = TerminalSession(
            id: "ts-run-1",
            waveId: wave.id,
            waveRunId: "run-1",
            step: "build",
            agent: "lf",
            cwd: "/tmp/repo",
            tmuxName: "lf-jack-heart-model-20260423_1303",
            status: .running,
            createdAt: .now
        )

        #expect(state.consumeAutoPresentTerminal(for: wave.id) == false)

        state.attachTerminalPane(to: runSession)

        let updatedPane = state.multiplexerStore.pane(ofType: .terminal, for: wave.id)
        #expect(updatedPane?.id == paneId, "the same pane should be updated, not replaced")
        #expect(
            updatedPane?.config.terminalSessionId == runSession.id,
            "pane must attach to the run's lfd terminal session id"
        )
        #expect(
            state.consumeAutoPresentTerminal(for: wave.id),
            "auto-present should be armed so focus shifts to the terminal pane"
        )
    }
}
