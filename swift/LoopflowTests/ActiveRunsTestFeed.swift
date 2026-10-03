import Foundation
import Testing
@testable import Loopflow

/// Controlled wire observations; production consumers still decode shared DTOs.
actor ActiveSessionsTestFeed {
    private var continuation: AsyncThrowingStream<ActiveSessionsSnapshot, any Error>.Continuation?
    private(set) var starts = 0
    private(set) var cancellations = 0
    private(set) var requests: [ActiveSessionsObservation.Request] = []

    func open(initial: String? = nil) throws -> ActiveSessionsObservation {
        let (stream, continuation) = AsyncThrowingStream<ActiveSessionsSnapshot, any Error>
            .makeStream(bufferingPolicy: .bufferingNewest(1))
        self.continuation = continuation
        starts += 1
        if let initial { try send(initial) }
        return ActiveSessionsObservation(snapshots: stream, request: { await self.request($0) }, cancel: {
            continuation.finish()
            await self.cancelled()
        })
    }

    func send(_ json: String) throws {
        continuation?.yield(try JSONDecoder().decode(ActiveSessionsSnapshot.self, from: Data(json.utf8)))
    }

    func fail(_ error: any Error = RegistryQueryError("read failed")) { continuation?.finish(throwing: error) }
    private func request(_ request: ActiveSessionsObservation.Request) { requests.append(request) }
    private func cancelled() { cancellations += 1 }
}

@MainActor
func waitForActiveSessions(_ predicate: () -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(3)
    while !predicate(), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    try #require(predicate())
}
