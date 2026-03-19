import Foundation

@MainActor
@Observable
public final class TerminalWorkspaceStore {
    public private(set) var sessionsById: [String: TerminalSession] = [:]
    public private(set) var orderedSessionIds: [String] = []
    public private(set) var selectedSessionIdsByWave: [String: String] = [:]
    public var selectedSessionId: String?

    private let userDefaults: UserDefaults
    private var repoKey: String?
    private var keepsGlobalSelectionCleared = false

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    public var orderedSessions: [TerminalSession] {
        orderedSessionIds.compactMap { sessionsById[$0] }
    }

    public var selectedSession: TerminalSession? {
        session(id: selectedSessionId)
    }

    public func orderedSessions(for waveId: String) -> [TerminalSession] {
        orderedSessions.filter { $0.waveId == waveId }
    }

    public func selectedSessionId(for waveId: String) -> String? {
        if let selectedSessionId,
           let session = activeSession(id: selectedSessionId),
           session.waveId == waveId {
            return selectedSessionId
        }
        if let storedSessionId = selectedSessionIdsByWave[waveId],
           activeSession(id: storedSessionId) != nil {
            return storedSessionId
        }
        return nextSelectableSessionId(for: waveId)
    }

    public func selectedSession(for waveId: String) -> TerminalSession? {
        session(id: selectedSessionId(for: waveId))
    }

    public func orderedSessions(for waveId: String) -> [TerminalSession] {
        orderedSessions.filter { $0.waveId == waveId }
    }

    public func selectedSessionId(for waveId: String) -> String? {
        if let selectedSessionId,
           let session = sessionsById[selectedSessionId],
           session.waveId == waveId,
           !session.status.isTerminal {
            return selectedSessionId
        }
        guard let storedSessionId = selectedSessionIdsByWave[waveId],
              let session = sessionsById[storedSessionId],
              !session.status.isTerminal else {
            return nextSelectableSessionId(for: waveId)
        }
        return storedSessionId
    }

    public func selectedSession(for waveId: String) -> TerminalSession? {
        selectedSessionId(for: waveId).flatMap { sessionsById[$0] }
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
        if select {
            selectedSessionId = session.id
            keepsGlobalSelectionCleared = false
<<<<<<< HEAD
=======
        }
        if select || selectedSessionIdsByWave[session.waveId] == nil {
            selectedSessionIdsByWave[session.waveId] = session.id
>>>>>>> 3a87b793 (concerto: keep terminal workspaces scoped to each wave)
        }
<<<<<<< HEAD
        if select || selectedSessionIdsByWave[session.waveId] == nil {
            selectedSessionIdsByWave[session.waveId] = session.id
        }
=======
>>>>>>> caddde45 (lf commit: compress)
        reconcileSelection()
        persist()
    }

    public func remove(_ sessionId: String) {
        sessionsById.removeValue(forKey: sessionId)
        orderedSessionIds.removeAll { $0 == sessionId }
        reconcileSelection()
        persist()
    }

    public func select(_ sessionId: String?, waveId: String? = nil) {
        selectedSessionId = sessionId
<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> 3a87b793 (concerto: keep terminal workspaces scoped to each wave)
        keepsGlobalSelectionCleared = sessionId == nil
        if let sessionId,
           let session = sessionsById[sessionId] {
            selectedSessionIdsByWave[session.waveId] = sessionId
        } else if let waveId, selectedSessionIdsByWave[waveId] == nil {
            selectedSessionIdsByWave[waveId] = nextSelectableSessionId(for: waveId)
        }
<<<<<<< HEAD
=======
>>>>>>> caddde45 (lf commit: compress)
=======
>>>>>>> 3a87b793 (concerto: keep terminal workspaces scoped to each wave)
        reconcileSelection()
        persist()
    }

    public func activeSession(for waveId: String) -> TerminalSession? {
        selectedSession(for: waveId) ?? orderedSessions.first { $0.waveId == waveId && !$0.status.isTerminal }
    }

    private func persist() {
        guard let orderKey = storageKey("order"),
<<<<<<< HEAD
<<<<<<< HEAD
              let selectedKey = storageKey("selected"),
              let waveSelectionKey = storageKey("selectedByWave") else {
=======
              let selectedKey = storageKey("selected") else {
>>>>>>> 0d6e9b1c (lf land: stage uncommitted changes)
=======
              let selectedKey = storageKey("selected"),
              let waveSelectionKey = storageKey("selectedByWave") else {
>>>>>>> 3a87b793 (concerto: keep terminal workspaces scoped to each wave)
            return
        }
        userDefaults.set(orderedSessionIds, forKey: orderKey)
        userDefaults.set(selectedSessionId, forKey: selectedKey)
<<<<<<< HEAD
<<<<<<< HEAD
        userDefaults.set(selectedSessionIdsByWave, forKey: waveSelectionKey)
    }

    private func reconcileSelection() {
        reconcileWaveSelections()

        guard let currentSelection = selectedSessionId else {
            if !keepsGlobalSelectionCleared {
                selectedSessionId = nextSelectableSessionId()
            }
            return
        }

        guard let session = activeSession(id: currentSelection) else {
            keepsGlobalSelectionCleared = false
            let currentWaveId = session(id: currentSelection)?.waveId
            selectedSessionId = currentWaveId.flatMap { nextSelectableSessionId(for: $0, excluding: currentSelection) }
                ?? nextSelectableSessionId(excluding: currentSelection)
            return
        }

        keepsGlobalSelectionCleared = false
        selectedSessionIdsByWave[session.waveId] = currentSelection
    }

    private func nextSelectableSessionId(
        for waveId: String? = nil,
        excluding excludedSessionId: String? = nil
    ) -> String? {
        orderedSessionIds.first { sessionId in
            guard sessionId != excludedSessionId,
                  let session = activeSession(id: sessionId) else {
                return false
            }
            return waveId == nil || session.waveId == waveId
        }
    }

    private func reconcileWaveSelections() {
        let waveIds = Set(sessionsById.values.map(\.waveId)).union(selectedSessionIdsByWave.keys)
        for waveId in waveIds {
            let currentSelection = selectedSessionIdsByWave[waveId]
            guard let currentSelection,
                  let session = activeSession(id: currentSelection) else {
                selectedSessionIdsByWave[waveId] = nextSelectableSessionId(
                    for: waveId,
                    excluding: currentSelection
                )
                continue
            }
            selectedSessionIdsByWave[waveId] = session.id
        }
=======
>>>>>>> 0d6e9b1c (lf land: stage uncommitted changes)
=======
        userDefaults.set(selectedSessionIdsByWave, forKey: waveSelectionKey)
>>>>>>> 3a87b793 (concerto: keep terminal workspaces scoped to each wave)
    }

    private func reconcileSelection() {
        reconcileWaveSelections()

        guard let currentSelection = selectedSessionId else {
            if !keepsGlobalSelectionCleared {
                selectedSessionId = nextSelectableSessionId()
            }
            return
        }

        guard let session = sessionsById[currentSelection],
              !session.status.isTerminal else {
            keepsGlobalSelectionCleared = false
            let currentWaveId = sessionsById[currentSelection]?.waveId
            selectedSessionId = currentWaveId.flatMap { nextSelectableSessionId(for: $0, excluding: currentSelection) }
                ?? nextSelectableSessionId(excluding: currentSelection)
            return
        }

        keepsGlobalSelectionCleared = false
        selectedSessionIdsByWave[session.waveId] = currentSelection
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

    private func nextSelectableSessionId(for waveId: String, excluding excludedSessionId: String? = nil) -> String? {
        orderedSessionIds.first { sessionId in
            guard sessionId != excludedSessionId,
                  let session = sessionsById[sessionId],
                  session.waveId == waveId else {
                return false
            }
            return !session.status.isTerminal
        }
    }

    private func reconcileWaveSelections() {
        let waveIds = Set(sessionsById.values.map(\.waveId)).union(selectedSessionIdsByWave.keys)
        for waveId in waveIds {
            let currentSelection = selectedSessionIdsByWave[waveId]
            guard let currentSelection,
                  let session = sessionsById[currentSelection],
                  !session.status.isTerminal else {
                selectedSessionIdsByWave[waveId] = nextSelectableSessionId(
                    for: waveId,
                    excluding: currentSelection
                )
                continue
            }
            selectedSessionIdsByWave[waveId] = session.id
        }
    }

    private func restoreSelection() {
        guard let orderKey = storageKey("order"),
<<<<<<< HEAD
<<<<<<< HEAD
              let selectedKey = storageKey("selected"),
              let waveSelectionKey = storageKey("selectedByWave") else {
=======
              let selectedKey = storageKey("selected") else {
>>>>>>> 0d6e9b1c (lf land: stage uncommitted changes)
=======
              let selectedKey = storageKey("selected"),
              let waveSelectionKey = storageKey("selectedByWave") else {
>>>>>>> 3a87b793 (concerto: keep terminal workspaces scoped to each wave)
            orderedSessionIds = []
            selectedSessionIdsByWave = [:]
            selectedSessionId = nil
            return
        }
<<<<<<< HEAD
<<<<<<< HEAD
        orderedSessionIds = userDefaults.stringArray(forKey: orderKey) ?? []
        selectedSessionId = userDefaults.string(forKey: selectedKey)
        selectedSessionIdsByWave = userDefaults.dictionary(forKey: waveSelectionKey) as? [String: String] ?? [:]
        keepsGlobalSelectionCleared = false
        reconcileSelection()
    }

    private func storageKey(_ suffix: String) -> String? {
        guard let repoKey else { return nil }
        return "terminalWorkspace.\(suffix).\(repoKey)"
    }

    private func session(id: String?) -> TerminalSession? {
        guard let id else { return nil }
        return sessionsById[id]
    }

    private func activeSession(id: String?) -> TerminalSession? {
        guard let session = session(id: id),
              !session.status.isTerminal else {
            return nil
        }
        return session
=======
        orderedSessionIds = userDefaults.stringArray(forKey: "terminalWorkspace.order.\(repoKey)") ?? []
        selectedSessionId = userDefaults.string(forKey: "terminalWorkspace.selected.\(repoKey)")
=======
        orderedSessionIds = userDefaults.stringArray(forKey: orderKey) ?? []
        selectedSessionId = userDefaults.string(forKey: selectedKey)
>>>>>>> 0d6e9b1c (lf land: stage uncommitted changes)
        reconcileSelection()
>>>>>>> caddde45 (lf commit: compress)
    }

    private func storageKey(_ suffix: String) -> String? {
        guard let repoKey else { return nil }
        return "terminalWorkspace.\(suffix).\(repoKey)"
    }
}
