import Foundation
import Loopflow
import os

/// Shared daily interaction journal. Mobile state observations stay unmeasured
/// until event timestamps and presented-frame observations are connected.
@MainActor
enum Perf {
    private static let signposter = PerfClock.signposter
    private static let log = Logger(subsystem: "studio.loopflow", category: "perf")
    private static var launchMarks = Set<String>()
    private static let journal: InteractionJournal = {
        var identity = ["platform": "iOS", "launch": UUID().uuidString]
        identity["app"] = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        identity["revision"] = Bundle.main.object(forInfoDictionaryKey: "LoopflowSourceRevision") as? String
        return InteractionJournal(directory: URL.applicationSupportDirectory.appending(path: "performance/interactions"),
                                  identity: identity)
    }()

    static func begin(_ name: StaticString, scenario: String) -> Interaction {
        Interaction(name: "\(name)", scenario: scenario, input: InteractionTrace.input,
            context: Interaction.Context(thermalState: ProcessInfo.processInfo.thermalState.rawValue,
                                         liveSessions: nil), sink: journal.append)
    }

    static func end(_ interaction: Interaction) {
        interaction.usable(at: ProcessInfo.processInfo.systemUptime, evidence: .appState)
        interaction.finish(.unmeasured)
    }

    /// Time since the kernel started this process, recorded once per mark.
    static func launch(_ mark: String) {
        guard launchMarks.insert(mark).inserted, let ms = PerfClock.millisecondsSinceProcessStart() else { return }
        signposter.emitEvent("launch", "\(mark, privacy: .public)")
        log.notice("launch \(mark, privacy: .public) ms=\(ms, format: .fixed(precision: 1))")
    }
}
