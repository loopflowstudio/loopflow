// The local macOS registry query: `RegistryQuery` over this Machine. The Work
// watches, served reads and served writes run in the app through Loopflow's
// library; other commands, and the remote Task file watch, run the bundled `lf`
// off the main actor so a slow one never stalls the UI.

#if os(macOS)
import Foundation
import Loopflow

enum RegistryQueryLocal {
    static let shared = RegistryQuery(runWithInput: { args, cwd, input in
        try await LocalWaveAgentLauncher.queryLf(args, cwd: cwd, input: input)
    }, serve: { query in
        try await LoopflowLibrary.read(query)
    }, write: { change in
        try await LoopflowLibrary.write(change)
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
    }, watchWork: {
        try WorkWatches.observe(launch: LoopflowLibrary.watch)
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
