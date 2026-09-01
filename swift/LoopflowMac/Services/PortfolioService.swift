// Service for persisting portfolio repositories.

import Foundation

@Observable
final class PortfolioService {
    private let defaults: UserDefaults
    private let key: String
    private let scanner = RepoScanner()

    private(set) var repos: [PortfolioRepo] = []

    init(
        defaults: UserDefaults = .standard,
        key: String = "portfolioRepos"
    ) {
        self.defaults = defaults
        self.key = key
        loadRepos()
    }

    @discardableResult
    func addRepo(_ url: URL) -> URL? {
        guard let main = scanner.mainRepository(url) else { return nil }
        let path = main.normalizedFilePath
        repos.removeAll { $0.path == path }
        repos.insert(PortfolioRepo(path: path, lastOpened: Date()), at: 0)
        saveRepos()
        return main
    }

    func removeRepo(_ url: URL) {
        let path = scanner.mainRepository(url)?.normalizedFilePath
            ?? url.normalizedFilePath
        repos.removeAll { $0.path == path }
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
        if repos != decoded { saveRepos() }
    }

    private func saveRepos() {
        guard let data = try? JSONEncoder().encode(repos) else { return }
        defaults.set(data, forKey: key)
    }

    private func normalizedRepos(_ entries: [PortfolioRepo]) -> [PortfolioRepo] {
        var seen = Set<String>()
        var normalized: [PortfolioRepo] = []

        for entry in entries.sorted(by: { $0.lastOpened > $1.lastOpened }) {
            guard let main = scanner.mainRepository(entry.url) else { continue }
            let path = main.normalizedFilePath
            guard seen.insert(path).inserted else { continue }
            normalized.append(PortfolioRepo(path: path, lastOpened: entry.lastOpened))
        }

        return normalized
    }
}
