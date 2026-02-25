#if os(macOS)
import Foundation
import LoopflowCore

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
