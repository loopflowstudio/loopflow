import Foundation
import Loopflow

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
    static let shared = WorkLinkRouter()

    private struct Target {
        let incarnation: UUID
        let focus: () -> Void
        let receive: (URL) async -> Void
        let inspect: (UUID) -> DesktopWindowInspection
    }
    private var targets: [String: Target] = [:]
    private var pending: [String: [URL]] = [:]
    private struct Delivery {
        let id: UUID
        let task: Task<Void, Never>
    }
    private var delivering: [String: Delivery] = [:]

    func register(_ incarnation: UUID, repository: String,
                  focus: @escaping () -> Void,
                  inspect: @escaping (UUID) -> DesktopWindowInspection,
                  receive: @escaping (URL) async -> Void) {
        if targets[repository]?.incarnation != incarnation {
            delivering.removeValue(forKey: repository)?.task.cancel()
        }
        targets[repository] = Target(incarnation: incarnation, focus: focus, receive: receive, inspect: inspect)
        deliverPending(repository)
    }

    /// No focus change, reads, surface allocation, or provider launch.
    func inspect() -> DesktopInspection {
        DesktopInspection(observedAt: Int64(Date().timeIntervalSince1970), windows: targets.keys.sorted().compactMap { key in
            targets[key].map { $0.inspect($0.incarnation) }
        })
    }

    func remove(_ incarnation: UUID, repository: String) {
        guard targets[repository]?.incarnation == incarnation else { return }
        targets[repository] = nil
        delivering.removeValue(forKey: repository)?.task.cancel()
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

    /// A link remains pending through the read, until its receiving window is
    /// still present at completion. Closing/replacing a window cancels its read
    /// and lets the replacement retry without waiting for that read to return.
    private func deliverPending(_ repository: String) {
        guard let target = targets[repository], pending[repository]?.isEmpty == false,
              delivering[repository] == nil else { return }
        let incarnation = target.incarnation
        let deliveryID = UUID()
        let task = Task {
            defer {
                if delivering[repository]?.id == deliveryID {
                    delivering[repository] = nil
                }
            }
            while !Task.isCancelled,
                  let receiver = targets[repository], receiver.incarnation == incarnation,
                  let url = pending[repository]?.first {
                await receiver.receive(url)
                guard !Task.isCancelled, targets[repository]?.incarnation == incarnation else { return }
                pending[repository]?.removeFirst()
                if pending[repository]?.isEmpty == true { pending[repository] = nil }
            }
        }
        delivering[repository] = Delivery(id: deliveryID, task: task)
    }
}
