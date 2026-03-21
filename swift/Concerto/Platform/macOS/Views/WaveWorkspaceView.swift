// Wave workspace — multiplexer view for the selected wave.
// Interactive sessions still take over the view when a wave is waiting
// for input; otherwise the workspace opens into the per-wave multiplexer.

import SwiftUI
import LoopflowCore

struct WaveWorkspaceView: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer

    private var currentWave: WaveViewModel {
        repoState.waveStore.wave(for: wave.id) ?? wave
    }

    private var activeSessionState: SessionState? {
        guard repoState.shouldShowInteractiveSession(for: currentWave) else {
            return nil
        }

        if let sessionId = repoState.interactiveSessionId(for: currentWave.id) {
            return repoState.sessionState(for: currentWave.id, joinSessionId: sessionId)
        }
        return repoState.sessionState(for: currentWave.id)
    }

    var body: some View {
        Group {
            if let state = activeSessionState {
                WaveSessionView(state: state)
            } else if outputBuffer.hasActiveSession(for: currentWave.id),
                      let session = outputBuffer.interactiveSession {
                InteractiveSessionView(session: session)
            } else {
                MultiplexerView(wave: currentWave)
                    .id(currentWave.id)
            }
        }
        .task(id: currentWave.id) {
            _ = repoState.multiplexerStore.layout(for: currentWave)
            repoState.loadWaveContent(for: currentWave.id)
            repoState.loadRuns(for: currentWave.id)
            focusTerminalPaneIfNeeded()
        }
    }

    private func focusTerminalPaneIfNeeded() {
        guard repoState.consumeAutoPresentTerminal(for: currentWave.id),
              let terminalPane = repoState.multiplexerStore.pane(ofType: .terminal, for: currentWave.id) else {
            return
        }

        repoState.multiplexerStore.setFocusedPane(terminalPane.id, for: currentWave.id)
    }
}
