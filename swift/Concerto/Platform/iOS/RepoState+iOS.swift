#if os(iOS)
import Foundation
import LoopflowCore

extension RepoState {
    convenience init() {
        self.init()
        connectionStore.setMode(.remote)
    }
}
#endif
