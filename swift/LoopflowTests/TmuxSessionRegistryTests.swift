import Testing
@testable import Concerto

@Suite("Tmux session registry")
struct TmuxSessionRegistryTests {
    @Test("killing a wave session removes it from future cleanup")
    func killWaveSessionRemovesTracking() {
        var killed: [String] = []
        let registry = TmuxSessionRegistry(killSession: { killed.append($0) })

        registry.track(sessionName: "lf-wave-1-pane-a")
        registry.killSession(named: "lf-wave-1-pane-a")
        registry.killAllSynchronously()

        #expect(killed == ["lf-wave-1-pane-a"])
    }

    @Test("tracking the same wave twice still kills one tmux session")
    func trackingSameWaveIsIdempotent() {
        var killed: [String] = []
        let registry = TmuxSessionRegistry(killSession: { killed.append($0) })

        registry.track(sessionName: "lf-wave-1-pane-a")
        registry.track(sessionName: "lf-wave-1-pane-a")
        registry.killAllSynchronously()

        #expect(killed == ["lf-wave-1-pane-a"])
    }

    @Test("kill all cleans up every tracked tmux session once")
    func killAllTrackedSessions() {
        var killed: [String] = []
        let registry = TmuxSessionRegistry(killSession: { killed.append($0) })

        registry.track(sessionName: "lf-wave-2-pane-a")
        registry.track(sessionName: "lf-wave-1-pane-a")
        registry.killAllSynchronously()
        registry.killAllSynchronously()

        #expect(Set(killed) == ["lf-wave-1-pane-a", "lf-wave-2-pane-a"])
        #expect(killed.count == 2)
    }
}
