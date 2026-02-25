#if os(macOS)
import Foundation

extension RepoState {
    convenience init() {
        let bundledDaemon = BundledDaemonManager()
        self.init(
            startBundledDaemon: {
                try await bundledDaemon.start()
            },
            shellCommandRunner: LocalShellCommandRunner.run
        )
    }
}
#endif
