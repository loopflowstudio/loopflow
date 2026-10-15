import Foundation
import Loopflow

/// Retains the last shared reading while lf owns discovery and configuration.
@MainActor
@Observable
final class PortfolioService {
    private let query: RegistryQuery
    private let defaults: UserDefaults
    private(set) var selection: Portfolio?
    private(set) var error: String?
    private var pending: Task<Void, Never>?
    var isPending: Bool { pending != nil }
    private(set) var repos: [PortfolioRepo] = []

    init(query: RegistryQuery = RegistryQueryLocal.shared, defaults: UserDefaults = .standard) {
        self.query = query
        self.defaults = defaults
    }

    var legacyPaths: [String] {
        guard selection?.configured == false,
              let data = defaults.data(forKey: "portfolioRepos"),
              let entries = try? JSONDecoder().decode([LegacyRepo].self, from: data)
        else { return [] }
        return entries.map(\.path)
    }

    private struct LegacyRepo: Decodable { let path: String }

    func refresh() async {
        await perform([])
    }

    func perform(_ arguments: [String]) async {
        while let pending {
            await pending.value
            if arguments.isEmpty { return }
        }
        let operation = Task { await load(arguments) }
        pending = operation
        await operation.value
    }

    private func load(_ arguments: [String]) async {
        defer { pending = nil }
        do {
            let result = try await query.portfolio(arguments)
            // Failed folder reads retain the previous usable membership.
            selection = result
            if result.error == nil { repos = result.repos }
            error = result.error
        } catch {
            self.error = error.localizedDescription
        }
    }

    func importLegacy() async {
        let paths = legacyPaths
        guard !paths.isEmpty else { return }
        await perform(["import"] + paths)
    }
}
