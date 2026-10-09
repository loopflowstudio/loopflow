import Foundation
import Loopflow
import Observation

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
@Observable
final class WorkLinkRouter {
    static let shared = WorkLinkRouter()

    private struct Target {
        let incarnation: UUID
        let focus: () -> Void
        let receive: (URL) async -> Void
        let inspect: (UUID) -> DesktopWindowInspection
        let controlPane: (DesktopPaneCommand) throws -> Void
        let readText: (DesktopTextRequest) throws -> DesktopTextReading
    }
    private struct RepositoryOpening {
        let id: UUID
        var repository: String?
        var receipt: DesktopOpening
    }
    // A locator exists before plan lookup can succeed. These receipts never
    // fabricate a window or supply Work identity from a filesystem path.
    private var repositoryOpenings: [String: RepositoryOpening] = [:]
    private var targets: [String: Target] = [:]
    private var pending: [String: [URL]] = [:]
    private struct Delivery {
        let id: UUID
        let task: Task<Void, Never>
    }
    private var delivering: [String: Delivery] = [:]

    func register(_ incarnation: UUID, repository: String, openingRequests: Set<UUID> = [],
                  focus: @escaping () -> Void,
                  inspect: @escaping (UUID) -> DesktopWindowInspection,
                  controlPane: @escaping (DesktopPaneCommand) throws -> Void,
                  readText: @escaping (DesktopTextRequest) throws -> DesktopTextReading,
                  receive: @escaping (URL) async -> Void) {
        if targets[repository]?.incarnation != incarnation {
            delivering.removeValue(forKey: repository)?.task.cancel()
        }
        targets[repository] = Target(incarnation: incarnation, focus: focus, receive: receive, inspect: inspect, controlPane: controlPane, readText: readText)
        confirmRegisteredWorkspace(repository, requests: openingRequests)
        deliverPending(repository)
    }

    /// All entry points resolve before opening, including a repeated plain repo
    /// request. Late reads may neither open a window nor overwrite newer feedback.
    func openRepository(path: String, link: URL?, query: RegistryQuery,
                        openWindow: (RepositoryWorkspace) -> Void) async throws -> RepositoryWorkspace {
        let path = path.normalizedFilePath
        var components = URLComponents()
        components.scheme = "loopflow"
        components.host = "open"
        components.queryItems = [URLQueryItem(name: "repo", value: path)]
        guard let url = link ?? components.url else { throw RegistryQueryError("Invalid repository opening path.") }
        let id = UUID()
        repositoryOpenings[path] = RepositoryOpening(id: id, repository: nil,
            receipt: DesktopOpening(url: url.absoluteString, status: .opening, reason: nil))
        do {
            let workspace = try await RepositoryWorkspace.resolve(path: path, query: query)
            try Task.checkCancellation()
            guard repositoryOpenings[path]?.id == id else { throw CancellationError() }
            var destination = link
            if let link {
                let target = try TaskLink(url: link)
                destination = TaskLink(issue: target.issue, repo: workspace.path, session: target.session, diff: target.diff).url
            }
            repositoryOpenings[path]?.repository = workspace.id
            if deliver(destination, repository: workspace.id) {
                finishRepositoryOpening(path, id: id, status: .usable)
            } else {
                openWindow(workspace)
            }
            return workspace
        } catch {
            guard repositoryOpenings[path]?.id == id else { throw CancellationError() }
            finishRepositoryOpening(path, id: id, status: .failed,
                reason: error is CancellationError ? "Repository opening canceled." : error.localizedDescription)
            throw error
        }
    }

    func hasWindow(_ repository: String) -> Bool { targets[repository] != nil }

    func confirmRegisteredWorkspace(_ repository: String, requests: Set<UUID>) {
        guard !Task.isCancelled, hasWindow(repository) else { return }
        for (path, request) in repositoryOpenings where request.repository == repository && requests.contains(request.id) {
            finishRepositoryOpening(path, id: request.id, status: .usable)
        }
    }

    func workspaceRequests(_ repository: String) -> Set<UUID> {
        Set(repositoryOpenings.values.compactMap {
            $0.repository == repository ? $0.id : nil
        })
    }

    /// Restored scenes use the same resolver. Failure belongs to requests that
    /// were waiting when this validation began, never arrivals during the read.
    func resolveWorkspace(_ workspace: RepositoryWorkspace, query: RegistryQuery) async throws -> RepositoryWorkspace {
        let requests = workspaceRequests(workspace.id)
        do {
            let current = try await RepositoryWorkspace.resolve(path: workspace.path, query: query)
            try Task.checkCancellation()
            guard current.id == workspace.id else {
                throw RegistryQueryError("This location now selects another repository plan. Open it explicitly from Open Repo; the restored workspace was not changed.")
            }
            guard workspaceRequests(workspace.id) == requests else {
                throw CancellationError()
            }
            return current
        } catch {
            guard workspaceRequests(workspace.id) == requests else { throw CancellationError() }
            for (path, request) in repositoryOpenings where requests.contains(request.id) {
                finishRepositoryOpening(path, id: request.id, status: .failed,
                    reason: error is CancellationError ? "Repository opening canceled." : error.localizedDescription)
            }
            throw error
        }
    }

    private func finishRepositoryOpening(_ path: String, id: UUID, status: DesktopOpening.Status, reason: String? = nil) {
        guard let request = repositoryOpenings[path], request.id == id,
              request.receipt.status == .opening else { return }
        repositoryOpenings[path]?.receipt = DesktopOpening(url: request.receipt.url, status: status, reason: reason)
    }

    /// No focus change, reads, surface allocation, or provider launch.
    func inspect() -> DesktopInspection {
        DesktopInspection(observedAt: Int64(Date().timeIntervalSince1970), windows: targets.sorted { $0.key < $1.key }.map { _, target in
            target.inspect(target.incarnation)
        }, openings: repositoryOpenings.sorted { $0.key < $1.key }.compactMap { _, request in
            // Task outcome ownership transfers to the receiving WorkModel.
            if request.receipt.status == .usable, URL(string: request.receipt.url)?.host == "task" { return nil }
            return request.receipt
        })
    }

    /// Validation and mutation stay in this MainActor turn; no focus fallback.
    func controlPane(_ request: DesktopPaneCommand) throws -> DesktopInspection {
        let receiver = try receiver(for: request.target)
        try receiver.controlPane(request)
        return inspect()
    }

    /// Resolve and read synchronously; no client acquisition or focus fallback.
    func readText(_ request: DesktopTextRequest) throws -> DesktopTextReading {
        guard (1...1_048_576).contains(request.maxBytes) else {
            throw RegistryQueryError("Text byte limit must be between 1 and 1048576.")
        }
        return try receiver(for: request.target).readText(request)
    }

    private func receiver(for target: DesktopPaneTarget) throws -> Target {
        guard let receiver = targets[target.repository],
              receiver.incarnation.uuidString == target.window else {
            throw RegistryQueryError("The repository window was closed or replaced. Inspect Desktop again; no pane was changed.")
        }
        return receiver
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
