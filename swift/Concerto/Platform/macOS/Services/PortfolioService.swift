// Service for persisting portfolio repositories.

import Foundation

@Observable
final class PortfolioService {
    private let defaults: UserDefaults
    private let key: String

    private(set) var repos: [PortfolioRepo] = []

    var orderedRepos: [PortfolioRepo] {
        repos.sorted { lhs, rhs in
            let lhsTier = lhs.tier
            let rhsTier = rhs.tier
            if lhsTier.order != rhsTier.order {
                return lhsTier.order < rhsTier.order
            }
            if lhs.priority != rhs.priority {
                return lhs.priority < rhs.priority
            }
            if lhs.lastOpened != rhs.lastOpened {
                return lhs.lastOpened > rhs.lastOpened
            }
            return lhs.path.localizedCaseInsensitiveCompare(rhs.path) == .orderedAscending
        }
    }

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
        if let existingIndex = repos.firstIndex(where: { $0.path == path }) {
            repos[existingIndex].lastOpened = Date()
            saveRepos()
            return
        }

        let active = PortfolioTier.default
        let priority = repos
            .filter { $0.tier.id == active.id }
            .map(\.priority)
            .min()
            .map { $0 - 1 } ?? 0
        repos.append(
            PortfolioRepo(path: path, lastOpened: Date(), tierId: active.id, priority: priority)
        )
        saveRepos()
    }

    func removeRepo(_ url: URL) {
        repos.removeAll { $0.path == url.normalizedFilePath }
        saveRepos()
    }

    func reposByTier() -> [(tier: PortfolioTier, repos: [PortfolioRepo])] {
        let grouped = Dictionary(grouping: orderedRepos) { repo in
            repo.tier.id
        }
        return PortfolioTier.all.map { tier in
            (tier, grouped[tier.id] ?? [])
        }
    }

    func reorder(_ movedPath: String, into tier: PortfolioTier, above: PortfolioRepo?, below: PortfolioRepo?) {
        let normalizedPath = movedPath.normalizedFilePath
        guard let movedIndex = repos.firstIndex(where: { $0.path == normalizedPath }) else { return }

        let validAbove = above?.path == normalizedPath ? nil : above
        let validBelow = below?.path == normalizedPath ? nil : below
        repos[movedIndex].tierId = tier.id
        repos[movedIndex].priority = priority(above: validAbove, below: validBelow)
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

        for entry in entries {
            let path = entry.path.normalizedFilePath
            guard FileManager.default.fileExists(atPath: path), !seen.contains(path) else {
                continue
            }
            seen.insert(path)
            normalized.append(
                PortfolioRepo(
                    path: path,
                    lastOpened: entry.lastOpened,
                    tierId: entry.tier.id,
                    priority: entry.priority
                )
            )
        }

        return normalized
    }

    private func priority(above: PortfolioRepo?, below: PortfolioRepo?) -> Double {
        switch (above, below) {
        case (.some(let above), .some(let below)):
            return (above.priority + below.priority) / 2
        case (.some(let above), .none):
            return above.priority + 1
        case (.none, .some(let below)):
            return below.priority - 1
        case (.none, .none):
            return 0
        }
    }
}
