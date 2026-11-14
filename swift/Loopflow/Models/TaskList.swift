import Foundation

/// Which Tasks a list shows, as a view expression the server resolves. Desktop's
/// sidebar and the phone ask for the same `started` list: Working Tasks, then
/// Autonomous ones, each carrying its states.
public enum TaskList: String, CaseIterable, Identifiable, Sendable {
    case started, available, busy, backlog, done

    public var id: String { rawValue }

    private var states: [String] {
        switch self {
        case .started: ["working", "autonomous"]
        case .available: ["available"]
        case .busy: ["busy"]
        case .backlog: ["backlog"]
        case .done: ["done"]
        }
    }

    /// The view expression for these Tasks, within one Wave when `wave` names it.
    public func expression(wave: String?) -> String {
        let scope = wave.map { "wave:\($0) " } ?? ""
        return states.map { "\(scope)kind:task state:\($0)" }.joined(separator: " or ")
    }
}
