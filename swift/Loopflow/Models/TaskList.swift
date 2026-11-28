import Foundation

/// The Task list Desktop and iPhone request from the Machine.
public enum TaskList: String, CaseIterable, Identifiable, Sendable {
    case started, available, busy, backlog, done

    public var id: String { rawValue }

    public var state: WorkSelectionState {
        switch self {
        case .started: .working
        case .available: .available
        case .busy: .busy
        case .backlog: .backlog
        case .done: .done
        }
    }

    /// The view expression for these Tasks, within one Wave when `wave` names it.
    public func expression(wave: String?) -> String {
        let scope = wave.map { "wave:\($0) " } ?? ""
        return "\(scope)kind:task state:\(state.rawValue)"
    }
}
