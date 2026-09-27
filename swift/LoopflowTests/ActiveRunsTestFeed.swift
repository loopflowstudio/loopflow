import Foundation
import Testing
@testable import Loopflow

/// Controlled wire observations; production consumers still decode shared DTOs.
actor ActiveRunsTestFeed {
    private var continuation: AsyncThrowingStream<ActiveRunsSnapshot, any Error>.Continuation?
    private(set) var starts = 0
    private(set) var cancellations = 0
    private(set) var requests: [ActiveRunsObservation.Request] = []

    func open(initial: String? = nil) throws -> ActiveRunsObservation {
        let (stream, continuation) = AsyncThrowingStream<ActiveRunsSnapshot, any Error>
            .makeStream(bufferingPolicy: .bufferingNewest(1))
        self.continuation = continuation
        starts += 1
        if let initial { try send(initial) }
        return ActiveRunsObservation(snapshots: stream, request: { await self.request($0) }, cancel: {
            continuation.finish()
            await self.cancelled()
        })
    }

    func send(_ json: String) throws {
        continuation?.yield(try JSONDecoder().decode(ActiveRunsSnapshot.self, from: Data(json.utf8)))
    }

    func fail(_ error: any Error = RegistryQueryError("read failed")) { continuation?.finish(throwing: error) }
    private func request(_ request: ActiveRunsObservation.Request) { requests.append(request) }
    private func cancelled() { cancellations += 1 }
}

@MainActor
func waitForActiveRuns(_ predicate: () -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(3)
    while !predicate(), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    try #require(predicate())
}
