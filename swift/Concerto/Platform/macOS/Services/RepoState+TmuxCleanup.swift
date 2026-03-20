import LoopflowCore

extension RepoState {
    func deleteWaveAndCleanupTmux(_ wave: WaveViewModel) async throws {
        let sessionNames = multiplexerStore.terminalSessionNames(for: wave.id)
        try await deleteWave(wave)
        multiplexerStore.removeWave(wave.id)
        TmuxSessionRegistry.shared.killSessions(named: sessionNames)
    }
}
