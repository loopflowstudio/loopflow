// Runs `lf ci watch` for each repository open in a window, and stops it on quit.
//
// The watcher is the same command a person can run in a terminal or install as
// a launchd service. A second copy stands by behind a live one, so Desktop
// never competes with either. Nothing here decides or repairs anything.

#if os(macOS)
import AppKit
import Foundation
import Loopflow

@MainActor
final class CIWatchers {
    static let shared = CIWatchers()

    private struct Watcher {
        let process: Process
        var windows: Int
    }

    private let launch: (String) -> Process?
    private var watchers: [String: Watcher] = [:]
    private var quitObserver: NSObjectProtocol?

    init(launch: @escaping (String) -> Process? = CIWatchers.launchWatcher) {
        self.launch = launch
    }

    /// `--parent-pid` stops the watcher if the app dies without quitting.
    nonisolated static func command(lfPath: String, parentPID: Int32) -> [String] {
        [lfPath, "repo", "ci", "watch", "--parent-pid", String(parentPID)]
    }

    var watchedRepos: Set<String> { Set(watchers.keys) }

    func open(_ repoPath: String) {
        let repo = repoPath.normalizedFilePath
        if let watcher = watchers[repo], watcher.process.isRunning {
            watchers[repo]?.windows += 1
            return
        }
        guard let process = launch(repo) else { return }
        watchers[repo] = Watcher(process: process, windows: 1)
        if quitObserver == nil {
            quitObserver = NotificationCenter.default.addObserver(
                forName: NSApplication.willTerminateNotification, object: nil, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.stopAll() }
            }
        }
    }

    func close(_ repoPath: String) {
        let repo = repoPath.normalizedFilePath
        guard var watcher = watchers[repo] else { return }
        watcher.windows -= 1
        if watcher.windows > 0 {
            watchers[repo] = watcher
            return
        }
        watchers[repo] = nil
        watcher.process.terminate()
    }

    func stopAll() {
        for watcher in watchers.values { watcher.process.terminate() }
        watchers.removeAll()
    }

    nonisolated private static func launchWatcher(_ repoPath: String) -> Process? {
        guard let lfPath = try? LocalWaveAgentLauncher.controlLfPath() else { return nil }
        let process = LocalWaveAgentLauncher.queryProcess(
            command(lfPath: lfPath, parentPID: ProcessInfo.processInfo.processIdentifier),
            cwd: repoPath
        )
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do { try process.run() } catch { return nil }
        return process
    }
}
#endif
