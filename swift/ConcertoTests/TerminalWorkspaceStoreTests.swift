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

    private func makeSession(id: String, createdAt: TimeInterval) -> TerminalSession {
        TerminalSession(
            id: id,
            waveId: "wave-\(id)",
            step: "implement",
            agent: "claude",
            cwd: "/tmp/repo",
            status: .pending,
            createdAt: Date(timeIntervalSince1970: createdAt)
        )
    }
}
