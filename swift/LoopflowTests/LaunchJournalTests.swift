#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Launch journal")
struct LaunchJournalTests {
    @Test("A launch records each milestone once, with the app version and no arguments")
    func launchRecord() throws {
        let directory = try temporaryDirectory()
        let journal = LaunchJournal(directory: directory)
        journal.begin(app: "9.9.9", preMainMs: 41.26)
        journal.mark(.restored, ["cache": "hit"], ms: 60)
        journal.mark(.usable, ["source": "saved"], ms: 300)
        journal.mark(.usable, ["source": "fresh"], ms: 9000)
        journal.read(["task", "comments", "--id", "LOO-1", "--json"], ms: 12.34, ok: true)
        journal.read(["desktop", "plan", "--all", "--json"], ms: 14000, ok: false)
        journal.refreshed("planning", ms: 14001, ok: false)
        journal.mark(.fresh, ms: 15000)
        journal.refreshed("sessions", ms: 20, ok: false)
        journal.flush()

        let launches = try lines(directory, "launches")
        #expect(launches.map { $0["event"] as? String } == ["launch", "restored", "usable", "refresh_failed", "fresh"])
        #expect(Set(launches.map { $0["launch"] as? String }).count == 1)
        #expect(launches.allSatisfy { $0["app"] as? String == "9.9.9" })
        #expect(launches[0]["ms"] as? Double == 41.3)
        #expect(launches[2]["source"] as? String == "saved")
        #expect(launches[3]["part"] as? String == "planning")

        let reads = try lines(directory, "reads")
        #expect(reads.map { $0["verb"] as? String } == ["task comments", "desktop plan", nil, nil])
        #expect(reads.map { $0["ok"] as? Bool } == [true, false, false, false])
        #expect(!String(decoding: try Data(contentsOf: directory.appendingPathComponent("reads.ndjson")), as: UTF8.self).contains("LOO-1"))
    }

    @Test("A launch names its process, bundle, revision and macOS")
    func launchIdentity() throws {
        let directory = try temporaryDirectory()
        let journal = LaunchJournal(directory: directory)
        journal.begin(app: "9.9.9", preMainMs: 1, bundle: "com.loopflow.mac.dev", revision: "b11130060")
        journal.flush()
        let launch = try #require(try lines(directory, "launches").first)
        #expect(launch["pid"] as? Int == Int(getpid()))
        #expect(launch["bundle"] as? String == "com.loopflow.mac.dev")
        #expect(launch["rev"] as? String == "b11130060")
        #expect((launch["os"] as? String)?.split(separator: ".").count == 3)
    }

    @Test("A launch records one exit, whichever path reports first")
    func oneExit() throws {
        let directory = try temporaryDirectory()
        let journal = LaunchJournal(directory: directory)
        journal.begin(app: "9.9.9", preMainMs: 1)
        journal.recordExit("quit", ["sender": "Dock", "sender_pid": "612"])
        journal.recordExit("exit", ["caller": "LoopflowMac main"])
        journal.recordExit("exception", ["name": "NSInternalInconsistencyException"])

        let exits = try lines(directory, "launches").filter { $0["event"] as? String == "exit" }
        #expect(exits.count == 1)
        #expect(exits[0]["reason"] as? String == "quit")
        #expect(exits[0]["sender"] as? String == "Dock")
        #expect(exits[0]["app"] as? String == "9.9.9")
        #expect(exits[0]["at"] is String)
    }

    @Test("exit() names the first frame that called it outside the system libraries")
    func exitCaller() {
        let stack = [
            "0   LoopflowMac                         0x0000000100a1b2c4 $s11LoopflowMac13LaunchJournalC12recordAtExit + 40",
            "1   libsystem_c.dylib                   0x000000018c5e1f28 __cxa_finalize_ranges + 476",
            "2   libsystem_c.dylib                   0x000000018c5e1cf0 exit + 44",
            "3   libsystem_c.dylib                   0x000000018c5f0a10 abort_report_np + 12",
            "4   LoopflowMac                         0x0000000101f00010 ghostty_app_tick + 1220",
            "5   AppKit                              0x000000018f0e1234 -[NSApplication run] + 480",
        ]
        #expect(LaunchJournal.exitCaller(stack) == "LoopflowMac 0x0000000101f00010 ghostty_app_tick + 1220")
        #expect(LaunchJournal.exitCaller(Array(stack.prefix(2))) == nil)
    }

    @Test("A quit Apple event names why it was sent")
    func quitSender() {
        let event = NSAppleEventDescriptor(
            eventClass: 0x61657674, eventID: 0x71756974,  // 'aevt' 'quit'
            targetDescriptor: nil, returnID: AEReturnID(kAutoGenerateReturnID), transactionID: AETransactionID(kAnyTransactionID))
        #expect(LaunchJournal.quitSender(nil).isEmpty)
        // The sender pid is a system attribute only a delivered event carries.
        event.setAttribute(NSAppleEventDescriptor(enumCode: 0x73687574), forKeyword: 0x7768793F)  // 'why?' 'shut'
        let sender = LaunchJournal.quitSender(event)
        #expect(sender["sender_pid"] == nil)
        #expect(sender["quit_reason"] == "shutdown")
    }

    @Test("A signal's preformatted line is NDJSON naming the signal and the time it is stamped with")
    func signalLine() throws {
        var line = ExitSignals.line(["event": "exit", "launch": "L", "app": "9.9.9", "reason": "signal"], signal: "SIGTERM")
        line.withUnsafeMutableBufferPointer {
            ExitSignals.stamp(1_793_284_045, into: $0.baseAddress! + ExitSignals.timestampOffset)
        }
        #expect(line.last == 0x0A)
        let record = try #require(try JSONSerialization.jsonObject(with: Data(line.dropLast())) as? [String: Any])
        #expect(record["signal"] as? String == "SIGTERM")
        #expect(record["reason"] as? String == "signal")
        #expect(record["launch"] as? String == "L")
        #expect(record["at"] as? String == ISO8601DateFormatter().string(from: Date(timeIntervalSince1970: 1_793_284_045)))

        for epoch in [0, 951_782_400, 4_107_542_399] {  // epoch, 2000-02-29, 2100-02-28T23:59:59
            var stamp = Array(ExitSignals.timestampPlaceholder.utf8)
            stamp.withUnsafeMutableBufferPointer { ExitSignals.stamp(epoch, into: $0.baseAddress!) }
            #expect(String(decoding: stamp, as: UTF8.self)
                == ISO8601DateFormatter().string(from: Date(timeIntervalSince1970: TimeInterval(epoch))))
        }
    }

    @Test("Nothing is written before a launch begins")
    func silentUntilBegun() throws {
        let directory = try temporaryDirectory()
        let journal = LaunchJournal(directory: directory)
        journal.mark(.usable, ms: 1)
        journal.read(["desktop", "plan"], ms: 1, ok: true)
        journal.flush()
        #expect(!FileManager.default.fileExists(atPath: directory.path))
    }

    @Test("A full file keeps its newest whole lines, across processes")
    func bounded() throws {
        let directory = try temporaryDirectory()
        let count = LaunchJournal.maxBytes / 40
        for process in 0..<2 {
            let journal = LaunchJournal(directory: directory)
            journal.begin(app: "1", preMainMs: 1)
            for index in 0..<count { journal.read(["session", "list"], ms: Double(process * count + index), ok: true) }
            journal.flush()
        }
        let url = directory.appendingPathComponent("reads.ndjson")
        let size = try #require(try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int)
        #expect(size <= LaunchJournal.maxBytes)
        let reads = try lines(directory, "reads")
        #expect(reads.count > 100)
        #expect(reads.last?["ms"] as? Double == Double(2 * count - 1))
    }

    private func lines(_ directory: URL, _ name: String) throws -> [[String: Any]] {
        let text = String(decoding: try Data(contentsOf: directory.appendingPathComponent("\(name).ndjson")), as: UTF8.self)
        return try text.split(separator: "\n").map {
            try #require(try JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any])
        }
    }

    private func temporaryDirectory() throws -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("launch-journal-\(UUID().uuidString)")
    }
}
#endif
