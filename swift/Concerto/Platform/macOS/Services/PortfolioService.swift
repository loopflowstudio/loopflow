// Service for persisting portfolio repositories.

import Foundation

@Observable
final class PortfolioService {
    private let defaults: UserDefaults
    private let key: String

    private(set) var repos: [PortfolioRepo] = []

    init(
        defaults: UserDefaults = .standard,
        key: String = "portfolioRepos"
    ) {
        self.defaults = defaults
        self.key = key
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

    private func loadRepos() {
        guard let data = defaults.data(forKey: key),
              let decoded = try? JSONDecoder().decode([PortfolioRepo].self, from: data)
        else {
            repos = []
            return
        }
        repos = normalizedRepos(decoded)
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

