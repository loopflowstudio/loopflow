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
        resolvedLayout(for: waveId) {
            LayoutNode.defaultLayout()
        }
    }

    public func layout(for wave: WaveViewModel) -> LayoutNode {
        resolvedLayout(for: wave.id) {
            defaultLayout(for: wave)
        }
    }

    public func defaultLayout(for wave: WaveViewModel) -> LayoutNode {
        let roadmap = makePane(type: .roadmap, for: wave.id)
        let runs = makePane(type: .runs, for: wave.id)
        let terminal = makePane(type: .terminal, for: wave.id)

        return .split(
            .horizontal,
            first: .split(
                .vertical,
                first: .leaf(roadmap),
                second: .leaf(runs),
                ratio: 0.58
            ),
            second: .leaf(terminal),
            ratio: 0.42
        )
    }

    public func layout(for wave: WaveViewModel) -> LayoutNode {
        if let existing = loadedLayout(for: wave.id) {
            return existing
        }

        let defaultLayout = normalizeLayout(defaultLayout(for: wave), for: wave.id)
        layoutsByWave[wave.id] = defaultLayout
        return defaultLayout
    }

    public func defaultLayout(for wave: WaveViewModel) -> LayoutNode {
        let roadmap = makePane(type: .roadmap, for: wave.id)
        let runs = makePane(type: .runs, for: wave.id)
        let terminal = makePane(type: .terminal, for: wave.id)

        return .split(
            .horizontal,
            first: .split(
                .vertical,
                first: .leaf(roadmap),
                second: .leaf(runs),
                ratio: 0.58
            ),
            second: .leaf(terminal),
            ratio: 0.42
        )
    }

    public func focusedPaneId(for waveId: String) -> String? {
        let layout = layout(for: waveId)
        return focusedPaneByWave[waveId].flatMap { layout.pane(for: $0)?.id } ?? layout.firstPane?.id
    }

    public func focusedPane(for waveId: String) -> PaneState? {
        let layout = layout(for: waveId)
        return focusedPaneId(for: waveId).flatMap { layout.pane(for: $0) }
    }

    public func pane(ofType type: PaneType, for waveId: String) -> PaneState? {
        layout(for: waveId).allPanes.first { $0.type == type }
    }

    // MARK: - Mutations

    public func setLayout(_ layout: LayoutNode, for waveId: String) {
        layoutsByWave[waveId] = normalizeLayout(layout, for: waveId)
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
        let updated = layout(for: waveId).splitting(paneId, axis: axis, newPane: newPane)
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
            clearWaveState(waveId)
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
        clearWaveState(waveId)
        persist()
    }

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

    private func resolvedLayout(for waveId: String, makeDefault: () -> LayoutNode) -> LayoutNode {
        if let existing = loadedLayout(for: waveId) {
            return existing
        }

        let layout = normalizeLayout(makeDefault(), for: waveId)
        layoutsByWave[waveId] = layout
        return layout
    }

    private func ensureLoaded(_ waveId: String) {
        guard layoutsByWave[waveId] == nil, let repoKey else { return }

        let key = storageKey(repoKey: repoKey, waveId: waveId, suffix: "layout")
        guard let data = userDefaults.data(forKey: key),
              let decoded = try? JSONDecoder().decode(LayoutNode.self, from: data) else {
            return
        }

        layoutsByWave[waveId] = normalizeLayout(decoded, for: waveId)
    }

    private func reconcileFocus(for waveId: String) {
        let layout = layout(for: waveId)
        guard let focused = focusedPaneByWave[waveId],
              layout.pane(for: focused) == nil else {
            return
        }

        focusedPaneByWave[waveId] = layout.firstPane?.id
    }

    private func storageKey(repoKey: String, waveId: String? = nil, suffix: String) -> String {
        if let waveId {
            return "multiplexer.\(suffix).\(repoKey).\(waveId)"
        }
        return "multiplexer.\(suffix).\(repoKey)"
    }

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
}

public enum FocusDirection: Sendable {
    case next
    case previous
}
