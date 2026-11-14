import Foundation
import Loopflow

/// One repository's Tasks in a reading, in the Machine's order.
struct RepositoryTasks: Equatable {
    /// The repository's durable ID, as `WorkWorkspaceKey.repository` carries it.
    let repository: String
    /// Its path on the Machine, which is how Waves name their repository.
    let path: String
    let tasks: [WorkViewPlan.Content]

    var name: String { URL(fileURLWithPath: path).lastPathComponent }
}

typealias MobileTaskReading = Result<[RepositoryTasks], RegistryQueryError>

/// A reading and the wire bytes it was decoded from, which is what the phone keeps.
struct TaskListReading {
    let reading: MobileTaskReading
    let wire: Data
}

/// The Tasks a Work reading shows, in the Machine's order.
func mobileTasks(_ plan: WorkViewPlan) throws -> [WorkViewPlan.Content] {
    guard plan.resolution.unavailable.isEmpty else {
        throw RegistryQueryError(plan.resolution.unavailable.joined(separator: "; "))
    }
    let contents = Dictionary(plan.contents.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
    return try plan.projection.visible.filter { $0.work.kind == .task }.map { member in
        guard let task = contents[member.work] else {
            throw RegistryQueryError("The Machine returned an incomplete Task reading")
        }
        return task
    }
}

/// Whole replacement readings may coalesce. Checkpoint acknowledgements are
/// handled before coalescing, so refresh never waits on a discarded frame. A
/// lost connection resumes the same reading on the next one.
@MainActor
final class RemoteTaskObservation {
    let readings: AsyncThrowingStream<TaskListReading, any Error>
    private let continuation: AsyncThrowingStream<TaskListReading, any Error>.Continuation
    private let watch: ServerWatch<RawJSON>
    private var reader: Task<Void, Never>?
    private var nextCheckpoint: UInt64 = 0
    private var refreshing: (id: UInt64, continuation: CheckedContinuation<Void, any Error>)?
    private var closed = false

    init(query: ServedRead, connect: @escaping @Sendable () async throws -> ServerLink) {
        watch = ServerLink.subscribe(query, connect: connect) { update -> RawJSON? in
            guard case .reading(let frame) = update else { return nil }
            return frame
        }
        (readings, continuation) = AsyncThrowingStream.makeStream(bufferingPolicy: .bufferingNewest(1))
        reader = Task { [weak self, watch] in
            do {
                for try await frame in watch.updates {
                    guard let self, !self.closed else { return }
                    // A reading is tens to hundreds of kilobytes; decoding it here would
                    // stall a scroll for each one.
                    let reading = try await Task.detached { try Self.decode(frame: frame) }.value
                    guard !self.closed else { return }
                    self.receive(reading, wire: frame.bytes)
                }
                throw RegistryQueryError("Task updates ended. Refresh to reconnect.")
            } catch { self?.close(error: error) }
        }
    }

    func refresh() async throws {
        guard !closed else { throw RegistryQueryError("Task updates disconnected; reconnect for current Tasks") }
        guard refreshing == nil else { return }
        nextCheckpoint += 1
        let checkpoint = nextCheckpoint
        let timeout = Task { [weak self] in
            do { try await Task.sleep(for: RemoteMobileClient.requestDeadline) } catch { return }
            self?.close(error: RegistryQueryError("The Machine did not respond in time"))
        }
        defer { timeout.cancel() }
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                refreshing = (checkpoint, continuation)
                watch.checkpoint(checkpoint)
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.close() }
        }
    }

    func close() { close(error: CancellationError()) }

    private func close(error: any Error) {
        guard !closed else { return }
        closed = true
        watch.cancel()
        reader?.cancel()
        continuation.finish(throwing: error)
        refreshing?.continuation.resume(throwing: error)
        refreshing = nil
    }

    private func receive(_ reading: (answers: UInt64?, reading: MobileTaskReading), wire: Data) {
        continuation.yield(TaskListReading(reading: reading.reading, wire: wire))
        if let refreshing, let answers = reading.answers, answers >= refreshing.id {
            self.refreshing = nil
            refreshing.continuation.resume()
        }
    }

    nonisolated static func decode(frame: RawJSON) throws -> (answers: UInt64?, reading: MobileTaskReading) {
        struct Frame: Decodable {
            let answers: UInt64?
            let unavailable: String?
            let body: [WorkViewPlan]?
        }
        let reading = try frame.decode(Frame.self)
        if let reason = reading.unavailable {
            return (reading.answers, .failure(RegistryQueryError(reason)))
        }
        guard let plans = reading.body else { throw RegistryQueryError("Task reading is missing its contents") }
        // One repository that cannot answer leaves the others listed; the reading
        // fails only when none could.
        var failure: RegistryQueryError?
        let repositories = plans.map { plan in
            do { return RepositoryTasks(repository: plan.repository, path: plan.repositoryPath, tasks: try mobileTasks(plan)) } catch {
                failure = failure ?? (error as? RegistryQueryError) ?? RegistryQueryError(error.localizedDescription)
                return RepositoryTasks(repository: plan.repository, path: plan.repositoryPath, tasks: [])
            }
        }
        if let failure, repositories.allSatisfy(\.tasks.isEmpty), plans.allSatisfy({ !$0.resolution.unavailable.isEmpty }) {
            return (reading.answers, .failure(failure))
        }
        return (reading.answers, .success(repositories))
    }
}
