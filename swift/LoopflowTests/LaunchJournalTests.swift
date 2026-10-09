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
        journal.read(["wave", "show", "--all", "--json"], ms: 14000, ok: false)
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
        #expect(reads.map { $0["verb"] as? String } == ["task comments", "wave show", nil, nil])
        #expect(reads.map { $0["ok"] as? Bool } == [true, false, false, false])
        #expect(!String(decoding: try Data(contentsOf: directory.appendingPathComponent("reads.ndjson")), as: UTF8.self).contains("LOO-1"))
    }

    @Test("Nothing is written before a launch begins")
    func silentUntilBegun() throws {
        let directory = try temporaryDirectory()
        let journal = LaunchJournal(directory: directory)
        journal.mark(.usable, ms: 1)
        journal.read(["wave", "show"], ms: 1, ok: true)
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
