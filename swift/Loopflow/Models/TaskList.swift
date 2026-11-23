import Foundation

/// Which Tasks a list shows, as a view expression the server resolves. Desktop's
/// sidebar and the phone ask for the same `started` list: Working Tasks, each
/// carrying its states.
public enum TaskList: String, CaseIterable, Identifiable, Sendable {
    case started, available, busy, paused, backlog, done

    public var id: String { rawValue }

    public var state: WorkSelectionState {
        switch self {
        case .started: .working
        case .available: .available
        case .busy: .busy
        case .backlog: .backlog
        case .paused: .paused
        case .done: .done
        }
    }

    /// The view expression for these Tasks, within one Wave when `wave` names it.
    public func expression(wave: String?) -> String {
        let scope = wave.map { "wave:\($0) " } ?? ""
        return "\(scope)kind:task state:\(state.rawValue)"
    }
}
