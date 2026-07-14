#if os(macOS)
import Foundation
import Loopflow

@MainActor
enum SharedDaemon {
    static let manager = BundledDaemonManager()

    static func eagerStart() {
        guard ConnectionStore().mode == .bundled else { return }
        manager.eagerStart()
    }

    static var currentConnection: ServerConnection? {
        manager.currentConnection
    }
}

#endif
