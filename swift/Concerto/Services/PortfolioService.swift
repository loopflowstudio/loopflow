// Service for persisting portfolio repositories.

import Foundation

@Observable
final class PortfolioService {
    private let defaults: UserDefaults
    private let key: String
    private let legacyKey: String

    private(set) var repos: [PortfolioRepo] = []

    init(
        defaults: UserDefaults = .standard,
        key: String = "portfolioRepos",
        legacyKey: String = "recentRepos"
    ) {
        self.defaults = defaults
        self.key = key
        self.legacyKey = legacyKey
        loadRepos()
    }

    func addRepo(_ url: URL) {
        let path = url.normalizedFilePath
        repos.removeAll { $0.path == path }
        repos.insert(PortfolioRepo(path: path, lastOpened: Date()), at: 0)
        saveRepos()
    }

    func removeRepo(_ url: URL) {
        repos.removeAll { $0.path == url.normalizedFilePath }
        saveRepos()
    }

    func clearAll() {
        repos = []
        saveRepos()
    }

    private func loadRepos() {
        if let storedRepos = decodedRepos(forKey: key) {
            repos = normalizedRepos(storedRepos)
            return
        }

        guard let legacyRepos = decodedRepos(forKey: legacyKey) else {
            repos = []
            return
        }

        repos = normalizedRepos(legacyRepos)
        saveRepos()
    }

    private func decodedRepos(forKey key: String) -> [PortfolioRepo]? {
        guard let data = defaults.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode([PortfolioRepo].self, from: data)
    }

    private func saveRepos() {
        guard let data = try? JSONEncoder().encode(repos) else { return }
        defaults.set(data, forKey: key)
    }

    private func normalizedRepos(_ entries: [PortfolioRepo]) -> [PortfolioRepo] {
        var seen = Set<String>()
        var normalized: [PortfolioRepo] = []

        for entry in entries.sorted(by: { $0.lastOpened > $1.lastOpened }) {
            let path = entry.path.normalizedFilePath
            guard FileManager.default.fileExists(atPath: path), !seen.contains(path) else {
                continue
            }
            seen.insert(path)
            normalized.append(PortfolioRepo(path: path, lastOpened: entry.lastOpened))
        }

        return normalized
    }
}
