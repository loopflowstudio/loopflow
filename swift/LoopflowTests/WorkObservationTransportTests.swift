#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// The pipes under a window's `lf … --json --watch` processes, driven by
/// stand-in readers. Every window starts with the plan and the live-process watch.
@Suite("Workspace watch transport", .serialized)
struct WorkObservationTransportTests {
    @Test("Watch startup failure preserves the CLI diagnostic", arguments: [false, true])
    func startupFailure(closesOutputFirst: Bool) async throws {
        let diagnosis = "Selected Machine has an incompatible migration frontier"
        let shutdown = closesOutputFirst ? "exec 1>&-; IFS= read -r request; exit 23" : "exit 23"
        let launched = Launched { _ in "printf '%s\\n' '\(diagnosis)' >&2; \(shutdown)" }
        let reader = try launched.observe()
        do {
            for try await _ in reader.frames {
                Issue.record("Failed watch supplied a frame")
            }
            Issue.record("Failed watch finished successfully")
        } catch {
            #expect(error.localizedDescription.contains(diagnosis))
        }
        await reader.cancel()
        #expect(launched.processes.allSatisfy { !$0.isRunning })
    }

    @Test("A consumer that starts late receives every frame in order")
    func everyFrame() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        try Data((1...200).map { frame(answers: $0) }.joined().utf8).write(to: directory.appendingPathComponent("frames"))
        let launched = Launched(cwd: directory) { args in
            args.starts(with: ["desktop", "plan"]) ? "/bin/cat frames; IFS= read -r request" : "IFS= read -r request"
        }
        let reader = try launched.observe()
        var frames = reader.frames.makeAsyncIterator()
        for expected in 1...200 {
            let next = try await frames.next()
            #expect(next?.sequence == expected)
            #expect(next?.answers == expected)
        }
        await reader.cancel()
        #expect(launched.processes.allSatisfy { !$0.isRunning })
    }

    @Test("Fragmented frames, stderr backpressure and requests reach their watch")
    func pipesAndRequests() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        try Data(frame(answers: nil).utf8).write(to: directory.appendingPathComponent("first"))
        try Data(frame(answers: 1).utf8).write(to: directory.appendingPathComponent("refreshed"))
        let launched = Launched(cwd: directory) { args in
            guard args.starts(with: ["desktop", "plan"]) else { return "while IFS= read -r request; do :; done" }
            return """
            /usr/bin/head -c 131072 /dev/zero >&2
            /usr/bin/head -c 25 first
            /bin/sleep 0.05
            /usr/bin/tail -c +26 first
            while IFS= read -r request; do
                case "$request" in
                    *'"id":1'*) /bin/cat refreshed ;;
                esac
            done
            """
        }
        let reader = try launched.observe()
        var frames = reader.frames.makeAsyncIterator()
        #expect(try await frames.next()?.sequence == 1)
        reader.request(.refresh(id: 1))
        #expect(try await frames.next()?.answers == 1)
        // A scope that keeps the plan answers from its last reading and starts
        // the Task's and repository's watches.
        reader.request(.scope(id: 2, WorkScope(repo: "/src/loopflow", headless: false, task: "LOO-1", activity: nil)))
        #expect(try await frames.next()?.answers == 2)
        try await eventually { launched.arguments.contains { $0.starts(with: ["desktop", "task", "LOO-1"]) } }
        #expect(launched.arguments.filter { $0.starts(with: ["desktop", "plan"]) }.count == 1)
        await reader.cancel()
        #expect(launched.processes.allSatisfy { !$0.isRunning && $0.terminationStatus == 0 })
    }

    @Test("A checkpoint follows once every running watch has answered it")
    func checkpointPerWatch() async throws {
        let watches = FakeWatches()
        let observation = try WorkWatches.observe(launch: watches.launch)
        var frames = observation.frames.makeAsyncIterator()
        observation.request(.checkpoint(id: 5, replay: []))
        try await watches.answer(["desktop", "plan"], 5)
        #expect(try await frames.next()?.answers == 5)
        #expect(watches.requests(["monitor", "ps"]) == [#"{"id":5}"#])
        // The live-process watch has not answered, so nothing is confirmed yet.
        try await watches.answer(["monitor", "ps"], 5)
        let answered = try await frames.next()
        #expect(answered?.content.part == "activity")
        let checkpoint = try await frames.next()
        #expect(checkpoint?.content.part == "checkpoint")
        #expect(checkpoint?.answers == 5)
        await observation.cancel()
    }

    @Test("A stalled or noncooperative watch is reaped without touching other processes")
    func cancellation() async throws {
        let survivor = Process()
        survivor.executableURL = URL(fileURLWithPath: "/bin/sleep")
        survivor.arguments = ["30"]
        try survivor.run()
        defer { if survivor.isRunning { survivor.terminate() } }
        // exec preserves the ignored TERM disposition; KILL must be the fallback.
        let launched = Launched { _ in "trap '' TERM; exec /bin/sleep 30" }
        let reader = try launched.observe()
        try await Task.sleep(for: .milliseconds(100))
        let start = ContinuousClock.now
        await reader.cancel()
        #expect(launched.processes.allSatisfy { !$0.isRunning && $0.terminationStatus == SIGKILL })
        #expect(ContinuousClock.now - start < .seconds(6))
        #expect(survivor.isRunning)
    }

    @Test("Malformed and cut-off frames fail rather than becoming empty",
          arguments: ["malformed", "wrong-type", "partial-exit"])
    func invalidFrames(kind: String) async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let payload: String
        switch kind {
        case "malformed": payload = "{bad}\n"
        case "wrong-type": payload = "{\"answers\":\"not a number\"}\n"
        default: payload = "{\"answers\":"
        }
        try Data(payload.utf8).write(to: directory.appendingPathComponent("frame"))
        // Only the cut-off watch exits; the others stay open after their bad frame.
        let launched = Launched(cwd: directory) { args in
            guard args.starts(with: ["desktop", "plan"]) else { return "IFS= read -r request" }
            return kind == "partial-exit" ? "/bin/cat frame" : "/bin/cat frame; IFS= read -r request"
        }
        let reader = try launched.observe()
        do {
            for try await _ in reader.frames {
                Issue.record("Invalid transport supplied a frame")
            }
            Issue.record("Invalid transport finished successfully")
        } catch {
            #expect(error is RegistryQueryError)
            if kind == "malformed" { #expect(error.localizedDescription.contains("Invalid planning watch frame")) }
            if kind == "wrong-type" { #expect(error.localizedDescription.contains("answers")) }
        }
        await reader.cancel()
        #expect(launched.processes.allSatisfy { !$0.isRunning })
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

    @Test("Configuration invalidation retires every watch")
    func configurationChange() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let marker = directory.appendingPathComponent("changed")
        try Data(frame(answers: nil).utf8).write(to: directory.appendingPathComponent("frame"))
        let launched = Launched(cwd: directory, configurationChanged: {
            FileManager.default.fileExists(atPath: marker.path)
        }) { args in
            args.starts(with: ["desktop", "plan"]) ? "/bin/cat frame; IFS= read -r request" : "IFS= read -r request"
        }
        let reader = try launched.observe()
        var iterator = reader.frames.makeAsyncIterator()
        #expect(try await iterator.next()?.unavailable == "plan unavailable")
        try Data().write(to: marker)
        do {
            _ = try await iterator.next()
            Issue.record("Configuration change was ignored")
        } catch { #expect(error is WorkObservationError) }
        await reader.cancel()
        #expect(launched.processes.allSatisfy { !$0.isRunning })
    }

    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }

    /// One failed `lf wave show --all --watch` line.
    private func frame(answers: Int?) -> String {
        #"{"answers":\#(answers.map(String.init) ?? "null"),"unavailable":"plan unavailable","body":null}"# + "\n"
    }

    private func eventually(_ condition: () -> Bool) async throws {
        for _ in 0..<400 where !condition() { try await Task.sleep(for: .milliseconds(5)) }
        try #require(condition())
    }
}

/// Watches with no process: a test answers each request with a failed reading.
private final class FakeWatches: @unchecked Sendable {
    private let lock = NSLock()
    private var watches: [(args: [String], lines: AsyncThrowingStream<Data, any Error>.Continuation, sent: [String])] = []

    var launch: WorkWatches.Launch {
        { [self] args, _ in
            let (lines, continuation) = AsyncThrowingStream<Data, any Error>.makeStream()
            let index = lock.withLock { watches.append((args, continuation, [])); return watches.count - 1 }
            return LineWatch(lines: lines, send: { [self] line in
                lock.withLock { watches[index].sent.append(String(decoding: line, as: UTF8.self)
                    .trimmingCharacters(in: .newlines)) }
            }, cancel: { continuation.finish() })
        }
    }

    func requests(_ prefix: [String]) -> [String] {
        lock.withLock { watches.first { $0.args.starts(with: prefix) }?.sent ?? [] }
    }

    func answer(_ prefix: [String], _ id: Int) async throws {
        for _ in 0..<400 where !requests(prefix).contains(#"{"id":\#(id)}"#) {
            try await Task.sleep(for: .milliseconds(5))
        }
        let line = #"{"answers":\#(id),"unavailable":"gone","body":null}"#
        lock.withLock { watches.first { $0.args.starts(with: prefix) }?.lines.yield(Data(line.utf8)) }
    }
}

/// Stand-in `lf`: each watch runs the script chosen for its arguments.
private final class Launched: @unchecked Sendable {
    private let lock = NSLock()
    private var launched: [(args: [String], process: Process)] = []
    private let cwd: URL?
    private let configurationChanged: @Sendable () throws -> Bool
    private let script: @Sendable ([String]) -> String

    init(cwd: URL? = nil, configurationChanged: @escaping @Sendable () throws -> Bool = { false },
         script: @escaping @Sendable ([String]) -> String) {
        self.cwd = cwd
        self.configurationChanged = configurationChanged
        self.script = script
    }

    var processes: [Process] { lock.withLock { launched.map(\.process) } }
    var arguments: [[String]] { lock.withLock { launched.map(\.args) } }

    func observe() throws -> WorkObservation {
        try LocalWorkObservation.start(process: { [self] args, _ in
            let process = shell(script(args), cwd: cwd)
            lock.withLock { launched.append((args, process)) }
            return process
        }, configurationChanged: configurationChanged)
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
