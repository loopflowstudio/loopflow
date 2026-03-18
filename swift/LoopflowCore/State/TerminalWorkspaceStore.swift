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
        let knownIds = Set(orderedSessionIds)
        orderedSessionIds.append(
            contentsOf: sessions
                .sorted(by: { $0.createdAt < $1.createdAt })
                .map(\.id)
                .filter { !knownIds.contains($0) }
        )

        reconcileSelection()
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
        reconcileSelection()
        persist()
    }

    public func remove(_ sessionId: String) {
        sessionsById.removeValue(forKey: sessionId)
        orderedSessionIds.removeAll { $0 == sessionId }
        reconcileSelection()
        persist()
    }

    public func select(_ sessionId: String?) {
        selectedSessionId = sessionId
        reconcileSelection()
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

    private func reconcileSelection() {
        let currentSelection = selectedSessionId
        guard let currentSelection,
              let session = sessionsById[currentSelection],
              !session.status.isTerminal else {
            selectedSessionId = nextSelectableSessionId(excluding: currentSelection)
            return
        }
    }

    private func nextSelectableSessionId(excluding excludedSessionId: String? = nil) -> String? {
        orderedSessionIds.first { sessionId in
            guard sessionId != excludedSessionId,
                  let session = sessionsById[sessionId] else {
                return false
            }
            return !session.status.isTerminal
        }
    }

    private func restoreSelection() {
        guard let repoKey else {
            orderedSessionIds = []
            selectedSessionId = nil
            return
        }
        orderedSessionIds = userDefaults.stringArray(forKey: "terminalWorkspace.order.\(repoKey)") ?? []
        selectedSessionId = userDefaults.string(forKey: "terminalWorkspace.selected.\(repoKey)")
        reconcileSelection()
    }
}
