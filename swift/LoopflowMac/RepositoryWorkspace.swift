import Foundation

/// The plan identifies the window; the path is only its local opening locator.
/// SwiftUI compares values to reuse an existing window, including during opening.
struct RepositoryWorkspace: Codable, Hashable, Sendable {
    let id: String
    let path: String

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }
}

/// Queues are keyed before a window is requested. Registration order and focus
/// cannot change the destination. Receivers own their retained model/surfaces.
@MainActor
final class WorkLinkRouter {
    private struct Target {
        let incarnation: UUID
        let focus: () -> Void
        let receive: ([URL]) -> Void
    }
    private var targets: [String: Target] = [:]
    private var pending: [String: [URL]] = [:]

    func register(_ incarnation: UUID, repository: String,
                  focus: @escaping () -> Void, receive: @escaping ([URL]) -> Void) {
        targets[repository] = Target(incarnation: incarnation, focus: focus, receive: receive)
        if let links = pending.removeValue(forKey: repository), !links.isEmpty {
            receive(links)
        }
    }

    func remove(_ incarnation: UUID, repository: String) {
        guard targets[repository]?.incarnation == incarnation else { return }
        targets[repository] = nil
    }

    /// False means the caller should request the identity-keyed SwiftUI window.
    @discardableResult
    func deliver(_ url: URL?, repository: String) -> Bool {
        if let target = targets[repository] {
            target.focus()
            if let url { target.receive([url]) }
            return true
        }
        if let url { pending[repository, default: []].append(url) }
        return false
    }
}

