import Foundation
import Testing
@testable import LoopflowCore

@MainActor
@Suite("TerminalWorkspaceStore")
struct TerminalWorkspaceStoreTests {
    @Test("restores persisted ordering and keeps new sessions append-only")
    func restoresOrdering() {
        let defaults = UserDefaults(suiteName: "TerminalWorkspaceStoreTests.\(UUID().uuidString)")!
        defaults.set(["session-b", "session-a"], forKey: "terminalWorkspace.order./tmp/repo")
        defaults.set("session-b", forKey: "terminalWorkspace.selected./tmp/repo")

        let store = TerminalWorkspaceStore(userDefaults: defaults)
        store.configure(repoKey: "/tmp/repo")
        store.setAll([
            makeSession(id: "session-a", createdAt: 10),
            makeSession(id: "session-b", createdAt: 20),
            makeSession(id: "session-c", createdAt: 30),
        ])

        #expect(store.orderedSessions.map(\.id) == ["session-b", "session-a", "session-c"])
        #expect(store.selectedSession?.id == "session-b")
    }

    @Test("removing selected session falls back to next session")
    func removeSelectedSession() {
        let store = TerminalWorkspaceStore(userDefaults: UserDefaults(suiteName: "TerminalWorkspaceStoreTests.\(UUID().uuidString)")!)
        store.configure(repoKey: "/tmp/repo")
        store.setAll([
            makeSession(id: "session-a", createdAt: 10),
            makeSession(id: "session-b", createdAt: 20),
        ])
        store.select("session-b")

        store.remove("session-b")

        #expect(store.selectedSession?.id == "session-a")
        #expect(store.orderedSessions.map(\.id) == ["session-a"])
    }

    @Test("completed selected session falls back to another active session")
    func completedSessionIsDeselected() {
        let store = TerminalWorkspaceStore(userDefaults: UserDefaults(suiteName: "TerminalWorkspaceStoreTests.\(UUID().uuidString)")!)
        store.configure(repoKey: "/tmp/repo")
        store.setAll([
            makeSession(id: "session-a", createdAt: 10),
            makeSession(id: "session-b", createdAt: 20),
        ])
        store.select("session-b")

        store.upsert(makeSession(id: "session-b", createdAt: 20, status: .succeeded))

        #expect(store.selectedSession?.id == "session-a")
    }

    @Test("wave selections stay independent across workspaces")
    func waveSelectionsStayIndependent() {
        let store = TerminalWorkspaceStore(userDefaults: UserDefaults(suiteName: "TerminalWorkspaceStoreTests.\(UUID().uuidString)")!)
        store.configure(repoKey: "/tmp/repo")
        store.setAll([
            makeSession(id: "session-a1", waveId: "wave-a", createdAt: 10),
            makeSession(id: "session-a2", waveId: "wave-a", createdAt: 20),
            makeSession(id: "session-b1", waveId: "wave-b", createdAt: 30),
        ])

        store.select("session-a2", waveId: "wave-a")
        store.select("session-b1", waveId: "wave-b")

        #expect(store.selectedSession(for: "wave-a")?.id == "session-a2")
        #expect(store.selectedSession(for: "wave-b")?.id == "session-b1")
    }

    @Test("wave selection falls back inside the same wave")
    func completedWaveSessionFallsBackWithinWave() {
        let store = TerminalWorkspaceStore(userDefaults: UserDefaults(suiteName: "TerminalWorkspaceStoreTests.\(UUID().uuidString)")!)
        store.configure(repoKey: "/tmp/repo")
        store.setAll([
            makeSession(id: "session-a1", waveId: "wave-a", createdAt: 10),
            makeSession(id: "session-a2", waveId: "wave-a", createdAt: 20),
            makeSession(id: "session-b1", waveId: "wave-b", createdAt: 30),
        ])
        store.select("session-a2", waveId: "wave-a")

        store.upsert(makeSession(id: "session-a2", waveId: "wave-a", createdAt: 20, status: .succeeded))

        #expect(store.selectedSession(for: "wave-a")?.id == "session-a1")
        #expect(store.selectedSession(for: "wave-b")?.id == "session-b1")
    }

    private func makeSession(
        id: String,
        waveId: String? = nil,
        createdAt: TimeInterval,
        status: TerminalSessionStatus = .pending
    ) -> TerminalSession {
        TerminalSession(
            id: id,
            waveId: waveId ?? "wave-\(id)",
            step: "implement",
            agent: "claude",
            cwd: "/tmp/repo",
            status: status,
            createdAt: Date(timeIntervalSince1970: createdAt)
        )
    }
}
