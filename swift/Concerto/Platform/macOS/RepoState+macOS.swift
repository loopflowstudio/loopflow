#if os(macOS)
import Foundation
import LoopflowCore

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

extension RepoState {
    convenience init() {
        let bundledDaemon = SharedDaemon.manager
        self.init(
            startBundledDaemon: {
                try await bundledDaemon.start()
            },
            shellCommandRunner: LocalShellCommandRunner.run
        )
    }
}
#endif
