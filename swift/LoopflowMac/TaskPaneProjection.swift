import Loopflow

/// A read-only reference into a Task's one retained layout, never a new pane.
enum TaskPaneProjection: Equatable {
    case pane(id: String)
    case missingPrimary
    case missingPane(sessionId: String)
    case ambiguousPrimary

    @MainActor
    static func resolve(in store: MultiplexerStore, sessions: [SessionRecord]) -> Self {
        if let zoom = store.zoomedPaneId, store.layout.pane(for: zoom) != nil {
            return .pane(id: zoom)
        }
        let primary = sessions.filter { $0.taskPrimary && $0.state != .closed }
        guard let session = primary.first else { return .missingPrimary }
        guard primary.count == 1 else { return .ambiguousPrimary }
        guard let pane = store.pane(forSessionId: session.id) else {
            return .missingPane(sessionId: session.id)
        }
        return .pane(id: pane.id)
    }
}
