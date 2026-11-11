import Foundation
import Loopflow
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

    private static let signposter = PerfClock.signposter
    private static let log = Logger(subsystem: "studio.loopflow", category: "perf")
    private static var launchMarks = Set<String>()

    static func begin(_ name: StaticString) -> Wait {
        Wait(name: name, start: .now, state: signposter.beginInterval(name))
    }

    static func end(_ wait: Wait, _ detail: String = "") {
        signposter.endInterval(wait.name, wait.state)
        let ms = wait.start.elapsedMs
        log.notice("wait \(wait.name.description, privacy: .public) ms=\(ms, format: .fixed(precision: 1)) \(detail, privacy: .public)")
    }

    /// Time since the kernel started this process, recorded once per mark.
    static func launch(_ mark: String) {
        guard launchMarks.insert(mark).inserted, let ms = PerfClock.millisecondsSinceProcessStart() else { return }
        signposter.emitEvent("launch", "\(mark, privacy: .public)")
        log.notice("launch \(mark, privacy: .public) ms=\(ms, format: .fixed(precision: 1))")
    }
}
