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
        let defaultLayout = LayoutNode.defaultLayout()
        layoutsByWave[waveId] = defaultLayout
        return defaultLayout
    }

    public func focusedPaneId(for waveId: String) -> String? {
        let id = focusedPaneByWave[waveId]
        if let id, layout(for: waveId).pane(for: id) != nil {
            return id
        }
        return layout(for: waveId).firstPane?.id
    }

    public func focusedPane(for waveId: String) -> PaneState? {
        guard let id = focusedPaneId(for: waveId) else { return nil }
        return layout(for: waveId).pane(for: id)
    }

    // MARK: - Mutations

    public func setLayout(_ layout: LayoutNode, for waveId: String) {
        layoutsByWave[waveId] = layout
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
        if newPaneType == .terminal, let terminalPane = layout(for: waveId).allPanes.first(where: { $0.type == .terminal }) {
            focusedPaneByWave[waveId] = terminalPane.id
            persist()
            return terminalPane
        }

        let newPane = PaneState(type: newPaneType)
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
        guard closedPane.type != .terminal else { return nil }

        if let updated = current.removing(paneId) {
            layoutsByWave[waveId] = updated
            reconcileFocus(for: waveId)
        } else {
            layoutsByWave.removeValue(forKey: waveId)
            focusedPaneByWave.removeValue(forKey: waveId)
        }
        persist()
        return closedPane
    }

    public func updatePaneConfig(_ paneId: String, config: PaneConfig, for waveId: String) {
        let current = layout(for: waveId)
        layoutsByWave[waveId] = current.updatingPane(paneId, config: config)
        persist()
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
        persist()
    }

    public func terminalPane(for waveId: String) -> PaneState? {
        layout(for: waveId).allPanes.first(where: { $0.type == .terminal })
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
        layoutsByWave[waveId] = try? JSONDecoder().decode(LayoutNode.self, from: data)
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
}

public enum FocusDirection: Sendable {
    case next
    case previous
}
