#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Work active Session lifetime", .serialized)
@MainActor
struct ActiveSessionsLifetimeTests {
    @Test("Home replacement drains the old generation and rejects its late evidence")
    func replacement() async throws {
        let feed = ReplacingActiveSessionsFeed()
        let query = RegistryQuery(watchActiveSessions: { await feed.open() }) { _, _ in "[]" }
        let model = WorkModel(query: query)
        model.observeActiveSessions()
        try await waitForActiveSessions { model.activeSessions.value?.home == "/first" }
        await feed.replace()
        // Cancellation is deliberately held: the old Home disappears immediately,
        // while a replacement cannot start until that reader is reaped.
        try await waitForActiveSessions { model.activeSessions.isLoading }
        #expect(await feed.starts == 1)
        await feed.lateFrame()
        #expect(model.activeSessions.value == nil)
        await feed.release()
        try await waitForActiveSessions { model.activeSessions.value?.home == "/second" }
        #expect(await feed.starts == 2)
        await model.stopActiveSessions()
        #expect(await feed.cancellations == 2)
    }

    @Test("Window teardown releases the subscription even while its Monitor is hidden")
    func teardown() async throws {
        let feed = ActiveSessionsTestFeed()
        let wire = #"{"discovery":"ready","home":"/fixture","observed_at":1,"task":null,"sessions":[],"gaps":[]}"#
        let query = RegistryQuery(watchActiveSessions: { try await feed.open(initial: wire) }) { _, _ in "[]" }
        var model: WorkModel? = WorkModel(query: query)
        weak var reference = model
        model?.observeActiveSessions()
        try await waitForActiveSessions { model?.activeSessions.value != nil }
        model?.setRepoPath("/other/repo")
        model = nil
        #expect(reference == nil)
        let deadline = ContinuousClock.now + .seconds(3)
        while await feed.cancellations == 0, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(await feed.cancellations == 1)
    }
}

private actor ReplacingActiveSessionsFeed {
    private var first: AsyncThrowingStream<ActiveSessionsSnapshot, any Error>.Continuation?
    private var cancellation: CheckedContinuation<Void, Never>?
    private(set) var starts = 0
    private(set) var cancellations = 0

    func open() -> ActiveSessionsObservation {
        starts += 1
        let index = starts
        let (stream, continuation) = AsyncThrowingStream<ActiveSessionsSnapshot, any Error>
            .makeStream(bufferingPolicy: .bufferingNewest(1))
        if index == 1 { first = continuation }
        continuation.yield(snapshot(index == 1 ? "/first" : "/second"))
        return ActiveSessionsObservation(snapshots: stream, request: { _ in }, cancel: {
            continuation.finish()
            await self.cancel(index)
        })
    }

    func replace() { first?.finish(throwing: ActiveSessionsObservationError.configurationChanged) }
    func lateFrame() { first?.yield(snapshot("/first")) }
    func release() { cancellation?.resume(); cancellation = nil }
    private func cancel(_ index: Int) async {
        cancellations += 1
        if index == 1 { await withCheckedContinuation { cancellation = $0 } }
    }
    private func snapshot(_ home: String) -> ActiveSessionsSnapshot {
        ActiveSessionsSnapshot(discovery: .ready, home: home, observedAt: 1, task: nil, sessions: [], gaps: [])
    }
}
#endif
