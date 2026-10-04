#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Session refresh", .serialized)
@MainActor
struct SessionRefreshTests {
    @Test("Overlapping refreshes await one complete enumeration and retain its failure")
    func overlappingRefreshes() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session.json"))
        let record = try JSONDecoder().decode(SessionRecord.self, from: data)
        let page = "{\"entries\":[\(String(decoding: data, as: UTF8.self))],\"next\":\"end\"}"
        let source = SessionRefreshSource(firstPage: page)
        let model = PodiumModel(query: RegistryQuery { args, _ in
            try await source.read(lastPage: args.contains("--after"))
        }, repoPath: "/src/loopflow")
        let first = Task { await model.refreshSessions() }
        await source.waitUntilRequested()
        var second: Task<Void, Never>?
        await withCheckedContinuation { entered in
            second = Task {
                entered.resume()
                await model.refreshSessions()
            }
        }
        await source.release()
        await first.value
        await second?.value
        #expect(model.sessions.value?.map(\.id) == [record.id])
        #expect(await source.pages == 2)
        // A settled read is not a cache: the next request observes new failures
        // while keeping the complete last-good inventory.
        await source.fail()
        await model.refreshSessions()
        #expect(model.sessions.value?.map(\.id) == [record.id])
        #expect(model.sessions.errorMessage == "offline")
    }
}

private actor SessionRefreshSource {
    let firstPage: String
    private var pending: [CheckedContinuation<Void, Never>] = []
    private var requested: CheckedContinuation<Void, Never>?
    private var released = false
    private var failed = false
    private(set) var pages = 0

    init(firstPage: String) { self.firstPage = firstPage }

    func read(lastPage: Bool) async throws -> String {
        if failed { throw RegistryQueryError("offline") }
        pages += 1
        requested?.resume()
        requested = nil
        if !released { await withCheckedContinuation { pending.append($0) } }
        return lastPage ? #"{"entries":[],"next":null}"# : firstPage
    }

    func waitUntilRequested() async {
        if pages > 0 { return }
        await withCheckedContinuation { requested = $0 }
    }

    func release() {
        released = true
        for waiter in pending { waiter.resume() }
        pending.removeAll()
    }

    func fail() { failed = true }
}
#endif
