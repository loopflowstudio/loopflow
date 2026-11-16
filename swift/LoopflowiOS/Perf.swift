import Foundation
import Loopflow
import os
import UIKit

/// Shared daily interaction journal, with original UIKit events and native
/// visual-observation upper bounds. Missing capture never becomes a success.
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

    private static var openings: [String: Interaction] = [:]

    static func opening(_ task: String) {
        openings.removeValue(forKey: task)?.finish(.superseded)
        let interaction = begin("return_session", scenario: "remote_attach")
        openings[task] = interaction
        acknowledged(interaction)
    }

    static func takeOpening(_ task: String) -> Interaction? { openings.removeValue(forKey: task) }

    static func begin(_ name: StaticString, scenario: String) -> Interaction {
        Interaction(name: "\(name)", scenario: scenario, input: InteractionTrace.input,
            context: Interaction.Context(thermalState: ProcessInfo.processInfo.thermalState.rawValue,
                                         liveSessions: nil, repository: UserDefaults.standard.string(forKey: "repository").flatMap { $0.isEmpty ? nil : $0 } ?? "all-repositories"), sink: journal.append)
    }

    private static var window: UIWindow? {
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .filter { $0.activationState == .foregroundActive }
            .flatMap(\.windows).first { $0.isKeyWindow }
    }

    static func acknowledged(_ interaction: Interaction) {
        let window = window
        interaction.acknowledged(at: ProcessInfo.processInfo.systemUptime, evidence: .appState)
        interaction.observe(.acknowledged) { [weak window] in await InteractionFrame.capture(window: window) }
    }

    static func end(_ interaction: Interaction, prepareFrame: @escaping @MainActor () -> Void = {}) {
        interaction.usable(at: ProcessInfo.processInfo.systemUptime, evidence: .appState)
        let window = window
        interaction.observe(.usable) { [weak window] in await InteractionFrame.capture(window: window, prepare: prepareFrame) }
    }

    /// Time since the kernel started this process, recorded once per mark.
    static func launch(_ mark: String) {
        guard launchMarks.insert(mark).inserted, let ms = PerfClock.millisecondsSinceProcessStart() else { return }
        signposter.emitEvent("launch", "\(mark, privacy: .public)")
        log.notice("launch \(mark, privacy: .public) ms=\(ms, format: .fixed(precision: 1))")
    }
}
