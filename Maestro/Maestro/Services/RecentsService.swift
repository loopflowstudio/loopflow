// Service for persisting recently opened repositories.

import Foundation

@Observable
final class RecentsService {
    private let defaults = UserDefaults.standard
    private let key = "recentRepos"
    private let maxRecents = 10

    private(set) var recentRepos: [RecentRepo] = []

    init() {
        loadRecents()
    }

    func addRecent(_ url: URL) {
        let path = url.path(percentEncoded: false)

        // Remove existing entry for this path
        recentRepos.removeAll { $0.path == path }

        // Add to front
        let recent = RecentRepo(path: path, lastOpened: Date())
        recentRepos.insert(recent, at: 0)

        // Trim to max
        if recentRepos.count > maxRecents {
            recentRepos = Array(recentRepos.prefix(maxRecents))
        }

        saveRecents()
    }

    func removeRecent(_ url: URL) {
        let path = url.path(percentEncoded: false)
        recentRepos.removeAll { $0.path == path }
        saveRecents()
    }

    func clearAll() {
        recentRepos = []
        saveRecents()
    }

    private func loadRecents() {
        guard let data = defaults.data(forKey: key),
              let decoded = try? JSONDecoder().decode([RecentRepo].self, from: data) else {
            recentRepos = []
            return
        }

        // Filter out repos that no longer exist
        recentRepos = decoded.filter { $0.exists }
    }

    private func saveRecents() {
        guard let data = try? JSONEncoder().encode(recentRepos) else { return }
        defaults.set(data, forKey: key)
    }
}
