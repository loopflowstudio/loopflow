#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// A window's watches, served by stand-ins or by Loopflow's library in the app.
/// Every window starts with the plan and the live-process watch.
@Suite("Workspace watch transport", .serialized)
struct WorkObservationTransportTests {
    @Test("Loopflow's library serves a read in the app and ends it on cancel")
    func servedInProcess() async throws {
        setenv("LF_HOME", Self.home.path, 1)
        defer { unsetenv("LF_HOME") }
        let watch = try LoopflowLibrary.watch(.desktopPlan)
        var lines = watch.lines.makeAsyncIterator()
        let first = try #require(try await lines.next())
        let frame = try #require(try JSONSerialization.jsonObject(with: first) as? [String: Any])
        #expect(frame.keys.contains("body"))
        watch.send(Data(#"{"id":7}"#.utf8 + [10]))
        let answered = try #require(try await lines.next())
        #expect((try JSONSerialization.jsonObject(with: answered) as? [String: Any])?["answers"] as? Int == 7)
        await watch.cancel()
        #expect(try await lines.next() == nil)
    }

    @Test("An operation Loopflow does not serve fails its watch")
    func unservedOperation() async throws {
        setenv("LF_HOME", Self.home.path, 1)
        defer { unsetenv("LF_HOME") }
        let watch = try LoopflowLibrary.watch(.desktopTask(directory: "/", issue: "NONE-404"))
        do {
            for try await _ in watch.lines {}
            Issue.record("An unserved operation finished successfully")
        } catch {
            #expect(error is RegistryQueryError)
        }
        await watch.cancel()
    }

    @Test("A scope change keeps a watch whose read is unchanged and starts the new ones")
    func scopeChange() async throws {
        let watches = FakeWatches()
        let observation = try WorkWatches.observe(launch: watches.launch)
        var frames = observation.frames.makeAsyncIterator()
        try await watches.answer(.desktopPlan, 0)
        #expect(try await frames.next()?.sequence == 1)
        observation.request(.scope(id: 2, WorkScope(repo: "/src/loopflow", headless: false, task: "LOO-1", activity: nil)))
        // The plan answers from its last reading.
        #expect(try await frames.next()?.answers == 2)
        #expect(watches.launched.contains(.desktopTask(directory: "/src/loopflow", issue: "LOO-1")))
        #expect(watches.launched.filter { $0 == .desktopPlan }.count == 1)
        await observation.cancel()
    }

    @Test("A checkpoint follows once every running watch has answered it")
    func checkpointPerWatch() async throws {
        let watches = FakeWatches()
        let observation = try WorkWatches.observe(launch: watches.launch)
        var frames = observation.frames.makeAsyncIterator()
        observation.request(.checkpoint(id: 5, replay: []))
        try await watches.answer(.desktopPlan, 5)
        #expect(try await frames.next()?.answers == 5)
        #expect(watches.requests(.monitorPs) == [#"{"id":5}"#])
        // The live-process watch has not answered, so nothing is confirmed yet.
        try await watches.answer(.monitorPs, 5)
        let answered = try await frames.next()
        #expect(answered?.content.part == "activity")
        let checkpoint = try await frames.next()
        #expect(checkpoint?.content.part == "checkpoint")
        #expect(checkpoint?.answers == 5)
        await observation.cancel()
    }

    @Test("Malformed frames and ended watches fail rather than becoming empty",
          arguments: ["malformed", "wrong-type", "ended"])
    func invalidFrames(kind: String) async throws {
        let watches = FakeWatches()
        let observation = try WorkWatches.observe(launch: watches.launch)
        switch kind {
        case "malformed": watches.send(.desktopPlan, "{bad}")
        case "wrong-type": watches.send(.desktopPlan, #"{"answers":"not a number"}"#)
        default: watches.end(.desktopPlan)
        }
        do {
            for try await _ in observation.frames {
                Issue.record("Invalid transport supplied a frame")
            }
            Issue.record("Invalid transport finished successfully")
        } catch {
            #expect(error is RegistryQueryError)
            if kind == "malformed" { #expect(error.localizedDescription.contains("Invalid planning watch frame")) }
            if kind == "wrong-type" { #expect(error.localizedDescription.contains("answers")) }
        }
        await observation.cancel()
    }

    @Test("The line reader accepts its frame limit and rejects the next byte", arguments: [false, true])
    func frameLimit(terminated: Bool) async throws {
        let allowed = String(repeating: "x", count: 32)
        let oversized = allowed + "x" + (terminated ? "\n" : "")
        let process = shell("printf '%s\\n' '\(allowed)'; printf '%s' '\(oversized)'; IFS= read -r request")
        let reader = try LocalLineObservation<Data>.start(
            name: "Workspace", process: process, frameLimit: 32,
            configurationChanged: { false }, decode: { $0 }
        )
        var frames = reader.frames.makeAsyncIterator()
        #expect(try await frames.next() == Data(allowed.utf8))
        do {
            _ = try await frames.next()
            Issue.record("Oversized frame was accepted")
        } catch {
            #expect(error.localizedDescription.contains("frame exceeds"))
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Silence after the first frame expires a reader that must speak, not a watch")
    func stalled() async throws {
        let process = shell("echo ready; exec /bin/sleep 30")
        let reader = try LocalLineObservation<Data>.start(
            name: "Task files", process: process, frameLimit: 32,
            configurationChanged: { false }, decode: { $0 })
        var iterator = reader.frames.makeAsyncIterator()
        #expect(try await iterator.next() == Data("ready".utf8))
        do {
            _ = try await iterator.next()
            Issue.record("A stalled pipe was accepted")
        } catch {
            #expect(error.localizedDescription.contains("10 seconds"))
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    /// One Machine for the library's reads: Loopflow may keep its store open
    /// for the rest of this process.
    private static let home: URL = {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }()
}

/// Watches with no reader: a test sends each line a watch would.
private final class FakeWatches: @unchecked Sendable {
    private let lock = NSLock()
    private var watches: [(operation: WorkOperation, lines: AsyncThrowingStream<Data, any Error>.Continuation, sent: [String])] = []

    var launch: WorkWatches.Launch {
        { [self] operation in
            let (lines, continuation) = AsyncThrowingStream<Data, any Error>.makeStream()
            let index = lock.withLock { watches.append((operation, continuation, [])); return watches.count - 1 }
            return LineWatch(lines: lines, send: { [self] line in
                lock.withLock { watches[index].sent.append(String(decoding: line, as: UTF8.self)
                    .trimmingCharacters(in: .newlines)) }
            }, cancel: { continuation.finish() })
        }
    }

    var launched: [WorkOperation] { lock.withLock { watches.map(\.operation) } }

    func requests(_ operation: WorkOperation) -> [String] {
        lock.withLock { watches.first { $0.operation == operation }?.sent ?? [] }
    }

    func send(_ operation: WorkOperation, _ line: String) {
        _ = lock.withLock { watches.first { $0.operation == operation }?.lines.yield(Data(line.utf8)) }
    }

    func end(_ operation: WorkOperation) {
        lock.withLock { watches.first { $0.operation == operation }?.lines.finish() }
    }

    /// Answer request `id` (0 answers none) with a failed reading.
    func answer(_ operation: WorkOperation, _ id: Int) async throws {
        for _ in 0..<400 where id != 0 && !requests(operation).contains(#"{"id":\#(id)}"#) {
            try await Task.sleep(for: .milliseconds(5))
        }
        send(operation, id == 0 ? #"{"answers":null,"unavailable":"gone","body":null}"#
             : #"{"answers":\#(id),"unavailable":"gone","body":null}"#)
    }
}

private func shell(_ script: String, cwd: URL? = nil) -> Process {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/sh")
    process.arguments = ["-c", script]
    process.currentDirectoryURL = cwd
    process.environment = ["PATH": "/usr/bin:/bin"]
    return process
}
#endif
