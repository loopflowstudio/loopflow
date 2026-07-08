import Loopflow

extension RepoState {
    func deleteWaveAndCleanupTmux(_ wave: WaveViewModel) async throws {
        let sessionIds = multiplexerStore.terminalSessionIds(for: wave.id)
        for sessionId in sessionIds {
            _ = try? await cancelSession(sessionId)
        }
        try await deleteWave(wave)
        multiplexerStore.removeWave(wave.id)
    }
}
