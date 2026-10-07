#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// The pipes under `lf monitor work --watch --json`, driven by stand-in readers.
@Suite("Workspace reader transport", .serialized)
struct WorkObservationTransportTests {
    @Test("Reader startup failure preserves the CLI diagnostic", arguments: [false, true])
    func startupFailure(closesOutputFirst: Bool) async throws {
        let diagnosis = "Selected Home has an incompatible migration frontier"
        let shutdown = closesOutputFirst ? "exec 1>&-; IFS= read -r request; exit 23" : "exit 23"
        let process = shell("printf '%s\\n' '\(diagnosis)' >&2; \(shutdown)")
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: { false })
        do {
            for try await _ in reader.frames {
                Issue.record("Failed reader supplied a frame")
            }
            Issue.record("Failed reader finished successfully")
        } catch {
            #expect(error.localizedDescription.contains(diagnosis))
        }
        await reader.cancel()
        #expect(!process.isRunning)
        #expect(process.terminationStatus == 23)
    }

    @Test("A consumer that starts late receives every frame in order")
    func everyFrame() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        try Data((1...200).map { frame($0) }.joined().utf8).write(to: directory.appendingPathComponent("frames"))
        let process = shell("/bin/cat frames; IFS= read -r request", cwd: directory)
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: { false })
        var frames = reader.frames.makeAsyncIterator()
        for expected in 1...200 {
            #expect(try await frames.next()?.sequence == expected)
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Fragmented frames, stderr backpressure and requests use one reader")
    func pipesAndRequests() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        try Data(frame(1).utf8).write(to: directory.appendingPathComponent("first"))
        try Data(frame(2).utf8).write(to: directory.appendingPathComponent("refreshed"))
        try Data(frame(3).utf8).write(to: directory.appendingPathComponent("scoped"))
        let process = shell("""
        /usr/bin/head -c 131072 /dev/zero >&2
        /usr/bin/head -c 25 first
        /bin/sleep 0.05
        /usr/bin/tail -c +26 first
        while IFS= read -r request; do
            case "$request" in
                *refresh*) /bin/cat refreshed ;;
                *scope*) /bin/cat scoped ;;
            esac
        done
        """, cwd: directory)
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: { false })
        var frames = reader.frames.makeAsyncIterator()
        #expect(try await frames.next()?.sequence == 1)
        reader.request(.refresh(id: 1))
        reader.request(.scope(id: 2, WorkScope(repo: "/src/loopflow", headless: false, task: "LOO-1",
                                                    wave: nil, activity: nil)))
        #expect(try await frames.next()?.sequence == 2)
        #expect(try await frames.next()?.sequence == 3)
        await reader.cancel()
        #expect(!process.isRunning)
        #expect(process.terminationStatus == 0)
    }

    @Test("A stalled or noncooperative reader is reaped without touching other processes")
    func cancellation() async throws {
        let survivor = Foundation.Process()
        survivor.executableURL = URL(fileURLWithPath: "/bin/sleep")
        survivor.arguments = ["30"]
        try survivor.run()
        defer { if survivor.isRunning { survivor.terminate() } }
        // exec preserves the ignored TERM disposition; KILL must be the fallback.
        let process = shell("trap '' TERM; exec /bin/sleep 30")
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: { false })
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
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: { false })
        var iterator = reader.frames.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            Issue.record("A stalled pipe was accepted")
        } catch {
            #expect(error.localizedDescription.contains("ten seconds"))
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Malformed, oversized and cut-off frames fail rather than becoming empty",
          arguments: ["malformed", "oversized", "partial-exit"])
    func invalidFrames(kind: String) async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let payload: String
        switch kind {
        case "malformed": payload = "{bad}\n"
        case "oversized": payload = String(repeating: "x", count: 64 * 1024 * 1024 + 1)
        default: payload = "{\"part\":"
        }
        try Data(payload.utf8).write(to: directory.appendingPathComponent("frame"))
        // Only the cut-off reader exits; the others stay open after their bad frame.
        let process = shell(kind == "partial-exit" ? "/bin/cat frame" : "/bin/cat frame; IFS= read -r request",
                            cwd: directory)
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: { false })
        do {
            for try await _ in reader.frames {
                Issue.record("Invalid transport supplied a frame")
            }
            Issue.record("Invalid transport finished successfully")
        } catch {
            #expect(error is RegistryQueryError)
            if kind == "oversized" { #expect(error.localizedDescription.contains("64 MiB")) }
            if kind == "malformed" { #expect(error.localizedDescription.contains("Invalid workspace observation")) }
            if kind == "partial-exit" { #expect(!error.localizedDescription.contains("ten seconds")) }
        }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    @Test("Configuration invalidation retires the reader")
    func configurationChange() async throws {
        let directory = try directory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let marker = directory.appendingPathComponent("changed")
        try Data(frame(1).utf8).write(to: directory.appendingPathComponent("frame"))
        let process = shell("/bin/cat frame; IFS= read -r request", cwd: directory)
        let reader = try LocalWorkObservation.start(process: process, configurationChanged: {
            FileManager.default.fileExists(atPath: marker.path)
        })
        var iterator = reader.frames.makeAsyncIterator()
        #expect(try await iterator.next()?.home == "/fixture")
        try Data().write(to: marker)
        do {
            _ = try await iterator.next()
            Issue.record("Configuration change was ignored")
        } catch { #expect(error is WorkObservationError) }
        await reader.cancel()
        #expect(!process.isRunning)
    }

    private func directory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }

    /// One line shaped like the `task` entry of `work_frame.json`.
    private func frame(_ sequence: Int) -> String {
        #"{"part":"task","sequence":\#(sequence),"answers":null,"home":"/fixture","# +
            #""revisions":{"planning":911,"sessions":403,"flows":88,"processes":5120,"usage":77},"# +
            #""unavailable":"Task LOO-1 is not registered","body":null}"# + "\n"
    }

    private func shell(_ script: String, cwd: URL? = nil) -> Foundation.Process {
        let process = Foundation.Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", script]
        process.currentDirectoryURL = cwd
        process.environment = ["PATH": "/usr/bin:/bin"]
        return process
    }
}
#endif
