import Foundation

final class TmuxSessionRegistry: @unchecked Sendable {
    typealias KillSession = (String) throws -> Void

    static let shared = TmuxSessionRegistry()

    private let killSession: KillSession
    private let lock = NSLock()
    private var trackedSessionNames: Set<String> = []

    init(killSession: @escaping KillSession = TmuxSession.killSessionIfExists(named:)) {
        self.killSession = killSession
    }

    func track(sessionName: String) {
        _ = lock.withLock {
            trackedSessionNames.insert(sessionName)
        }
    }

    func untrack(sessionName: String) {
        _ = lock.withLock {
            trackedSessionNames.remove(sessionName)
        }
    }

    func killSession(named sessionName: String) {
        untrack(sessionName: sessionName)
        try? killSession(sessionName)
    }

    func killSessions(named sessionNames: [String]) {
        for sessionName in sessionNames {
            killSession(named: sessionName)
        }
    }

    func killAllSynchronously() {
        let sessionNames = lock.withLock {
            let names = trackedSessionNames
            trackedSessionNames.removeAll()
            return names
        }

        for sessionName in sessionNames {
            try? killSession(sessionName)
        }
    }
}

private extension NSLock {
    func withLock<T>(_ body: () -> T) -> T {
        lock()
        defer { unlock() }
        return body()
    }
}
