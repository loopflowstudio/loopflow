// The local macOS registry query: `RegistryQuery` backed by a launcher-resolved
// `lf`. Discovery and history are `lf` subprocesses over the local store, run off the
// main actor so a slow query never stalls the UI.

#if os(macOS)
import Foundation
import Loopflow

enum RegistryQueryLocal {
    static let shared = RegistryQuery(runWithInput: { args, cwd, input in
        try await Task.detached(priority: .userInitiated) {
            try LocalWaveAgentLauncher.queryLf(args, cwd: cwd, input: input)
        }.value
    }, watchActiveSessions: {
        try await Task.detached(priority: .userInitiated) {
            let configuration = try ActiveSessionsLaunchConfiguration.current()
            let process = LocalWaveAgentLauncher.queryProcess(
                [configuration.helper, "runs", "--active", "--watch", "--json"]
            )
            process.environment = configuration.environment
            return try LocalActiveSessionsObservation.start(
                process: process,
                configurationChanged: { try ActiveSessionsLaunchConfiguration.current() != configuration }
            )
        }.value
    }) { args, cwd in
        try await Perf.measure(Perf.lf, args.prefix(2).joined(separator: " ")) {
            try await Task.detached(priority: .userInitiated) {
                try LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
            }.value
        }
    }
}

/// Invalidate on replacement; let lf resolve the selected Home/store itself.
/// Metadata is sufficient here: these installation files are atomically replaced.
private struct ActiveSessionsLaunchConfiguration: Equatable {
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
