import Foundation
import Loopflow

/// Whole replacement readings may coalesce. Checkpoint acknowledgements are
/// handled before coalescing, so refresh never waits on a discarded frame.
@MainActor
final class RemoteTaskObservation: MobileTaskObservation {
    let readings: AsyncThrowingStream<MobileTaskReading, any Error>
    private let continuation: AsyncThrowingStream<MobileTaskReading, any Error>.Continuation
    private let connection: MachineConnection
    private let id: String
    private let instance: String
    private var reader: Task<Void, Never>?
    private var nextCheckpoint: UInt64 = 0
    private var refreshing: (id: UInt64, continuation: CheckedContinuation<Void, any Error>)?
    private var closed = false

    private init(connection: MachineConnection, first: MachineResponse) {
        self.connection = connection
        id = first.id
        instance = first.instance
        (readings, continuation) = AsyncThrowingStream.makeStream(bufferingPolicy: .bufferingNewest(1))
    }

    static func open(credential: MachineCredential, query: ServedRead) async throws -> RemoteTaskObservation {
        let connection = try MachineConnection(endpoint: credential.endpoint, token: credential.token)
        do {
            let first = try await connection.request(.subscribe(query, generation: 1))
            let observation = RemoteTaskObservation(connection: connection, first: first)
            try observation.receive(first)
            connection.keepAlive()
            observation.reader = Task { [weak observation] in
                guard let observation else { return }
                do {
                    while !observation.closed {
                        try observation.receive(await connection.receiveResponse(id: observation.id))
                    }
                } catch { observation.close(error: error) }
            }
            return observation
        } catch { connection.close(); throw error }
    }

    func refresh() async throws {
        guard !closed else { throw RegistryQueryError("Task updates disconnected; reconnect for current Tasks") }
        guard refreshing == nil else { return }
        nextCheckpoint += 1
        let checkpoint = nextCheckpoint
        let timeout = connection.deadline()
        defer { timeout.cancel() }
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                refreshing = (checkpoint, continuation)
                Task {
                    do { try await connection.sendFrame(Data("{\"id\":\(checkpoint)}".utf8)) }
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
        connection.close()
        reader?.cancel()
        continuation.finish(throwing: error)
        refreshing?.continuation.resume(throwing: error)
        refreshing = nil
    }

    private func receive(_ response: MachineResponse) throws {
        guard response.instance == instance else { throw RegistryQueryError("The Machine service changed; refresh Tasks") }
        let frame = try Self.decode(response)
        continuation.yield(frame.reading)
        if let refreshing, let answers = frame.answers, answers >= refreshing.id {
            self.refreshing = nil
            refreshing.continuation.resume()
        }
    }

    static func decode(_ response: MachineResponse) throws -> (answers: UInt64?, reading: MobileTaskReading) {
        guard case .reading(let generation, let frame) = response.result, generation == 1 else {
            throw RegistryQueryError("Received a different Task subscription")
        }
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
        do { return (reading.answers, .success(try plans.flatMap { try mobileTasks($0) })) }
        catch let error as RegistryQueryError { return (reading.answers, .failure(error)) }
    }
}
