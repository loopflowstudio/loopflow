// Native Work reads, writes and subscriptions use the local server.
// Execution launches and remote file streams retain their CLI transports.

#if os(macOS)
import Foundation
import Loopflow

enum RegistryQueryLocal {
    static let shared = RegistryQuery(runWithInput: { args, cwd, input in
        try await LocalWaveAgentLauncher.queryLf(args, cwd: cwd, input: input)
    }, serve: { query in
        try await ServerClient.read(query)
    }, write: { change in
        switch change {
        case .taskCreate(_, let requestID, _, _, _, _, _, _), .sessionNew(_, let requestID, _, _):
            return try await CreationSubmissions.shared.submit(change, requestID: requestID)
        default: return try await ServerClient.write(change)
        }
    }, watchFiles: { args, cwd in
        // Only a remote Task's files: `lf --machine … task watch-files` over SSH.
        try await Task.detached(priority: .userInitiated) {
            let configuration = try ReaderLaunchConfiguration.current()
            let process = LocalWaveAgentLauncher.queryProcess([configuration.helper] + args)
            process.environment = configuration.environment
            if let cwd { process.currentDirectoryURL = URL(fileURLWithPath: cwd) }
            let reader = try LocalLineObservation<TaskFilesFrame>.start(
                name: "Task files", process: process, frameLimit: 65536,
                configurationChanged: { try ReaderLaunchConfiguration.current() != configuration },
                decode: { try JSONDecoder().decode(TaskFilesFrame.self, from: $0) })
            return TaskFilesObservation(frames: reader.frames, cancel: reader.cancel)
        }.value
    }, subscribeRead: { args, cwd in
        try await Task.detached(priority: .userInitiated) {
            let configuration = try ReaderLaunchConfiguration.current()
            let process = LocalWaveAgentLauncher.queryProcess([configuration.helper] + args)
            process.environment = configuration.environment
            if let cwd { process.currentDirectoryURL = URL(fileURLWithPath: cwd) }
            let reader = try LocalLineObservation<Data>.start(
                name: "Task history", process: process, frameLimit: 32 * 1024 * 1024, silence: nil,
                configurationChanged: { try ReaderLaunchConfiguration.current() != configuration },
                decode: { $0 })
            return LineSubscription(lines: reader.frames, send: reader.send, cancel: reader.cancel)
        }.value
    }, subscribeWork: {
        try WorkSubscriptions.observe { ServerClient.subscribe($0) }
    }) { args, cwd in
        try await Perf.measure(Perf.lf, args.prefix(2).joined(separator: " ")) {
            let started = ContinuousClock.now
            var ok = false
            defer { LaunchJournal.home.read(args, ms: started.elapsedMs, ok: ok) }
            let output = try await LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
            ok = true
            return output
        }
    }
}

/// Invalidate on replacement; let lf resolve the selected Machine/store itself.
/// Metadata is sufficient here: these installation files are atomically replaced.
private struct ReaderLaunchConfiguration: Equatable {
    let helper: String
    let environment: [String: String]
    let files: [String]

    static func current() throws -> Self {
        let helper = try LocalWaveAgentLauncher.controlLfPath()
        let install = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".lf-machine/install")
        let paths = [URL(fileURLWithPath: helper),
                     install.appendingPathComponent("active.json"),
                     install.appendingPathComponent("switch.json")]
        let files = try paths.map { url -> String in
            let resolved = url.resolvingSymlinksInPath().path
            do {
                let attributes = try FileManager.default.attributesOfItem(atPath: resolved)
                return "\(resolved):\(String(describing: attributes[.systemFileNumber])):\(String(describing: attributes[.size])):\(String(describing: attributes[.modificationDate]))"
            } catch CocoaError.fileReadNoSuchFile {
                return "\(resolved):absent"
            }
        }
        return Self(helper: helper,
                    environment: GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment),
                    files: files)
    }
}
#endif
