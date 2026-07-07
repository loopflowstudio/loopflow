#if os(iOS)
import Foundation
import LoopflowCore

extension RepoState {
    convenience init() {
        self.init(startBundledDaemon: nil, shellCommandRunner: nil)
        connectionStore.setMode(.remote)
    }
}
#endif
