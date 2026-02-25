#if os(iOS)
import Foundation
import LoopflowCore

extension RepoState {
    convenience init() {
        self.init(
            startBundledDaemon: {
                throw WaveServiceError.commandFailed("Bundled daemon is unavailable on iOS. Use a remote lfd connection.")
            },
            shellCommandRunner: nil
        )
        connectionStore.setMode(.remote)
    }
}
#endif
