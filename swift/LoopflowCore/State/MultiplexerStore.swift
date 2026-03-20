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
        ensureLoaded(waveId)
        if let existing = layoutsByWave[waveId] {
            return existing
        }
        let defaultLayout = assignTerminalSessions(in: LayoutNode.defaultLayout(), for: waveId)
        layoutsByWave[waveId] = defaultLayout
        return defaultLayout
    }

    public func focusedPaneId(for waveId: String) -> String? {
        let layout = layout(for: waveId)
        let id = focusedPaneByWave[waveId]
        if let id, layout.pane(for: id) != nil {
            return id
        }
        return layout.firstPane?.id
    }

    public func focusedPane(for waveId: String) -> PaneState? {
        let layout = layout(for: waveId)
        guard let id = focusedPaneId(for: waveId) else { return nil }
        return layout.pane(for: id)
    }

    // MARK: - Mutations

    public func setLayout(_ layout: LayoutNode, for waveId: String) {
        layoutsByWave[waveId] = assignTerminalSessions(in: layout, for: waveId)
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
        let newPane = makePane(type: newPaneType, for: waveId)
        let current = layout(for: waveId)
        let updated = current.splitting(paneId, axis: axis, newPane: newPane)
        layoutsByWave[waveId] = updated
        focusedPaneByWave[waveId] = newPane.id
        persist()
        return newPane
    }

    public func closePane(_ paneId: String, for waveId: String) -> PaneState? {
        let current = layout(for: waveId)
        guard let closedPane = current.pane(for: paneId) else { return nil }

        if let updated = current.removing(paneId) {
            layoutsByWave[waveId] = updated
            reconcileFocus(for: waveId)
        } else {
            layoutsByWave.removeValue(forKey: waveId)
            focusedPaneByWave.removeValue(forKey: waveId)
            removePersistedLayout(for: waveId)
        }
        persist()
        return closedPane
    }

    public func updatePaneConfig(_ paneId: String, config: PaneConfig, for waveId: String) {
        let current = layout(for: waveId)
        layoutsByWave[waveId] = current.updatingPane(paneId, config: config)
        persist()
    }

    public func replacePane(
        _ paneId: String,
        with newPaneType: PaneType,
        config: PaneConfig = .empty,
        for waveId: String
    ) -> PaneState? {
        let current = layout(for: waveId)
        guard current.pane(for: paneId) != nil else { return nil }

        var newConfig = config
        if newPaneType == .terminal, newConfig.terminalSessionName == nil {
            newConfig.terminalSessionName = terminalSessionName(for: waveId, paneId: paneId)
        }

        let replacement = PaneState(id: paneId, type: newPaneType, config: newConfig)
        layoutsByWave[waveId] = current.replacingPane(paneId, with: replacement)
        focusedPaneByWave[waveId] = paneId
        persist()
        return replacement
    }

    public func moveFocus(_ direction: FocusDirection, for waveId: String) {
        guard let current = focusedPaneId(for: waveId) else { return }
        let layout = layout(for: waveId)
        let next: PaneState?
        switch direction {
        case .next: next = layout.nextPane(after: current)
        case .previous: next = layout.previousPane(before: current)
        }
        if let next {
            focusedPaneByWave[waveId] = next.id
            persist()
        }
    }

    public func removeWave(_ waveId: String) {
        layoutsByWave.removeValue(forKey: waveId)
        focusedPaneByWave.removeValue(forKey: waveId)
        removePersistedLayout(for: waveId)
        persist()
    }

    public func terminalSessionNames(for waveId: String) -> [String] {
        ensureLoaded(waveId)
        guard let layout = layoutsByWave[waveId] else { return [] }
        return layout.allPanes.compactMap { pane in
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
        let focusKey = storageKey(repoKey: repoKey, suffix: "focus")
        userDefaults.set(focusedPaneByWave, forKey: focusKey)
    }

    private func restore() {
        guard let repoKey else {
            layoutsByWave.removeAll()
            focusedPaneByWave.removeAll()
            return
        }
        // Restore focus map
        let focusKey = storageKey(repoKey: repoKey, suffix: "focus")
        focusedPaneByWave = userDefaults.dictionary(forKey: focusKey) as? [String: String] ?? [:]

        // Layouts are restored lazily on access — we don't know all wave IDs upfront.
        // But we can scan UserDefaults for matching keys.
        layoutsByWave.removeAll()
    }

    /// Lazily restore a wave's layout from UserDefaults if not already loaded.
    private func ensureLoaded(_ waveId: String) {
        guard layoutsByWave[waveId] == nil, let repoKey else { return }
        let key = storageKey(repoKey: repoKey, waveId: waveId, suffix: "layout")
        guard let data = userDefaults.data(forKey: key) else { return }
        if let decoded = try? JSONDecoder().decode(LayoutNode.self, from: data) {
            layoutsByWave[waveId] = assignTerminalSessions(in: decoded, for: waveId)
        }
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

    private func makePane(type: PaneType, for waveId: String) -> PaneState {
        let paneId = UUID().uuidString
        var config = PaneConfig.empty
        if type == .terminal {
            config.terminalSessionName = terminalSessionName(for: waveId, paneId: paneId)
        }
        return PaneState(id: paneId, type: type, config: config)
    }

    private func assignTerminalSessions(in layout: LayoutNode, for waveId: String) -> LayoutNode {
        var updated = layout
        for pane in layout.allPanes where pane.type == .terminal && pane.config.terminalSessionName == nil {
            var config = pane.config
            config.terminalSessionName = terminalSessionName(for: waveId, paneId: pane.id)
            updated = updated.updatingPane(pane.id, config: config)
        }
        return updated
    }

    private func terminalSessionName(for waveId: String, paneId: String) -> String {
        "lf-\(waveId)-\(paneId)"
    }

    private func removePersistedLayout(for waveId: String) {
        guard let repoKey else { return }
        userDefaults.removeObject(forKey: storageKey(repoKey: repoKey, waveId: waveId, suffix: "layout"))
    }
}

public enum FocusDirection: Sendable {
    case next
    case previous
}
