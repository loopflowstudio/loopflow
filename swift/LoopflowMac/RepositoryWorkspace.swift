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
        let receive: (URL) async -> Void
    }
    private var targets: [String: Target] = [:]
    private var pending: [String: [URL]] = [:]
    private var delivering: Set<String> = []

    func register(_ incarnation: UUID, repository: String,
                  focus: @escaping () -> Void, receive: @escaping (URL) async -> Void) {
        targets[repository] = Target(incarnation: incarnation, focus: focus, receive: receive)
        deliverPending(repository)
    }

    func remove(_ incarnation: UUID, repository: String) {
        guard targets[repository]?.incarnation == incarnation else { return }
        targets[repository] = nil
    }

    /// False means the caller should request the identity-keyed SwiftUI window.
    @discardableResult
    func deliver(_ url: URL?, repository: String) -> Bool {
        if let url { pending[repository, default: []].append(url) }
        guard let target = targets[repository] else { return false }
        target.focus()
        deliverPending(repository)
        return true
    }

    /// Keep undelivered links here, not in a view's Task chain: removing a
    /// receiver must not discard its queue. Each repository progresses separately.
    private func deliverPending(_ repository: String) {
        guard targets[repository] != nil, pending[repository]?.isEmpty == false,
              delivering.insert(repository).inserted else { return }
        Task {
            defer { delivering.remove(repository) }
            while let target = targets[repository], let url = pending[repository]?.first {
                pending[repository]?.removeFirst()
                if pending[repository]?.isEmpty == true { pending[repository] = nil }
                await target.receive(url)
            }
        }
    }
}
