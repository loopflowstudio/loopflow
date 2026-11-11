import Darwin
import Foundation
import os

/// How long a person waited for something, as a signpost interval and one log
/// line (`studio.loopflow`, category `perf`) that `log show` and Instruments read.
/// Every wait ends where its result is handed to the screen; the frame that
/// shows it follows within one display refresh, which these do not measure.
@MainActor
enum Perf {
    struct Wait {
        fileprivate let name: StaticString
        fileprivate let start: ContinuousClock.Instant
        fileprivate let state: OSSignpostIntervalState
    }

    private static let signposter = OSSignposter(subsystem: "studio.loopflow", category: "perf")
    private static let log = Logger(subsystem: "studio.loopflow", category: "perf")
    private static var launchMarks = Set<String>()

    static func begin(_ name: StaticString) -> Wait {
        Wait(name: name, start: .now, state: signposter.beginInterval(name))
    }

    static func end(_ wait: Wait, _ detail: String = "") {
        signposter.endInterval(wait.name, wait.state)
        let elapsed = ContinuousClock.now - wait.start
        let ms = Double(elapsed.components.seconds) * 1000 + Double(elapsed.components.attoseconds) / 1e15
        log.notice("wait \(wait.name.description, privacy: .public) ms=\(ms, format: .fixed(precision: 1)) \(detail, privacy: .public)")
    }

    /// Time since the kernel started this process, recorded once per mark.
    static func launch(_ mark: String) {
        guard launchMarks.insert(mark).inserted, let ms = sinceProcessStart() else { return }
        signposter.emitEvent("launch", "\(mark, privacy: .public)")
        log.notice("launch \(mark, privacy: .public) ms=\(ms, format: .fixed(precision: 1))")
    }

    private static func sinceProcessStart() -> Double? {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var name = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&name, UInt32(name.count), &info, &size, nil, 0) == 0 else { return nil }
        let started = info.kp_proc.p_starttime
        var now = timeval()
        gettimeofday(&now, nil)
        return Double(now.tv_sec - started.tv_sec) * 1000 + Double(now.tv_usec - started.tv_usec) / 1000
    }
}
