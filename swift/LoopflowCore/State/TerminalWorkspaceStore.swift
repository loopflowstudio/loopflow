import Foundation

@MainActor
@Observable
public final class TerminalWorkspaceStore {
    public private(set) var sessionsById: [String: TerminalSession] = [:]
    public private(set) var orderedSessionIds: [String] = []
    public var selectedSessionId: String?

    private let userDefaults: UserDefaults
    private var repoKey: String?

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    public var orderedSessions: [TerminalSession] {
        orderedSessionIds.compactMap { sessionsById[$0] }
    }

    public var selectedSession: TerminalSession? {
        selectedSessionId.flatMap { sessionsById[$0] }
    }

    public func configure(repoKey: String?) {
        self.repoKey = repoKey
        restoreSelection()
    }

    public func setAll(_ sessions: [TerminalSession]) {
        sessionsById = Dictionary(uniqueKeysWithValues: sessions.map { ($0.id, $0) })
        let activeIds = Set(sessionsById.keys)

        orderedSessionIds = orderedSessionIds.filter(activeIds.contains)
        for session in sessions.sorted(by: { $0.createdAt < $1.createdAt }) where !orderedSessionIds.contains(session.id) {
            orderedSessionIds.append(session.id)
        }

        if let selectedSessionId, sessionsById[selectedSessionId] == nil {
            self.selectedSessionId = orderedSessionIds.first
        } else if self.selectedSessionId == nil {
            self.selectedSessionId = orderedSessionIds.first
        }

        persist()
    }

    public func upsert(_ session: TerminalSession, select: Bool = false) {
        sessionsById[session.id] = session
        if !orderedSessionIds.contains(session.id) {
            orderedSessionIds.append(session.id)
        }
        if select || selectedSessionId == nil {
            selectedSessionId = session.id
        }
        if session.status.isTerminal {
            if selectedSessionId == session.id {
                selectedSessionId = orderedSessionIds.first(where: { $0 != session.id })
            }
        }
        persist()
    }

    public func remove(_ sessionId: String) {
        sessionsById.removeValue(forKey: sessionId)
        orderedSessionIds.removeAll { $0 == sessionId }
        if selectedSessionId == sessionId {
            selectedSessionId = orderedSessionIds.first
        }
        persist()
    }

    public func select(_ sessionId: String?) {
        selectedSessionId = sessionId
        persist()
    }

    public func activeSession(for waveId: String) -> TerminalSession? {
        orderedSessions.first { $0.waveId == waveId && !$0.status.isTerminal }
    }

    private func persist() {
        guard let repoKey else { return }
        userDefaults.set(orderedSessionIds, forKey: "terminalWorkspace.order.\(repoKey)")
        userDefaults.set(selectedSessionId, forKey: "terminalWorkspace.selected.\(repoKey)")
    }

    private func restoreSelection() {
        guard let repoKey else {
            orderedSessionIds = []
            selectedSessionId = nil
            return
        }
        orderedSessionIds = userDefaults.stringArray(forKey: "terminalWorkspace.order.\(repoKey)") ?? []
        selectedSessionId = userDefaults.string(forKey: "terminalWorkspace.selected.\(repoKey)")
    }
}
