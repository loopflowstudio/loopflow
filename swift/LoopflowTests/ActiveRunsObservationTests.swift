#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Active Run transport", .serialized)
struct ActiveRunsObservationTests {
    @Test("Reader startup failure preserves the CLI diagnostic", arguments: [false, true])
    func startupFailure(closesOutputFirst: Bool) async throws {
        let diagnosis = "Selected Home has an incompatible migration frontier"
        let shutdown = closesOutputFirst ? "exec 1>&-; IFS= read -r request; exit 23" : "exit 23"
        let process = shell("printf '%s\\n' '\(diagnosis)' >&2; \(shutdown)")
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        do {
            for try await _ in reader.snapshots {
                Issue.record("Failed reader supplied an observation")
            }
            Issue.record("Failed reader finished successfully")
        } catch {
            #expect(error.localizedDescription.contains(diagnosis))
        }
        await reader.cancel()
        #expect(!process.isRunning)
        #expect(process.terminationStatus == 23)
    }

    @Test("A slow consumer receives the latest complete snapshot without an output backlog")
    func latestDelivery() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let payload = (1...200).map { frame(time: $0) }.joined()
        try Data(payload.utf8).write(to: directory.appendingPathComponent("frames"))
        let process = shell("/bin/cat frames; /usr/bin/touch delivered; IFS= read -r request", cwd: directory)
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        let deadline = ContinuousClock.now + .seconds(3)
        while !FileManager.default.fileExists(atPath: directory.appendingPathComponent("delivered").path), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(FileManager.default.fileExists(atPath: directory.appendingPathComponent("delivered").path))
        // No consumer runs while the complete burst is decoded off the main actor.
        try await Task.sleep(for: .milliseconds(100))
        var frames = reader.snapshots.makeAsyncIterator()
        #expect(try await frames.next()?.observedAt == 200)
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Fragmented frames, stderr backpressure, refresh and rescan use one reader")
    func pipesAndRequests() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let first = frame(time: 1)
        try Data(first.utf8).write(to: directory.appendingPathComponent("first"))
        try Data(frame(time: 2).utf8).write(to: directory.appendingPathComponent("next"))
        let process = shell("""
        /usr/bin/head -c 131072 /dev/zero >&2
        /usr/bin/head -c 25 first
        /bin/sleep 0.05
        /usr/bin/tail -c +26 first
        while IFS= read -r request; do
            case "$request" in
                *rescan*) /bin/cat first ;;
                *refresh*) /bin/cat next ;;
            esac
        done
        """, cwd: directory)
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        var frames = reader.snapshots.makeAsyncIterator()
        #expect(try await frames.next()?.observedAt == 1)
        await reader.request(.refresh)
        #expect(try await frames.next()?.observedAt == 2)
        await reader.request(.rescan)
        #expect(try await frames.next()?.observedAt == 1)
        await reader.cancel()
        #expect(!process.isRunning)
        #expect(process.terminationStatus == 0)
    }

    @Test("A stalled or noncooperative reader is reaped without touching other clients")
    func cancellation() async throws {
        let survivor = Process()
        survivor.executableURL = URL(fileURLWithPath: "/bin/sleep")
        survivor.arguments = ["30"]
        try survivor.run()
        defer { survivor.terminate(); survivor.waitUntilExit() }
        // exec preserves the ignored TERM disposition; KILL must be the fallback.
        let process = shell("trap '' TERM; exec /bin/sleep 30")
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        try await Task.sleep(for: .milliseconds(100))
        let start = ContinuousClock.now
        await reader.cancel()
        #expect(!process.isRunning)
        #expect(process.terminationStatus == SIGKILL)
        #expect(ContinuousClock.now - start < .seconds(4))
        #expect(survivor.isRunning)
    }

    @Test("Silence expires the reading and stops the reader")
    func stalled() async throws {
        let process = shell("exec /bin/sleep 30")
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        var iterator = reader.snapshots.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            Issue.record("A stalled pipe was accepted")
        } catch {
            #expect(error.localizedDescription.contains("ten seconds"))
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Malformed, oversized and wrong-Home observations fail rather than becoming empty",
          arguments: ["malformed", "oversized", "wrong-home", "home-change", "partial-exit"])
    func invalidFrames(kind: String) async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let payload: String
        switch kind {
        case "malformed": payload = "{bad}\n"
        case "oversized": payload = String(repeating: "x", count: 16 * 1024 * 1024 + 1)
        case "wrong-home": payload = frame(home: "/other")
        case "home-change": payload = frame() + frame(home: "/other")
        default: payload = "{\"home\":"
        }
        try Data(payload.utf8).write(to: directory.appendingPathComponent("frame"))
        let process = shell("/bin/cat frame; IFS= read -r request", cwd: directory)
        if kind == "partial-exit" { process.arguments = ["-c", "/bin/cat frame"] }
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        do {
            for try await snapshot in reader.snapshots {
                #expect(kind == "home-change")
                #expect(snapshot.home == "/fixture")
            }
            Issue.record("Invalid transport finished successfully")
        } catch {
            #expect(error is RegistryQueryError)
            if kind == "oversized" { #expect(error.localizedDescription.contains("16 MiB")) }
            if kind == "wrong-home" || kind == "home-change" { #expect(error.localizedDescription.contains("changed Home")) }
            if kind == "malformed" { #expect(error.localizedDescription.contains("Invalid active Run observation")) }
            if kind == "partial-exit" { #expect(!error.localizedDescription.contains("ten seconds")) }
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Configuration invalidation retires the exact reader")
    func configurationChange() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let marker = directory.appendingPathComponent("changed")
        try Data(frame().utf8).write(to: directory.appendingPathComponent("frame"))
        let process = shell("/bin/cat frame; IFS= read -r request", cwd: directory)
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: {
            FileManager.default.fileExists(atPath: marker.path)
        })
        var iterator = reader.snapshots.makeAsyncIterator()
        #expect(try await iterator.next()?.home == "/fixture")
        try Data().write(to: marker)
        do {
            _ = try await iterator.next()
            Issue.record("Configuration change was ignored")
        } catch { #expect(error is ActiveRunsObservationError) }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("The real foreground CLI delivers automatic observations and rescan through Swift")
    func realCLI() async throws {
        let home = try directory()
        defer { try? FileManager.default.removeItem(at: home) }
        let client = Process()
        client.executableURL = URL(fileURLWithPath: "/bin/cat")
        let clientInput = Pipe()
        client.standardInput = clientInput
        client.standardOutput = FileHandle.nullDevice
        try client.run()
        defer {
            try? clientInput.fileHandleForWriting.close()
            client.waitUntilExit()
        }
        let firstID = "run_00000000000000000000000000000001"
        let secondID = "run_00000000000000000000000000000002"
        try publishClient(client, id: firstID, home: home)
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let process = LocalWaveAgentLauncher.queryProcess([
            root.appendingPathComponent("target/debug/lf").path, "runs", "--active", "--watch", "--json",
        ])
        var environment = process.environment ?? [:]
        for key in ["LF_HOME", "LF_CONTROL_HOME"] { environment[key] = home.path }
        for key in ["LF_DB_PATH", "LF_CONTROL_DB_PATH"] { environment[key] = home.appendingPathComponent("loopflow.db").path }
        process.environment = environment
        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
        var iterator = reader.snapshots.makeAsyncIterator()
        let first = try await nextReady(&iterator, count: 1)
        #expect(URL(fileURLWithPath: first.home).resolvingSymlinksInPath().standardizedFileURL.path
                == home.standardizedFileURL.path)
        #expect(first.gaps.isEmpty)
        #expect(first.runs.map(\.id) == [firstID])
        let next = try await nextReady(&iterator, count: 1)
        #expect(next.observedAt > first.observedAt)
        let secondClient = Process()
        secondClient.executableURL = URL(fileURLWithPath: "/bin/cat")
        let secondInput = Pipe()
        secondClient.standardInput = secondInput
        secondClient.standardOutput = FileHandle.nullDevice
        try secondClient.run()
        defer {
            try? secondInput.fileHandleForWriting.close()
            secondClient.waitUntilExit()
        }
        try publishClient(secondClient, id: secondID, home: home)
        let published = try await nextReady(&iterator, count: 2)
        #expect(Set(published.runs.map(\.id)) == [firstID, secondID])
        await reader.request(.rescan)
        let recovered = try await nextReady(&iterator, count: 2)
        #expect(recovered.gaps.isEmpty)
        #expect(recovered.home == first.home)
        try secondInput.fileHandleForWriting.close()
        secondClient.waitUntilExit()
        let afterExit = try await nextReady(&iterator, count: 1)
        #expect(afterExit.runs.map(\.id) == [firstID])
        await reader.cancel()
        #expect(!process.isRunning)
        #expect(process.terminationStatus == 0)
        #expect(client.isRunning)
    }

    private func nextReady(_ iterator: inout AsyncThrowingStream<ActiveRunsSnapshot, any Error>.Iterator, count: Int) async throws -> ActiveRunsSnapshot {
        let deadline = ContinuousClock.now + .seconds(12)
        while let value = try await iterator.next() {
            try #require(ContinuousClock.now < deadline)
            try #require(value.discovery != .unavailable)
            if value.discovery == .ready && value.runs.count == count { return value }
        }
        throw RegistryQueryError("Reader ended before a complete observation")
    }

    private func publishClient(_ client: Process, id: String, home: URL) throws {
        let directory = home.appendingPathComponent("runs/00/\(id)")
        try FileManager.default.createDirectory(at: directory.appendingPathComponent("provider-clients"), withIntermediateDirectories: true)
        let manifest: [String: Any] = [
            "schema_version": 1, "run_id": id, "parent_run_id": NSNull(),
            "created_at": "2020-01-01T00:00:00Z", "harness": "cat", "model": NSNull(),
            "surface": "tui", "cwd": home.path, "repo": NSNull(), "worktree": NSNull(),
            "skill": NSNull(), "subjects": [], "launch": NSNull(), "context": NSNull(),
            "runtime_path": NSNull(), "runtime_digest": NSNull(), "host": "fixture", "boot_id": NSNull(),
        ]
        try JSONSerialization.data(withJSONObject: manifest).write(to: directory.appendingPathComponent("manifest.json"), options: .atomic)
        let receipt: [String: Any] = ["schema_version": 1, "pid": client.processIdentifier,
                                      "terminal_id": NSNull(), "started_at": Date().ISO8601Format(.init(includingFractionalSeconds: true))]
        try JSONSerialization.data(withJSONObject: receipt).write(
            to: directory.appendingPathComponent("provider-clients/\(client.processIdentifier).json"), options: .atomic)
    }

    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }

    private func frame(time: Int = 1, home: String = "/fixture") -> String {
        "{\"discovery\":\"ready\",\"home\":\"\(home)\",\"observed_at\":\(time),\"task\":null,\"runs\":[],\"gaps\":[]}\n"
    }

    private func shell(_ script: String, cwd: URL? = nil) -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", script]
        process.currentDirectoryURL = cwd
        process.environment = ["PATH": "/usr/bin:/bin", "LF_CONTROL_HOME": "/fixture"]
        return process
    }
}
#endif
