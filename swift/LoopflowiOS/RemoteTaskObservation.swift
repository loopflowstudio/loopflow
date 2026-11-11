import Foundation
import Loopflow

/// One repository's Tasks in a reading, in the Machine's order.
struct RepositoryTasks: Equatable {
    /// The repository's durable ID, as `WorkWorkspaceKey.repository` carries it.
    let repository: String
    let tasks: [WorkViewPlan.Content]
}

typealias MobileTaskReading = Result<[RepositoryTasks], RegistryQueryError>

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
/// handled before coalescing, so refresh never waits on a discarded frame.
@MainActor
final class RemoteTaskObservation {
    let readings: AsyncThrowingStream<MobileTaskReading, any Error>
    private let continuation: AsyncThrowingStream<MobileTaskReading, any Error>.Continuation
    private let subscription: MachineSubscription
    private var reader: Task<Void, Never>?
    private var nextCheckpoint: UInt64 = 0
    private var refreshing: (id: UInt64, continuation: CheckedContinuation<Void, any Error>)?
    private var closed = false

    private init(subscription: MachineSubscription) {
        self.subscription = subscription
        (readings, continuation) = AsyncThrowingStream.makeStream(bufferingPolicy: .bufferingNewest(1))
    }

    static func open(connection: MachineConnection, query: ServedRead) async throws -> RemoteTaskObservation {
        let subscription = try await connection.subscribe(query)
        let observation = RemoteTaskObservation(subscription: subscription)
        observation.reader = Task { [weak observation] in
            do {
                for try await frame in subscription.readings {
                    guard let observation, !observation.closed else { return }
                    try observation.receive(frame)
                }
                throw RegistryQueryError("Task updates ended. Refresh to reconnect.")
            } catch { observation?.close(error: error) }
        }
        return observation
    }

    func refresh() async throws {
        guard !closed else { throw RegistryQueryError("Task updates disconnected; reconnect for current Tasks") }
        guard refreshing == nil else { return }
        nextCheckpoint += 1
        let checkpoint = nextCheckpoint
        let timeout = Task { [weak self] in
            do { try await Task.sleep(for: .seconds(15)) } catch { return }
            self?.close(error: RegistryQueryError("The Machine did not respond in time"))
        }
        defer { timeout.cancel() }
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                refreshing = (checkpoint, continuation)
                Task {
                    do { try await subscription.checkpoint(checkpoint) }
                    catch { close(error: error) }
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.close() }
        }
    }

    func close() { close(error: CancellationError()) }

    private func close(error: any Error) {
        guard !closed else { return }
        closed = true
        subscription.cancel()
        reader?.cancel()
        continuation.finish(throwing: error)
        refreshing?.continuation.resume(throwing: error)
        refreshing = nil
    }

    private func receive(_ frame: JSONValue) throws {
        let reading = try Self.decode(frame: frame)
        continuation.yield(reading.reading)
        if let refreshing, let answers = reading.answers, answers >= refreshing.id {
            self.refreshing = nil
            refreshing.continuation.resume()
        }
    }

    static func decode(frame: JSONValue) throws -> (answers: UInt64?, reading: MobileTaskReading) {
        struct Frame: Decodable {
            let answers: UInt64?
            let unavailable: String?
            let body: [WorkViewPlan]?
        }
        let reading = try JSONDecoder().decode(Frame.self, from: JSONEncoder().encode(frame))
        if let reason = reading.unavailable {
            return (reading.answers, .failure(RegistryQueryError(reason)))
        }
        guard let plans = reading.body else { throw RegistryQueryError("Task reading is missing its contents") }
        do { return (reading.answers, .success(try plans.map { RepositoryTasks(repository: $0.repository, tasks: try mobileTasks($0)) })) }
        catch let error as RegistryQueryError { return (reading.answers, .failure(error)) }
    }
}
