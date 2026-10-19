import Foundation
import Loopflow
import Observation

/// The initial plan identifies the scene; the path is only its local opening locator.
/// The router retains that scene key across explicitly associated plan identities.
struct RepositoryWorkspace: Codable, Hashable, Sendable {
    let id: String
    let path: String

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }
}

/// Application lifetime, keyed by the stable repository scene resolved by WorkLinkRouter.
/// Closing a window releases its mounts, not its Work, drafts or terminal surfaces.
@MainActor
final class RepositoryWorkspaces {
    private var repositories: [String: SessionsWorkspaceRegistry] = [:]
    private let localMachineId: String?

    init(localMachineId: String? = nil) { self.localMachineId = localMachineId }

    func registry(for repository: String) -> SessionsWorkspaceRegistry {
        if let retained = repositories[repository] { return retained }
        let registry = SessionsWorkspaceRegistry(localMachineId: localMachineId)
        repositories[repository] = registry
        return registry
    }
}

/// Queues are keyed before a window is requested. Registration order and focus
/// cannot change the destination. Receivers mount application-retained workspaces.
@MainActor
@Observable
final class WorkLinkRouter {
    static let shared = WorkLinkRouter()

    private struct Target {
        let registration: UUID
        var incarnation: UUID
        let focus: () -> Void
        let receive: (URL) async -> Void
        let inspect: (String, UUID) -> DesktopWindowInspection
        let controlPane: (DesktopPaneCommand) throws -> String
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
    // Scene reservations survive registration and reassociation. Only an lf
    // identity reading can change their selected plan. Queues and native owners
    // keep the original scene key, so an in-flight delivery is never replayed.
    private struct RepositoryReading: Equatable {
        // A completed lookup is an observation, even when binding back to the
        // same plan. Scene equality deliberately ignores that distinction.
        let observation = UUID()
        let identity: RepositoryIdentity
        let path: String
    }
    private var workspaces: [String: RepositoryReading] = [:]
    // Previously resolved opening paths (including worktrees) locate retained
    // scenes for display only. New link delivery still needs a fresh reading.
    private var openingScenes: [String: String] = [:]
    private var targets: [String: Target] = [:]
    private var pending: [String: [URL]] = [:]
    private struct Delivery {
        let id: UUID
        let task: Task<Void, Never>
    }
    private var delivering: [String: Delivery] = [:]

    func register(_ incarnation: UUID, repository: String, openingRequests: Set<UUID> = [],
                  focus: @escaping () -> Void,
                  inspect: @escaping (String, UUID) -> DesktopWindowInspection,
                  controlPane: @escaping (DesktopPaneCommand) throws -> String,
                  readText: @escaping (DesktopTextRequest) throws -> DesktopTextReading,
                  receive: @escaping (URL) async -> Void) {
        let previous = targets[repository]
        if previous?.registration != incarnation {
            delivering.removeValue(forKey: repository)?.task.cancel()
        }
        let token = previous.flatMap { $0.registration == incarnation ? $0.incarnation : nil } ?? incarnation
        targets[repository] = Target(registration: incarnation, incarnation: token, focus: focus, receive: receive, inspect: inspect, controlPane: controlPane, readText: readText)
        confirmRegisteredWorkspace(repository, requests: openingRequests)
        deliverPending(repository)
    }

    /// Known content can mount before lookup; new destinations and link delivery
    /// still require resolution. Late reads cannot overwrite newer requests.
    func openRepository(path: String, link: URL?, query: RegistryQuery,
                        openWindow: (RepositoryWorkspace) -> Void) async throws -> RepositoryWorkspace {
        let path = path.normalizedFilePath
        var components = URLComponents()
        components.scheme = "loopflow"
        components.host = "open"
        components.queryItems = [URLQueryItem(name: "repo", value: path)]
        guard let url = link ?? components.url else { throw RegistryQueryError("Invalid repository opening path.") }
        try Task.checkCancellation()
        let id = UUID()
        repositoryOpenings[path] = RepositoryOpening(id: id, repository: nil,
            receipt: DesktopOpening(url: url.absoluteString, status: .opening, reason: nil))
        let observed = workspaces
        let retained = openingScenes[path].flatMap { retainedWorkspace($0) }
        if let retained, !deliver(nil, scene: retained.id) {
            openWindow(retained)
        }
        do {
            let resolved = try await Self.readRepository(path: path, query: query)
            try Task.checkCancellation()
            guard repositoryOpenings[path]?.id == id else { throw CancellationError() }
            let workspace = retainScene(resolved, observed: observed)
            openingScenes[path] = workspace.id
            openingScenes[workspace.path] = workspace.id
            var destination = link
            if let link, link.host == "task" {
                let target = try TaskLink(url: link)
                destination = TaskLink(issue: target.issue, repo: workspace.path, session: target.session, diff: target.diff, machine: target.machine, repository: target.repository).url
            }
            repositoryOpenings[path]?.repository = workspace.id
            if deliver(destination, scene: workspace.id) {
                finishRepositoryOpening(path, id: id, status: .usable)
            } else if retained?.id != workspace.id {
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

    func repositoryPath(_ repository: String) -> String? { workspaces[repository]?.path }

    /// Only a completed identity lookup supplies content for a reopened window.
    func retainedWorkspace(_ repository: String) -> RepositoryWorkspace? {
        workspaces[repository].map { RepositoryWorkspace(id: repository, path: $0.path) }
    }

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
        let observed = workspaces
        do {
            let retainedPath = workspaces[workspace.id]?.path ?? workspace.path
            var resolved = try? await Self.readRepository(path: retainedPath, query: query)
            try Task.checkCancellation()
            guard workspaceRequests(workspace.id) == requests else { throw CancellationError() }
            if resolved?.identity.locators.contains(workspace.id) != true {
                let path = try await query.repositoryPath(id: workspace.id)
                resolved = try await Self.readRepository(path: path, query: query)
            }
            guard let resolved else { throw RegistryQueryError("Repository location is unavailable.") }
            try Task.checkCancellation()
            guard resolved.identity.locators.contains(workspace.id) else {
                throw RegistryQueryError("This location no longer retains this repository identity. The restored workspace was not changed.")
            }
            guard workspaceRequests(workspace.id) == requests else {
                throw CancellationError()
            }
            // A second restored locator can resolve while the first scene is
            // still mounting. Reserve its existing scene before either registers.
            let scene = scene(for: resolved.identity) ?? workspace.id
            let retained = retainScene(resolved, scene: scene, observed: observed)
            openingScenes[workspace.path.normalizedFilePath] = retained.id
            openingScenes[retained.path] = retained.id
            return retained
        } catch {
            guard workspaceRequests(workspace.id) == requests else { throw CancellationError() }
            for (path, request) in repositoryOpenings where requests.contains(request.id) {
                finishRepositoryOpening(path, id: request.id, status: .failed,
                    reason: error is CancellationError ? "Repository opening canceled." : error.localizedDescription)
            }
            throw error
        }
    }

    private static func readRepository(path: String, query: RegistryQuery) async throws -> RepositoryReading {
        let localPath = await Task.detached {
            RepositoryPath().mainRepository(URL(fileURLWithPath: path))?.normalizedFilePath
        }.value
        guard let localPath else { throw RegistryQueryError("\(path) is not an available local Git repository.") }
        return try await RepositoryReading(identity: query.repositoryIdentity(cwd: localPath), path: localPath)
    }

    /// The common store proves association; paths, clone names and remotes do not.
    private func scene(for identity: RepositoryIdentity) -> String? {
        workspaces.first { scene, reading in
            identity.locators.contains(scene) || reading.identity.locators.contains(identity.id)
        }?.key
    }

    private func retainScene(_ reading: RepositoryReading, scene: String? = nil,
                             observed: [String: RepositoryReading]) -> RepositoryWorkspace {
        let scene = scene ?? self.scene(for: reading.identity) ?? reading.identity.id
        if workspaces[scene] != observed[scene], let current = workspaces[scene] {
            // Another locator completed an observation during this lookup.
            // Opening and restoration both retain it, including A → B → A.
            return RepositoryWorkspace(id: scene, path: current.path)
        }
        if let previous = workspaces[scene], previous.identity.id != reading.identity.id {
            // Renew only the targeting capability, not the native receiver or its
            // delivery lifetime. Even binding back cannot revive an old target.
            targets[scene]?.incarnation = UUID()
        }
        workspaces[scene] = reading
        return RepositoryWorkspace(id: scene, path: reading.path)
    }

    private func scene(for repository: String) -> String? {
        if let scene = workspaces.first(where: { $0.value.identity.id == repository })?.key { return scene }
        // Directly registered receivers have no pending scene reservation.
        return workspaces[repository] == nil && targets[repository] != nil ? repository : nil
    }

    private func finishRepositoryOpening(_ path: String, id: UUID, status: DesktopOpening.Status, reason: String? = nil) {
        guard let request = repositoryOpenings[path], request.id == id,
              request.receipt.status == .opening else { return }
        repositoryOpenings[path]?.receipt = DesktopOpening(url: request.receipt.url, status: status, reason: reason)
    }

    /// No focus change, reads, surface allocation, or provider launch.
    func inspect() -> DesktopInspection {
        DesktopInspection(observedAt: Int64(Date().timeIntervalSince1970), windows: targets.sorted { $0.key < $1.key }.map { scene, target in
            target.inspect(workspaces[scene]?.identity.id ?? scene, target.incarnation)
        }, openings: repositoryOpenings.sorted { $0.key < $1.key }.compactMap { _, request in
            // Task outcome ownership transfers to the receiving WorkModel.
            if request.receipt.status == .usable, URL(string: request.receipt.url)?.host == "task" { return nil }
            return request.receipt
        })
    }

    /// Validation and mutation stay in this MainActor turn; no focus fallback.
    @discardableResult
    func controlPane(_ request: DesktopPaneCommand) throws -> DesktopPaneReceipt {
        let receiver = try receiver(for: request.target)
        let pane = try receiver.controlPane(request)
        return DesktopPaneReceipt(target: request.target, pane: pane, observedAt: Int64(Date().timeIntervalSince1970))
    }

    /// Resolve and read synchronously; no client acquisition or focus fallback.
    func readText(_ request: DesktopTextRequest) throws -> DesktopTextReading {
        guard (1...1_048_576).contains(request.maxBytes) else {
            throw RegistryQueryError("Text byte limit must be between 1 and 1048576.")
        }
        return try receiver(for: request.target).readText(request)
    }

    private func receiver(for target: DesktopPaneTarget) throws -> Target {
        guard let scene = scene(for: target.repository), let receiver = targets[scene],
              receiver.incarnation.uuidString == target.window else {
            throw RegistryQueryError("The repository window was closed or replaced. Inspect Desktop again; no pane was changed.")
        }
        return receiver
    }

    func remove(_ incarnation: UUID, repository: String) {
        guard targets[repository]?.registration == incarnation else { return }
        targets[repository] = nil
        delivering.removeValue(forKey: repository)?.task.cancel()
    }

    /// False means the caller should request the identity-keyed SwiftUI window.
    @discardableResult
    func deliver(_ url: URL?, repository: String) -> Bool {
        deliver(url, scene: scene(for: repository) ?? repository)
    }

    private func deliver(_ url: URL?, scene: String) -> Bool {
        if let url { pending[scene, default: []].append(url) }
        guard let target = targets[scene] else { return false }
        target.focus()
        deliverPending(scene)
        return true
    }

    /// A link remains pending through the read, until its receiving window is
    /// still present at completion. Closing/replacing a window cancels its read
    /// and lets the replacement retry without waiting for that read to return.
    private func deliverPending(_ repository: String) {
        guard let target = targets[repository], pending[repository]?.isEmpty == false,
              delivering[repository] == nil else { return }
        let registration = target.registration
        let deliveryID = UUID()
        let task = Task {
            defer {
                if delivering[repository]?.id == deliveryID {
                    delivering[repository] = nil
                }
            }
            while !Task.isCancelled,
                  let receiver = targets[repository], receiver.registration == registration,
                  let url = pending[repository]?.first {
                await receiver.receive(url)
                guard !Task.isCancelled, targets[repository]?.registration == registration else { return }
                pending[repository]?.removeFirst()
                if pending[repository]?.isEmpty == true { pending[repository] = nil }
            }
        }
        delivering[repository] = Delivery(id: deliveryID, task: task)
    }
}
