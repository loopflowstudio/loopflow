#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// A window's subscriptions, served by stand-ins or by a private Loopflow server.
/// Every window starts with the plan subscription.
@Suite("Workspace subscription transport", .serialized)
struct WorkObservationTransportTests {
    @Test("A server command completes from an async caller")
    func writeFromAsyncCaller() async throws {
        let repo = Self.home.appendingPathComponent("write-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: repo) }
        let lf = try BuiltLf(home: Self.home, cwd: repo)
        _ = try await lf.run(["init", "--quiet"], executable: "/usr/bin/git")

        _ = try await lf.run(["config", "repo", "--json"])
        let server = LocalServerFixture(lf)
        let printed = try await ServerClient.write(.graphCustomize(directory: repo.path, name: "code"), location: server.location())
        await server.stop()
        let path = try #require(try JSONSerialization.jsonObject(with: Data(printed.utf8)) as? [String: String])["path"]!
        let graph = try String(contentsOfFile: path, encoding: .utf8)
        #expect(graph.contains("graph: build"))
    }

    @Test("The server serves a read and ends it on cancel")
    func servedOverSocket() async throws {
        let lf = try BuiltLf(home: Self.home)
        _ = try await lf.run(["init", "--quiet"], executable: "/usr/bin/git")
        _ = try await lf.run(["config", "repo", "--json"])
        let server = LocalServerFixture(lf)
        let subscription = ServerClient.subscribe(.allPlans, location: server.location())
        var lines = subscription.lines.makeAsyncIterator()
        let first = try #require(try await lines.next())
        let frame = try #require(try JSONSerialization.jsonObject(with: first) as? [String: Any])
        #expect(frame.keys.contains("body"))
        subscription.checkpoint(7)
        let answered = try #require(try await lines.next())
        #expect((try JSONSerialization.jsonObject(with: answered) as? [String: Any])?["answers"] as? Int == 7)
        await subscription.cancel()
        await server.stop()
        #expect(try await lines.next() == nil)
    }

    @Test("A read Loopflow does not serve fails its subscription")
    func unservedRead() async throws {
        let lf = try BuiltLf(home: Self.home)
        _ = try await lf.run(["init", "--quiet"], executable: "/usr/bin/git")
        _ = try await lf.run(["config", "repo", "--json"])
        let server = LocalServerFixture(lf)
        let subscription = ServerClient.subscribe(.machineSelf, location: server.location())
        do {
            for try await _ in subscription.lines {}
            Issue.record("An unserved read finished successfully")
        } catch {
            #expect((error as? ServerError)?.code == .invalid)
        }
        await subscription.cancel()
        await server.stop()
    }

    @Test("A scope change keeps a subscription whose read is unchanged and starts the new ones")
    func scopeChange() async throws {
        let subscriptions = FakeSubscriptions()
        let observation = try WorkSubscriptions.observe(launch: subscriptions.launch)
        var frames = observation.frames.makeAsyncIterator()
        try subscriptions.answer(.allPlans, 0)
        #expect(try await frames.next()?.sequence == 1)
        observation.request(.scope(id: 2, WorkScope(repo: "/src/loopflow", headless: false, task: "LOO-1", activity: nil, taskListWave: nil)))
        // The plan answers from its last reading.
        #expect(try await frames.next()?.answers == 2)
        #expect(subscriptions.launched.contains(.taskGet(directory: "/src/loopflow", issue: "LOO-1")))
        #expect(subscriptions.launched.filter { $0 == .allPlans }.count == 1)
        await observation.cancel()
    }

    @Test("A checkpoint follows once every running subscription has answered it")
    func checkpointPerSubscription() async throws {
        let subscriptions = FakeSubscriptions()
        let observation = try WorkSubscriptions.observe(launch: subscriptions.launch)
        var frames = observation.frames.makeAsyncIterator()
        try subscriptions.answer(.allPlans, 0)
        #expect(try await frames.next()?.sequence == 1)
        observation.request(.scope(id: 2, WorkScope(repo: nil, headless: false, task: "LOO-1", activity: nil, taskListWave: nil)))
        #expect(try await frames.next()?.answers == 2)
        let task = ServedRead.taskGet(directory: "/", issue: "LOO-1")
        observation.request(.checkpoint(id: 5, replay: []))
        try subscriptions.answer(.allPlans, 5)
        #expect(try await frames.next()?.answers == 5)
        // The Task subscription has not answered, so nothing is confirmed yet.
        try subscriptions.answer(task, 5)
        let answered = try await frames.next()
        #expect(answered?.content.part == "task")
        let checkpoint = try await frames.next()
        #expect(checkpoint?.content.part == "checkpoint")
        #expect(checkpoint?.answers == 5)
        await observation.cancel()
    }

    @Test("Malformed frames and ended subscriptions fail rather than becoming empty",
          arguments: ["malformed", "wrong-type", "ended"])
    func invalidFrames(kind: String) async throws {
        let subscriptions = FakeSubscriptions()
        let observation = try WorkSubscriptions.observe(launch: subscriptions.launch)
        switch kind {
        case "malformed": subscriptions.send(.allPlans, "{bad}")
        case "wrong-type": subscriptions.send(.allPlans, #"{"answers":"not a number"}"#)
        default: subscriptions.end(.allPlans)
        }
        do {
            for try await _ in observation.frames {
                Issue.record("Invalid transport supplied a frame")
            }
            Issue.record("Invalid transport finished successfully")
        } catch {
            #expect(error is RegistryQueryError)
            if kind == "malformed" { #expect(error.localizedDescription.contains("Invalid planning subscription frame")) }
            if kind == "wrong-type" { #expect(error.localizedDescription.contains("answers")) }
        }
        await observation.cancel()
    }

    /// One private Machine for this serialized suite.
    private static let home: URL = {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }()
}

/// Subscriptions with no reader: a test sends each line a subscription would.
private final class FakeSubscriptions: @unchecked Sendable {
    private let lock = NSLock()
    private var subscriptions: [(read: ServedRead, lines: AsyncThrowingStream<Data, any Error>.Continuation, sent: [Int])] = []

    var launch: WorkSubscriptions.Launch {
        { [self] read in
            let (lines, continuation) = AsyncThrowingStream<Data, any Error>.makeStream()
            let index = lock.withLock { subscriptions.append((read, continuation, [])); return subscriptions.count - 1 }
            return LineSubscription(lines: lines, checkpoint: { [self] id in
                lock.withLock { subscriptions[index].sent.append(id) }
            }, cancel: { continuation.finish() }, unreadable: { nil })
        }
    }

    var launched: [ServedRead] { lock.withLock { subscriptions.map(\.read) } }

    func requests(_ read: ServedRead) -> [Int] {
        lock.withLock { subscriptions.first { $0.read == read }?.sent ?? [] }
    }

    func send(_ read: ServedRead, _ line: String) {
        _ = lock.withLock { subscriptions.first { $0.read == read }?.lines.yield(Data(line.utf8)) }
    }

    func end(_ read: ServedRead) {
        lock.withLock { subscriptions.first { $0.read == read }?.lines.finish() }
    }

    /// Answer request `id` (0 answers none) with a failed reading.
    func answer(_ read: ServedRead, _ id: Int) throws {
        try #require(id == 0 || requests(read).contains(id))
        send(read, id == 0 ? #"{"answers":null,"unavailable":"gone","body":null,"read_at":null}"#
             : #"{"answers":\#(id),"unavailable":"gone","body":null,"read_at":null}"#)
    }
}

#endif
