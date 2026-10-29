#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Desktop log")
struct DesktopLogTests {
    @Test("Lines join lf.log in lf's shape, ending with the process and build that wrote them")
    func lfShapedLines() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("desktop-log-\(UUID().uuidString)/logs/lf.log")
        let log = DesktopLog(url: url, app: "9.9.9", revision: "b11130060")
        log.warn("dropped before begin")
        log.begin()
        log.warn("Desktop API unavailable", ["path": "/tmp/x.sock", "error": "Address already in use"])
        log.info("vitals", ["footprint_mb": "412"])
        log.flush()

        let lines = String(decoding: try Data(contentsOf: url), as: UTF8.self).split(separator: "\n").map(String.init)
        #expect(lines.count == 2)
        let suffix = " pid=\(getpid()) desktop=9.9.9 rev=b11130060"
        #expect(lines.allSatisfy { $0.hasSuffix(suffix) })
        #expect(lines[0].contains("Z  WARN loopflow::desktop: Desktop API unavailable path=/tmp/x.sock error=\"Address already in use\" pid="))
        #expect(lines[1].contains("Z  INFO loopflow::desktop: vitals footprint_mb=412 pid="))
        #expect(ISO8601DateFormatter().date(from: String(lines[0].prefix(while: { $0 != " " }))
            .replacingOccurrences(of: #"\.\d+Z$"#, with: "Z", options: .regularExpression)) != nil)
    }

    @Test("The heartbeat counts reads and kills, and warns at a storm of cancelled reads")
    func heartbeat() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("desktop-log-\(UUID().uuidString)/logs/lf.log")
        let log = DesktopLog(url: url, app: "9.9.9", revision: "b11130060")
        log.begin()
        let journal = LaunchJournal(directory: url.deletingLastPathComponent())
        journal.begin(app: "9.9.9", preMainMs: 1)
        journal.read(["desktop", "plan", "--all"], ms: 5, ok: true)
        journal.read(["task", "diff", "task_87e0"], ms: 5, ok: false)
        journal.killed(["task", "diff", "task_87e0"])
        let sample = DesktopVitals.Sample(footprintMB: 412, peakMB: 600, fds: 61, threads: 40)
        DesktopVitals.beat(sample, journal.takeTally(), log: log)
        #expect(journal.takeTally() == LaunchJournal.Tally())

        var storm = LaunchJournal.Tally()
        storm.reads = ["task diff": 1000, "desktop plan": 3, "session list": 2, "monitor ps": 1]
        storm.killed = ["task diff": DesktopVitals.stormKills + 1]
        DesktopVitals.beat(sample, storm, log: log)
        log.flush()

        let lines = String(decoding: try Data(contentsOf: url), as: UTF8.self).split(separator: "\n").map(String.init)
        #expect(lines.count == 2)
        #expect(lines[0].contains(
            " INFO loopflow::desktop: vitals footprint_mb=412 peak_mb=600 fds=61 threads=40 reads=2 failed=1 killed=1 top=\"desktop plan×1,task diff×1\" pid="))
        #expect(!lines[0].contains("task_87e0"))
        #expect(lines[1].contains(" WARN loopflow::desktop: lf reads cancelled at storm rate "))
        #expect(lines[1].contains("killed=101 top=\"task diff×1000,desktop plan×3,session list×2\""))
    }

    @Test("A sample of this process reads real vitals")
    func sample() {
        let sample = DesktopVitals.sample()
        #expect(sample.footprintMB > 0)
        #expect(sample.peakMB >= sample.footprintMB)
        #expect(sample.fds > 2)
        #expect(sample.threads > 0)
    }
}
#endif
