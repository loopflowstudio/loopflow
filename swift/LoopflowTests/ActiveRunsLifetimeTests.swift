#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Podium active Run lifetime", .serialized)
@MainActor
struct ActiveRunsLifetimeTests {
    @Test("Home replacement drains the old generation and rejects its late evidence")
    func replacement() async throws {
        let feed = ReplacingActiveRunsFeed()
        let query = RegistryQuery(watchActiveRuns: { await feed.open() }) { _, _ in "[]" }
        let model = PodiumModel(query: query)
        model.observeActiveRuns()
        try await waitForActiveRuns { model.activeRuns.value?.home == "/first" }
        await feed.replace()
        // Cancellation is deliberately held: the old Home disappears immediately,
        // while a replacement cannot start until that reader is reaped.
        try await waitForActiveRuns { model.activeRuns.isLoading }
        #expect(await feed.starts == 1)
        await feed.lateFrame()
        #expect(model.activeRuns.value == nil)
        await feed.release()
        try await waitForActiveRuns { model.activeRuns.value?.home == "/second" }
        #expect(await feed.starts == 2)
        await model.stopActiveRuns()
        #expect(await feed.cancellations == 2)
    }

    @Test("Window teardown releases the subscription even while its Monitor is hidden")
    func teardown() async throws {
        let feed = ActiveRunsTestFeed()
        let wire = #"{"discovery":"ready","home":"/fixture","observed_at":1,"task":null,"runs":[],"gaps":[]}"#
        let query = RegistryQuery(watchActiveRuns: { try await feed.open(initial: wire) }) { _, _ in "[]" }
        var model: PodiumModel? = PodiumModel(query: query)
        weak let reference = model
        model?.observeActiveRuns()
        try await waitForActiveRuns { model?.activeRuns.value != nil }
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

private actor ReplacingActiveRunsFeed {
    private var first: AsyncThrowingStream<ActiveRunsSnapshot, any Error>.Continuation?
    private var cancellation: CheckedContinuation<Void, Never>?
    private(set) var starts = 0
    private(set) var cancellations = 0

    func open() -> ActiveRunsObservation {
        starts += 1
        let index = starts
        let (stream, continuation) = AsyncThrowingStream<ActiveRunsSnapshot, any Error>
            .makeStream(bufferingPolicy: .bufferingNewest(1))
        if index == 1 { first = continuation }
        continuation.yield(snapshot(index == 1 ? "/first" : "/second"))
        return ActiveRunsObservation(snapshots: stream, request: { _ in }, cancel: {
            continuation.finish()
            await self.cancel(index)
        })
    }

    func replace() { first?.finish(throwing: ActiveRunsObservationError.configurationChanged) }
    func lateFrame() { first?.yield(snapshot("/first")) }
    func release() { cancellation?.resume(); cancellation = nil }
    private func cancel(_ index: Int) async {
        cancellations += 1
        if index == 1 { await withCheckedContinuation { cancellation = $0 } }
    }
    private func snapshot(_ home: String) -> ActiveRunsSnapshot {
        ActiveRunsSnapshot(discovery: .ready, home: home, observedAt: 1, task: nil, runs: [], gaps: [])
    }
}
#endif
