import Foundation
import LoopflowCore

@MainActor
final class BundledDaemonRegistry {
    static let shared = BundledDaemonRegistry()

    private struct Entry {
        let manager: BundledDaemonManager
        var owners: Int
    }

    private var entries: [String: Entry] = [:]

    func acquire(repoURL: URL) async throws -> ServerConnection {
        let canonicalURL = canonicalRepoURL(from: repoURL)
        let key = canonicalURL.path

        if var entry = entries[key] {
            entry.owners += 1
            entries[key] = entry
            return try await entry.manager.start()
        }

        let manager = BundledDaemonManager(repoURL: canonicalURL)
        entries[key] = Entry(manager: manager, owners: 1)

        do {
            return try await manager.start()
        } catch {
            entries.removeValue(forKey: key)
            throw error
        }
    }

    func release(repoURL: URL) {
        let key = canonicalRepoURL(from: repoURL).path
        guard var entry = entries[key] else { return }

        entry.owners -= 1
        if entry.owners <= 0 {
            entry.manager.stop()
            entries.removeValue(forKey: key)
        } else {
            entries[key] = entry
        }
    }

    func stopAll() {
        for entry in entries.values {
            entry.manager.stop()
        }
        entries.removeAll()
    }

    private func canonicalRepoURL(from repoURL: URL) -> URL {
        repoURL
            .resolvingSymlinksInPath()
            .standardizedFileURL
    }
}
