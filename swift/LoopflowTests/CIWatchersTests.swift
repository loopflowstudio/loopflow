// Desktop starts one CI watcher per open repository and stops it when the last
// window on that repository goes away.

#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("CI watchers")
@MainActor
struct CIWatchersTests {
    private static func sleeper(_: String) -> Foundation.Process? {
        let process = Foundation.Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sleep")
        process.arguments = ["60"]
        try? process.run()
        return process
    }

    @Test("Desktop runs the same command a person would")
    func commandShape() {
        #expect(CIWatchers.command(lfPath: "/Applications/Loopflow.app/Contents/MacOS/lf", parentPID: 42) == [
            "/Applications/Loopflow.app/Contents/MacOS/lf", "repo", "ci", "watch", "--parent-pid", "42",
        ])
    }

    @Test("one watcher per repository lasts until its last window closes")
    func oneWatcherPerRepository() {
        var launched: [Foundation.Process] = []
        let watchers = CIWatchers { repo in
            let process = Self.sleeper(repo)
            if let process { launched.append(process) }
            return process
        }
        let repo = "/tmp/repo".normalizedFilePath
        let other = "/tmp/other".normalizedFilePath
        watchers.open(repo)
        watchers.open(repo)
        watchers.open(other)
        #expect(launched.count == 2)
        #expect(watchers.watchedRepos == [repo, other])

        watchers.close(repo)
        #expect(watchers.watchedRepos == [repo, other])
        #expect(launched[0].isRunning)

        watchers.close(repo)
        launched[0].waitUntilExit()
        #expect(watchers.watchedRepos == [other])

        watchers.stopAll()
        launched[1].waitUntilExit()
        #expect(watchers.watchedRepos.isEmpty)
    }
}
#endif
