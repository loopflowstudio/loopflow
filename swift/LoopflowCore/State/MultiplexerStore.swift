// Per-wave multiplexer state: layout tree and focused pane.
// Persists to UserDefaults keyed by repo + wave.

import Foundation

@MainActor
@Observable
public final class MultiplexerStore {
    private var layoutsByWave: [String: LayoutNode] = [:]
    private var focusedPaneByWave: [String: String] = [:]
    private let userDefaults: UserDefaults
    private var repoKey: String?

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    public func configure(repoKey: String?) {
        self.repoKey = repoKey
        restore()
    }

    // MARK: - Layout access

    public func layout(for waveId: String) -> LayoutNode {
        if let existing = loadedLayout(for: waveId) {
            return existing
        }
<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
        let defaultLayout = assignTerminalSessions(in: LayoutNode.defaultLayout(), for: waveId)
=======
        let defaultLayout = LayoutNode.defaultLayout()
>>>>>>> 55cd605c (lf commit: implement)
=======
        let defaultLayout = assignTerminalSessions(in: LayoutNode.defaultLayout(), for: waveId)
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
=======

        let defaultLayout = normalizeLayout(LayoutNode.defaultLayout(), for: waveId)
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
        layoutsByWave[waveId] = defaultLayout
        return defaultLayout
    }

    public func focusedPaneId(for waveId: String) -> String? {
<<<<<<< HEAD
<<<<<<< HEAD
        let layout = layout(for: waveId)
        return focusedPaneByWave[waveId].flatMap { layout.pane(for: $0)?.id } ?? layout.firstPane?.id
    }

    public func focusedPane(for waveId: String) -> PaneState? {
        let layout = layout(for: waveId)
<<<<<<< HEAD
        guard let id = focusedPaneId(for: waveId) else { return nil }
        return layout.pane(for: id)
=======
=======
        let layout = layout(for: waveId)
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
        let id = focusedPaneByWave[waveId]
        if let id, layout.pane(for: id) != nil {
            return id
        }
        return layout.firstPane?.id
    }

    public func focusedPane(for waveId: String) -> PaneState? {
        let layout = layout(for: waveId)
        guard let id = focusedPaneId(for: waveId) else { return nil }
<<<<<<< HEAD
        return layout(for: waveId).pane(for: id)
>>>>>>> 55cd605c (lf commit: implement)
=======
        return layout.pane(for: id)
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
=======
        return focusedPaneId(for: waveId).flatMap { layout.pane(for: $0) }
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
    }

    // MARK: - Mutations

    public func setLayout(_ layout: LayoutNode, for waveId: String) {
<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
        layoutsByWave[waveId] = assignTerminalSessions(in: layout, for: waveId)
=======
        layoutsByWave[waveId] = layout
>>>>>>> 55cd605c (lf commit: implement)
=======
        layoutsByWave[waveId] = assignTerminalSessions(in: layout, for: waveId)
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
=======
        layoutsByWave[waveId] = normalizeLayout(layout, for: waveId)
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
        reconcileFocus(for: waveId)
        persist()
    }

    public func setFocusedPane(_ paneId: String, for waveId: String) {
        guard layout(for: waveId).pane(for: paneId) != nil else { return }
        focusedPaneByWave[waveId] = paneId
        persist()
    }

    public func splitPane(
        _ paneId: String,
        axis: SplitAxis,
        newPaneType: PaneType,
        for waveId: String
    ) -> PaneState {
<<<<<<< HEAD
<<<<<<< HEAD
        let newPane = makePane(type: newPaneType, for: waveId)
<<<<<<< HEAD
=======
        if newPaneType == .terminal, let terminalPane = layout(for: waveId).allPanes.first(where: { $0.type == .terminal }) {
            focusedPaneByWave[waveId] = terminalPane.id
            persist()
            return terminalPane
        }

        let newPane = PaneState(type: newPaneType)
>>>>>>> 55cd605c (lf commit: implement)
=======
        let newPane = makePane(type: newPaneType, for: waveId)
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
        let current = layout(for: waveId)
        let updated = current.splitting(paneId, axis: axis, newPane: newPane)
=======
        let updated = layout(for: waveId).splitting(paneId, axis: axis, newPane: newPane)
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
        layoutsByWave[waveId] = updated
        focusedPaneByWave[waveId] = newPane.id
        persist()
        return newPane
    }

    public func closePane(_ paneId: String, for waveId: String) -> PaneState? {
        let current = layout(for: waveId)
        guard let closedPane = current.pane(for: paneId) else { return nil }
<<<<<<< HEAD
<<<<<<< HEAD
=======
        guard closedPane.type != .terminal else { return nil }
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)

        if let updated = current.removing(paneId) {
            layoutsByWave[waveId] = updated
            reconcileFocus(for: waveId)
        } else {
<<<<<<< HEAD
            layoutsByWave.removeValue(forKey: waveId)
            focusedPaneByWave.removeValue(forKey: waveId)
<<<<<<< HEAD
<<<<<<< HEAD
            removePersistedLayout(for: waveId)
=======
>>>>>>> 55cd605c (lf commit: implement)
=======
            removePersistedLayout(for: waveId)
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
=======
            clearWaveState(waveId)
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
        }

        persist()
        return closedPane
    }

    public func updatePaneConfig(_ paneId: String, config: PaneConfig, for waveId: String) {
        let layout = layout(for: waveId)
        guard let pane = layout.pane(for: paneId) else { return }

        let normalizedConfig = normalizeConfig(config, for: waveId, paneId: paneId, type: pane.type)
        layoutsByWave[waveId] = layout.updatingPane(paneId, config: normalizedConfig)
        persist()
    }

<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
    public func replacePane(
        _ paneId: String,
        with newPaneType: PaneType,
        config: PaneConfig = .empty,
        for waveId: String
    ) -> PaneState? {
        let current = layout(for: waveId)
        guard current.pane(for: paneId) != nil else { return nil }

        let replacement = PaneState(
            id: paneId,
            type: newPaneType,
            config: normalizeConfig(config, for: waveId, paneId: paneId, type: newPaneType)
        )
        layoutsByWave[waveId] = current.replacingPane(paneId, with: replacement)
        focusedPaneByWave[waveId] = paneId
        persist()
        return replacement
    }

<<<<<<< HEAD
=======
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
    public func moveFocus(_ direction: FocusDirection, for waveId: String) {
        guard let current = focusedPaneId(for: waveId) else { return }

        let layout = layout(for: waveId)
        let nextPane: PaneState?
        switch direction {
        case .next:
            nextPane = layout.nextPane(after: current)
        case .previous:
            nextPane = layout.previousPane(before: current)
        }

        if let nextPane {
            focusedPaneByWave[waveId] = nextPane.id
            persist()
        }
    }

    public func removeWave(_ waveId: String) {
<<<<<<< HEAD
        layoutsByWave.removeValue(forKey: waveId)
        focusedPaneByWave.removeValue(forKey: waveId)
<<<<<<< HEAD
<<<<<<< HEAD
        removePersistedLayout(for: waveId)
=======
        clearWaveState(waveId)
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
        persist()
    }

<<<<<<< HEAD
    public func terminalSessionNames(for waveId: String) -> [String] {
        ensureLoaded(waveId)
        guard let layout = layoutsByWave[waveId] else { return [] }
        return layout.allPanes.compactMap { pane in
            guard pane.type == .terminal else { return nil }
            return pane.config.terminalSessionName
        }
=======
=======
        removePersistedLayout(for: waveId)
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
        persist()
    }

    public func terminalPane(for waveId: String) -> PaneState? {
        layout(for: waveId).allPanes.first(where: { $0.type == .terminal })
>>>>>>> 55cd605c (lf commit: implement)
    }

=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
    public func terminalSessionNames(for waveId: String) -> [String] {
        layout(for: waveId).allPanes.compactMap { pane in
            guard pane.type == .terminal else { return nil }
            return pane.config.terminalSessionName
        }
    }

    // MARK: - Persistence

    private func persist() {
        guard let repoKey else { return }

        let encoder = JSONEncoder()
        for (waveId, layout) in layoutsByWave {
            let key = storageKey(repoKey: repoKey, waveId: waveId, suffix: "layout")
            if let data = try? encoder.encode(layout) {
                userDefaults.set(data, forKey: key)
            }
        }

        userDefaults.set(
            focusedPaneByWave,
            forKey: storageKey(repoKey: repoKey, suffix: "focus")
        )
    }

    private func restore() {
        guard let repoKey else {
            layoutsByWave.removeAll()
            focusedPaneByWave.removeAll()
            return
        }

        focusedPaneByWave = userDefaults.dictionary(forKey: storageKey(repoKey: repoKey, suffix: "focus")) as? [String: String] ?? [:]
        layoutsByWave.removeAll()
    }

    private func loadedLayout(for waveId: String) -> LayoutNode? {
        ensureLoaded(waveId)
        return layoutsByWave[waveId]
    }

    private func ensureLoaded(_ waveId: String) {
        guard layoutsByWave[waveId] == nil, let repoKey else { return }

        let key = storageKey(repoKey: repoKey, waveId: waveId, suffix: "layout")
<<<<<<< HEAD
        guard let data = userDefaults.data(forKey: key) else { return }
<<<<<<< HEAD
<<<<<<< HEAD
        if let decoded = try? JSONDecoder().decode(LayoutNode.self, from: data) {
            layoutsByWave[waveId] = assignTerminalSessions(in: decoded, for: waveId)
        }
=======
        layoutsByWave[waveId] = try? JSONDecoder().decode(LayoutNode.self, from: data)
>>>>>>> 55cd605c (lf commit: implement)
=======
        if let decoded = try? JSONDecoder().decode(LayoutNode.self, from: data) {
            layoutsByWave[waveId] = assignTerminalSessions(in: decoded, for: waveId)
        }
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
=======
        guard let data = userDefaults.data(forKey: key),
              let decoded = try? JSONDecoder().decode(LayoutNode.self, from: data) else {
            return
        }

        layoutsByWave[waveId] = normalizeLayout(decoded, for: waveId)
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
    }

    private func reconcileFocus(for waveId: String) {
        guard let focused = focusedPaneByWave[waveId],
              layout(for: waveId).pane(for: focused) == nil else {
            return
        }

        focusedPaneByWave[waveId] = layout(for: waveId).firstPane?.id
    }

    private func storageKey(repoKey: String, waveId: String? = nil, suffix: String) -> String {
        if let waveId {
            return "multiplexer.\(suffix).\(repoKey).\(waveId)"
        }
        return "multiplexer.\(suffix).\(repoKey)"
    }
<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)

    private func makePane(type: PaneType, for waveId: String) -> PaneState {
        let paneId = UUID().uuidString
        return PaneState(
            id: paneId,
            type: type,
            config: normalizeConfig(.empty, for: waveId, paneId: paneId, type: type)
        )
    }

    private func normalizeLayout(_ layout: LayoutNode, for waveId: String) -> LayoutNode {
        layout.allPanes.reduce(layout) { updatedLayout, pane in
            let config = normalizeConfig(pane.config, for: waveId, paneId: pane.id, type: pane.type)
            guard config != pane.config else { return updatedLayout }
            return updatedLayout.updatingPane(pane.id, config: config)
        }
    }

    private func normalizeConfig(_ config: PaneConfig, for waveId: String, paneId: String, type: PaneType) -> PaneConfig {
        guard type == .terminal, config.terminalSessionName == nil else { return config }

        var updatedConfig = config
        updatedConfig.terminalSessionName = terminalSessionName(for: waveId, paneId: paneId)
        return updatedConfig
    }

    private func terminalSessionName(for waveId: String, paneId: String) -> String {
        "lf-\(waveId)-\(paneId)"
    }

    private func clearWaveState(_ waveId: String) {
        layoutsByWave.removeValue(forKey: waveId)
        focusedPaneByWave.removeValue(forKey: waveId)
        removePersistedLayout(for: waveId)
    }

    private func removePersistedLayout(for waveId: String) {
        guard let repoKey else { return }
        userDefaults.removeObject(forKey: storageKey(repoKey: repoKey, waveId: waveId, suffix: "layout"))
    }
<<<<<<< HEAD
=======
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
}

public enum FocusDirection: Sendable {
    case next
    case previous
}
