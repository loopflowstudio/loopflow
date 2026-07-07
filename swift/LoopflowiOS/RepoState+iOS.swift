#if os(iOS)
import Foundation
import Loopflow

extension RepoState {
    convenience init() {
        self.init(startBundledDaemon: nil, shellCommandRunner: nil)
        connectionStore.setMode(.remote)
    }
}
#endif
