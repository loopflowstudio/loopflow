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
        let path = url.standardizedFileURL.path(percentEncoded: false)

        // Remove existing entry for this path
        repos.removeAll { $0.path == path }

        // Add to front
        let repo = PortfolioRepo(path: path, lastOpened: Date())
        repos.insert(repo, at: 0)

        saveRepos()
    }

    func removeRepo(_ url: URL) {
        let path = url.standardizedFileURL.path(percentEncoded: false)
        repos.removeAll { $0.path == path }
        saveRepos()
    }

    func clearAll() {
        repos = []
        saveRepos()
    }

    private func loadRepos() {
        if let data = defaults.data(forKey: key),
           let decoded = try? JSONDecoder().decode([PortfolioRepo].self, from: data) {
            repos = normalizedRepos(decoded)
            return
        }

        guard let legacyData = defaults.data(forKey: legacyKey),
              let legacyDecoded = try? JSONDecoder().decode([PortfolioRepo].self, from: legacyData) else {
            repos = []
            return
        }

        // Filter out repos that no longer exist
        repos = normalizedRepos(legacyDecoded)
        saveRepos()
    }

    private func saveRepos() {
        guard let data = try? JSONEncoder().encode(repos) else { return }
        defaults.set(data, forKey: key)
    }

    private func normalizedRepos(_ entries: [PortfolioRepo]) -> [PortfolioRepo] {
        var seen = Set<String>()
        var normalized: [PortfolioRepo] = []

        for entry in entries.sorted(by: { $0.lastOpened > $1.lastOpened }) {
            let path = URL(fileURLWithPath: entry.path).standardizedFileURL.path(percentEncoded: false)
            guard FileManager.default.fileExists(atPath: path), !seen.contains(path) else {
                continue
            }
            seen.insert(path)
            normalized.append(PortfolioRepo(path: path, lastOpened: entry.lastOpened))
        }

        return normalized
    }
}
